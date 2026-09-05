use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use bank_domain::{
    AccountError, AccountType, Accounts, CreateAccountCommand, CreateAccountResult, Status,
};
use crate::extractors::AuthUser;
use crate::response::{ApiResponse, HttpError, WebError};
use crate::state::AppState;

impl WebError for AccountError {
    fn status_code(&self) -> StatusCode {
        match self {
            AccountError::NotFound => StatusCode::NOT_FOUND,
            AccountError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            AccountError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub account_type: AccountType,
}

#[derive(Debug, Serialize)]
pub struct AccountResponse {
    pub account_id: i64,
    pub account_number: String,
    pub account_type: AccountType,
    pub user_id: i64,
    pub status: Status,
    pub balance_cents: i64,
    pub created_at: DateTime<Utc>,
}

impl From<Accounts> for AccountResponse {
    fn from(acc: Accounts) -> Self {
        AccountResponse {
            account_id: acc.account_id.value(),
            account_number: acc.account_number.into_inner(),
            account_type: acc.account_type,
            user_id: acc.user_id.value(),
            status: acc.status,
            balance_cents: acc.balance.cents(),
            created_at: acc.created_at,
        }
    }
}

fn map_account_error(err_msg: Option<&str>) -> AccountError {
    match err_msg {
        Some(s) if s.contains("NotFound") => AccountError::NotFound,
        Some(s) if s.contains("InvalidInput") => AccountError::InvalidInput(s.to_string()),
        Some(s) => AccountError::DatabaseError(s.to_string()),
        None => AccountError::DatabaseError("Unknown worker error".to_string()),
    }
}

pub async fn create_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateAccountRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AccountResponse>>), HttpError<AccountError>> {
    // 1. Setup dynamic reply inbox and correlation ID
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let inbox = state.nats.new_inbox();
    let mut reply_sub = state
        .nats
        .subscribe(inbox.clone())
        .await
        .map_err(|e| HttpError(AccountError::DatabaseError(format!("NATS subscribe error: {}", e))))?;

    // 2. Publish CreateAccountCommand to NATS
    let command = CreateAccountCommand {
        correlation_id,
        reply_to: Some(inbox),
        user_id: auth.user_id,
        account_type: req.account_type,
        created_at: Utc::now(),
    };

    let payload = serde_json::to_vec(&command)
        .map_err(|e| HttpError(AccountError::DatabaseError(format!("Payload error: {}", e))))?;

    state
        .nats
        .publish("bank.accounts.create", payload.into())
        .await
        .map_err(|e| HttpError(AccountError::DatabaseError(format!("NATS publish error: {}", e))))?;

    // 3. Non-blocking await for Worker response with timeout
    let reply_msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), reply_sub.next())
        .await
        .map_err(|_| HttpError(AccountError::DatabaseError("Account creation timed out waiting for worker".into())))?
        .ok_or_else(|| HttpError(AccountError::DatabaseError("NATS reply channel closed".into())))?;

    let result: CreateAccountResult = serde_json::from_slice(&reply_msg.payload)
        .map_err(|e| HttpError(AccountError::DatabaseError(format!("Deserialization error: {}", e))))?;

    if result.success {
        let account = result.account.ok_or_else(|| {
            HttpError(AccountError::DatabaseError("Missing account in result".into()))
        })?;
        Ok(ApiResponse::created(AccountResponse::from(account)))
    } else {
        let err = map_account_error(result.error_message.as_deref());
        Err(HttpError(err))
    }
}
