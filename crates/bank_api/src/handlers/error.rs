use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use bank_domain::{AccountError, UsersError};
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    Users(UsersError),
    Account(AccountError),
}

impl From<UsersError> for ApiError {
    fn from(err: UsersError) -> Self {
        ApiError::Users(err)
    }
}

impl From<AccountError> for ApiError {
    fn from(err: AccountError) -> Self {
        ApiError::Account(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Users(UsersError::NotFound) => (StatusCode::NOT_FOUND, "user not found".to_string()),
            ApiError::Users(UsersError::InvalidInput(msg)) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Users(UsersError::DatabaseError(msg)) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::Account(AccountError::NotFound) => (StatusCode::NOT_FOUND, "account not found".to_string()),
            ApiError::Account(AccountError::InvalidInput(msg)) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Account(AccountError::DatabaseError(msg)) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
