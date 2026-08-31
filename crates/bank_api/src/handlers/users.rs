use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use bank_domain::{Users, UsersError};
use crate::response::{ApiResponse, HttpError, WebError};
use crate::state::AppState;

impl WebError for UsersError {
    fn status_code(&self) -> StatusCode {
        match self {
            UsersError::NotFound => StatusCode::NOT_FOUND,
            UsersError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            UsersError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUsersRequest {
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
    pub user_name: String,
    pub password: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UsersResponse {
    pub id: i64,
    pub name: String,
    pub user_name: String,
    pub created_at: DateTime<Utc>,
}

impl From<Users> for UsersResponse {
    fn from(user: Users) -> Self {
        UsersResponse {
            id: user.user_id.value(),
            name: user.name.full_name(),
            user_name: user.user_name,
            created_at: user.created_at,
        }
    }
}

pub async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUsersRequest>,
) -> Result<(StatusCode, Json<ApiResponse<UsersResponse>>), HttpError<UsersError>> {
    let user = state
        .user_service
        .create_user(
            req.first_name,
            req.middle_name,
            req.last_name,
            req.user_name,
            req.password,
            req.email,
        )
        .await?;

    Ok(ApiResponse::created(UsersResponse::from(user)))
}
