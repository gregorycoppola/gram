use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub struct AppError(pub anyhow::Error);

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": self.0.to_string(),
        }));
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
