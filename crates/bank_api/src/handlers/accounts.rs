use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use bank_domain::{AccountError, AccountType, Accounts, Status};
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

pub async fn create_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateAccountRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AccountResponse>>), HttpError<AccountError>> {
    let account = state
        .account_service
        .create_account(auth.user_id, req.account_type)
        .await?;

    Ok(ApiResponse::created(AccountResponse::from(account)))
}
