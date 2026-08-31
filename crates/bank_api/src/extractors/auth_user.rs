use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use bank_domain::{AccessToken, AuthError, UsersID};
use crate::response::{HttpError, WebError};
use crate::state::AppState;

impl WebError for AuthError {
    fn status_code(&self) -> StatusCode {
        match self {
            AuthError::InvalidToken
            | AuthError::ExpiredToken
            | AuthError::InvalidSignature
            | AuthError::InvalidCredentials
            | AuthError::InvalidRefreshToken => StatusCode::UNAUTHORIZED,
            AuthError::GenerationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub user_id: UsersID,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = HttpError<AuthError>;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|val| val.to_str().ok())
            .ok_or(HttpError(AuthError::InvalidToken))?;

        if !auth_header.starts_with("Bearer ") {
            return Err(HttpError(AuthError::InvalidToken));
        }

        let token = &auth_header[7..];
        let claims = AccessToken::verify(token, &state.jwt_secret)
            .map_err(HttpError)?;

        Ok(AuthUser {
            user_id: UsersID::from_db(claims.sub),
        })
    }
}
