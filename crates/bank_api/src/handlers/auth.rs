use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use bank_domain::AuthError;
use crate::response::{ApiResponse, HttpError};
use crate::services::AuthTokens;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub user_name: String,
    pub password: String,
    pub device_info: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AuthTokens>>), HttpError<AuthError>> {
    let tokens = state
        .auth_service
        .login(req.user_name, req.password, req.device_info)
        .await?;
    Ok(ApiResponse::created(tokens))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AuthTokens>>), HttpError<AuthError>> {
    let tokens = state.auth_service.refresh(req.refresh_token).await?;
    Ok(ApiResponse::created(tokens))
}

pub async fn logout(
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> Result<(StatusCode, Json<ApiResponse<()>>), HttpError<AuthError>> {
    state.auth_service.logout(req.refresh_token).await?;
    Ok(ApiResponse::created(()))
}
