use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use bank_domain::{AccountType, Accounts, Status};
use crate::handlers::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub user_id: i64,
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
    Json(req): Json<CreateAccountRequest>,
) -> Result<Json<AccountResponse>, ApiError> {
    let account = state
        .account_service
        .create_account(req.user_id, req.account_type)
        .await?;

    Ok(Json(AccountResponse::from(account)))
}
