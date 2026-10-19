use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Errors returned by the JSON API. Messages are stable codes, never user text:
/// the client translates them from its i18n files.
#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden(&'static str),
    NotFound,
    BadRequest(&'static str),
    TooManyRequests,
    Internal(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden(code) => (StatusCode::FORBIDDEN, code),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::BadRequest(code) => (StatusCode::BAD_REQUEST, code),
            Self::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "too_many_requests"),
            Self::Internal(e) => {
                tracing::error!(error = %format!("{e:#}"), "request failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        (status, Json(json!({ "error": code }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(e.into())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e)
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::Internal(e.into())
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;
