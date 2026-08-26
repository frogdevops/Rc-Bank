use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::users::{UsersID};

pub(crate) struct AccountID(i64);

impl AccountID {
	pub(crate) fn from_db(value: i64)-> Self {
		AccountID(value)
	}
	pub(crate) fn value(&self) -> i64{
		self.0
	}
}
pub(crate) struct AccountNumber(String);
pub(crate) struct Balance(i64);

pub(crate) enum AccountError {
	NotFound,
	InvalidInput(String),
	DatabaseError(String),
}

impl IntoResponse for AccountError {
	fn into_response(self) -> Response {
		let (status, message) = match self {
			AccountError::NotFound => (StatusCode::NOT_FOUND, "account not found".to_string()),
			AccountError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
			AccountError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
		};
		(status, Json(serde_json::json!({ "error": message }))).into_response()
	}
}

impl Balance {
	pub fn new(cents: i64) -> Result<Self, AccountError> {
		if cents < 0 {
			return Err(AccountError::InvalidInput("cents must be positive".into()));
		}
		Ok(Self(cents))
	}

	pub fn cents(&self) -> i64 {
		self.0
	}
}
pub(crate) struct UserID(i32);

impl UserID {
	//TODO: Get UserId from database
	// Simple handling
}
#[derive(Debug, Clone, Serialize)]
pub(crate) enum Status {
	Active,
	Frozen,
	Closed,
	Suspended,
}

impl Status {
	// TODO: Convert to String based on the status
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum AccountType {
	Savings,
	Checking,
}

impl AccountType {
	// TODO: From request gets deserialized
}
pub(crate) struct Accounts {
	account_id: AccountID,
	account_number: AccountNumber,
	user_id: UserID,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
	balance: Balance,
	status: Status,
}

impl Accounts {
	// TODO: wire up
}

pub(crate) struct NewAccount {
	user_id: UsersID,
	account_number: AccountNumber,
	account_type: AccountType,
}

pub(crate) struct CreateAccountSystem {

}


