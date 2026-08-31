use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use bank_domain::{
    AccountID, AccountNumber, Amount, Balance, TransactionError, TransactionType, Transactions,
};
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
    let amount = Amount::new(req.amount_cents)?;
    let tx = state
        .transactions_service
        .deposit(auth.user_id, AccountID::from_db(account_id), amount)
        .await?;

    Ok(ApiResponse::created(TransactionResponse::from(tx)))
}

pub async fn withdraw(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(account_id): Path<i64>,
    Json(req): Json<WithdrawRequest>,
) -> Result<(StatusCode, Json<ApiResponse<TransactionResponse>>), HttpError<TransactionError>> {
    let amount = Amount::new(req.amount_cents)?;
    let tx = state
        .transactions_service
        .withdraw(auth.user_id, AccountID::from_db(account_id), amount)
        .await?;

    Ok(ApiResponse::created(TransactionResponse::from(tx)))
}

pub async fn transfer(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TransferRequest>,
) -> Result<(StatusCode, Json<ApiResponse<TransferResponse>>), HttpError<TransactionError>> {
    let amount = Amount::new(req.amount_cents)?;
    let target_account_num = AccountNumber::from_db(req.to_account_number);
    let (tx_out, tx_in) = state
        .transactions_service
        .transfer(
            auth.user_id,
            AccountID::from_db(req.from_account_id),
            target_account_num,
            amount,
        )
        .await?;

    Ok(ApiResponse::created(TransferResponse {
        debit_transaction: TransactionResponse::from(tx_out),
        credit_transaction: TransactionResponse::from(tx_in),
    }))
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
