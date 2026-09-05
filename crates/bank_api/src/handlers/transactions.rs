use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use bank_domain::{
    AccountID, AccountNumber, Amount, Balance, DepositCommand, MoneyResult, TransactionError,
    TransactionType, Transactions, TransferCommand, TransferResult, WithdrawCommand,
};
use futures::StreamExt;
use crate::extractors::AuthUser;
use crate::response::{ApiResponse, HttpError, WebError};
use crate::state::AppState;

impl WebError for TransactionError {
    fn status_code(&self) -> StatusCode {
        match self {
            TransactionError::InsufficientFunds
            | TransactionError::InvalidAmount(_)
            | TransactionError::AccountNotActive
            | TransactionError::SelfTransferNotAllowed => StatusCode::BAD_REQUEST,
            TransactionError::AccountNotFound => StatusCode::NOT_FOUND,
            TransactionError::UnauthorizedAccountAccess => StatusCode::FORBIDDEN,
            TransactionError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DepositRequest {
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawRequest {
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    pub from_account_id: i64,
    pub to_account_number: String,
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct StatementQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub transaction_id: i64,
    pub account_id: i64,
    pub amount_cents: i64,
    pub transaction_type: TransactionType,
    pub previous_hash: Option<String>,
    pub current_hash: String,
    pub created_at: DateTime<Utc>,
}

impl From<Transactions> for TransactionResponse {
    fn from(tx: Transactions) -> Self {
        TransactionResponse {
            transaction_id: tx.transaction_id.value(),
            account_id: tx.account_id.value(),
            amount_cents: tx.amount_cents,
            transaction_type: tx.transaction_type,
            previous_hash: tx.previous_hash,
            current_hash: tx.current_hash,
            created_at: tx.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TransferResponse {
    pub debit_transaction: TransactionResponse,
    pub credit_transaction: TransactionResponse,
}

#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub account_id: i64,
    pub balance_cents: i64,
}

pub async fn deposit(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(account_id): Path<i64>,
    Json(req): Json<DepositRequest>,
) -> Result<(StatusCode, Json<ApiResponse<TransactionResponse>>), HttpError<TransactionError>> {
    // 1. Fast-fail input validation
    Amount::new(req.amount_cents)?;

    // 2. Setup dynamic reply inbox and correlation ID
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let inbox = state.nats.new_inbox();
    let mut reply_sub = state
        .nats
        .subscribe(inbox.clone())
        .await
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("NATS subscribe error: {}", e))))?;

    // 3. Publish DepositCommand to NATS
    let command = DepositCommand {
        correlation_id,
        reply_to: Some(inbox),
        user_id: auth.user_id,
        account_id: AccountID::from_db(account_id),
        amount_cents: req.amount_cents,
        created_at: Utc::now(),
    };

    let payload = serde_json::to_vec(&command)
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("Payload error: {}", e))))?;

    state
        .nats
        .publish("bank.deposits", payload.into())
        .await
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("NATS publish error: {}", e))))?;

    // 4. Non-blocking await for Worker response with timeout
    let reply_msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), reply_sub.next())
        .await
        .map_err(|_| HttpError(TransactionError::DatabaseError("Deposit timed out waiting for worker".into())))?
        .ok_or_else(|| HttpError(TransactionError::DatabaseError("NATS reply channel closed".into())))?;

    let result: MoneyResult = serde_json::from_slice(&reply_msg.payload)
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("Deserialization error: {}", e))))?;

    if result.success {
        let tx = result.transaction.ok_or_else(|| {
            HttpError(TransactionError::DatabaseError("Missing transaction in result".into()))
        })?;
        Ok(ApiResponse::created(TransactionResponse::from(tx)))
    } else {
        let err = map_money_error(result.error_message.as_deref());
        Err(HttpError(err))
    }
}

/// Maps a stringified error from a MoneyResult back to a typed TransactionError.
fn map_money_error(err_msg: Option<&str>) -> TransactionError {
    match err_msg {
        Some(s) if s.contains("InsufficientFunds") => TransactionError::InsufficientFunds,
        Some(s) if s.contains("AccountNotActive") => TransactionError::AccountNotActive,
        Some(s) if s.contains("AccountNotFound") => TransactionError::AccountNotFound,
        Some(s) if s.contains("UnauthorizedAccountAccess") => TransactionError::UnauthorizedAccountAccess,
        Some(s) => TransactionError::DatabaseError(s.to_string()),
        None => TransactionError::DatabaseError("Unknown worker error".to_string()),
    }
}

pub async fn withdraw(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(account_id): Path<i64>,
    Json(req): Json<WithdrawRequest>,
) -> Result<(StatusCode, Json<ApiResponse<TransactionResponse>>), HttpError<TransactionError>> {
    // 1. Fast-fail input validation
    Amount::new(req.amount_cents)?;

    // 2. Setup dynamic reply inbox and correlation ID
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let inbox = state.nats.new_inbox();
    let mut reply_sub = state
        .nats
        .subscribe(inbox.clone())
        .await
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("NATS subscribe error: {}", e))))?;

    // 3. Publish WithdrawCommand to NATS
    let command = WithdrawCommand {
        correlation_id,
        reply_to: Some(inbox),
        user_id: auth.user_id,
        account_id: AccountID::from_db(account_id),
        amount_cents: req.amount_cents,
        created_at: Utc::now(),
    };

    let payload = serde_json::to_vec(&command)
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("Payload error: {}", e))))?;

    state
        .nats
        .publish("bank.withdrawals", payload.into())
        .await
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("NATS publish error: {}", e))))?;

    // 4. Non-blocking await for Worker response with timeout
    let reply_msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), reply_sub.next())
        .await
        .map_err(|_| HttpError(TransactionError::DatabaseError("Withdraw timed out waiting for worker".into())))?
        .ok_or_else(|| HttpError(TransactionError::DatabaseError("NATS reply channel closed".into())))?;

    let result: MoneyResult = serde_json::from_slice(&reply_msg.payload)
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("Deserialization error: {}", e))))?;

    if result.success {
        let tx = result.transaction.ok_or_else(|| {
            HttpError(TransactionError::DatabaseError("Missing transaction in result".into()))
        })?;
        Ok(ApiResponse::created(TransactionResponse::from(tx)))
    } else {
        let err = map_money_error(result.error_message.as_deref());
        Err(HttpError(err))
    }
}

pub async fn transfer(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TransferRequest>,
) -> Result<(StatusCode, Json<ApiResponse<TransferResponse>>), HttpError<TransactionError>> {
    // 1. Fast-fail input validation
    Amount::new(req.amount_cents)?;

    // 2. Setup dynamic reply inbox and correlation ID
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let inbox = state.nats.new_inbox();
    let mut reply_sub = state
        .nats
        .subscribe(inbox.clone())
        .await
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("NATS subscribe error: {}", e))))?;

    // 3. Publish TransferCommand to NATS
    let command = TransferCommand {
        correlation_id,
        reply_to: Some(inbox),
        user_id: auth.user_id,
        from_account_id: AccountID::from_db(req.from_account_id),
        to_account_number: AccountNumber::from_db(req.to_account_number),
        amount_cents: req.amount_cents,
        created_at: Utc::now(),
    };

    let payload = serde_json::to_vec(&command)
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("Payload error: {}", e))))?;

    state
        .nats
        .publish("bank.transfers", payload.into())
        .await
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("NATS publish error: {}", e))))?;

    // 4. Non-blocking await for Worker response with timeout
    let reply_msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), reply_sub.next())
        .await
        .map_err(|_| HttpError(TransactionError::DatabaseError("Transfer timed out waiting for worker".into())))?
        .ok_or_else(|| HttpError(TransactionError::DatabaseError("NATS reply channel closed".into())))?;

    let result: TransferResult = serde_json::from_slice(&reply_msg.payload)
        .map_err(|e| HttpError(TransactionError::DatabaseError(format!("Deserialization error: {}", e))))?;

    if result.success {
        let debit = result.debit_transaction.ok_or_else(|| {
            HttpError(TransactionError::DatabaseError("Missing debit transaction in result".into()))
        })?;
        let credit = result.credit_transaction.ok_or_else(|| {
            HttpError(TransactionError::DatabaseError("Missing credit transaction in result".into()))
        })?;

        Ok(ApiResponse::created(TransferResponse {
            debit_transaction: TransactionResponse::from(debit),
            credit_transaction: TransactionResponse::from(credit),
        }))
    } else {
        let err_msg: Option<&str> = result.error_message.as_deref();
        let err = match err_msg {
            Some(s) if s.contains("InsufficientFunds") => TransactionError::InsufficientFunds,
            Some(s) if s.contains("AccountNotActive") => TransactionError::AccountNotActive,
            Some(s) if s.contains("AccountNotFound") => TransactionError::AccountNotFound,
            Some(s) if s.contains("SelfTransferNotAllowed") => TransactionError::SelfTransferNotAllowed,
            Some(s) if s.contains("UnauthorizedAccountAccess") => TransactionError::UnauthorizedAccountAccess,
            Some(s) => TransactionError::DatabaseError(s.to_string()),
            None => TransactionError::DatabaseError("Unknown worker error".to_string()),
        };
        Err(HttpError(err))
    }
}

pub async fn get_balance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(account_id): Path<i64>,
) -> Result<(StatusCode, Json<ApiResponse<BalanceResponse>>), HttpError<TransactionError>> {
    let balance: Balance = state
        .transactions_service
        .get_balance(auth.user_id, AccountID::from_db(account_id))
        .await?;

    Ok(ApiResponse::created(BalanceResponse {
        account_id,
        balance_cents: balance.cents(),
    }))
}

pub async fn get_statement(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(account_id): Path<i64>,
    Query(query): Query<StatementQuery>,
) -> Result<(StatusCode, Json<ApiResponse<Vec<TransactionResponse>>>), HttpError<TransactionError>> {
    let limit = query.limit.unwrap_or(20);
    let transactions: Vec<Transactions> = state
        .transactions_service
        .get_statement(auth.user_id, AccountID::from_db(account_id), limit)
        .await?;

    let responses: Vec<TransactionResponse> =
        transactions.into_iter().map(TransactionResponse::from).collect();

    Ok(ApiResponse::created(responses))
}
