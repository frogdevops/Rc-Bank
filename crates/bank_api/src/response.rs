use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

pub trait WebError: std::error::Error {
    fn status_code(&self) -> StatusCode;
    fn error_message(&self) -> String {
        self.to_string()
    }
}

#[derive(Debug)]
pub struct HttpError<E>(pub E);

impl<E: WebError> From<E> for HttpError<E> {
    fn from(err: E) -> Self {
        HttpError(err)
    }
}

impl<E: WebError> IntoResponse for HttpError<E> {
    fn into_response(self) -> Response {
        json_error(self.0.status_code(), self.0.error_message())
    }
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> (StatusCode, Json<Self>) {
        (
            StatusCode::OK,
            Json(Self {
                success: true,
                data: Some(data),
                error: None,
            }),
        )
    }

    pub fn created(data: T) -> (StatusCode, Json<Self>) {
        (
            StatusCode::CREATED,
            Json(Self {
                success: true,
                data: Some(data),
                error: None,
            }),
        )
    }

    pub fn error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ApiResponse<()>>) {
        (
            status,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(message.into()),
            }),
        )
    }
}

pub fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    ApiResponse::<()>::error(status, message).into_response()
}
