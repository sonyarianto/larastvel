use axum::{
    body::Body,
    extract::Request as AxumRequest,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

pub type Request = AxumRequest<Body>;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonResponse<T: Serialize> {
    pub data: T,
    pub message: Option<String>,
}

impl<T: Serialize> JsonResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            message: None,
        }
    }

    pub fn with_message(mut self, msg: &str) -> Self {
        self.message = Some(msg.to_string());
        self
    }
}

impl<T: Serialize> IntoResponse for JsonResponse<T> {
    fn into_response(self) -> Response {
        Json(serde_json::json!({
            "data": self.data,
            "message": self.message,
        }))
        .into_response()
    }
}

pub type LarastvelResult<T> = Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Validation Error: {0}")]
    Validation(String),
    #[error("Internal Error: {0}")]
    Internal(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Error::NotFound(msg) => (axum::http::StatusCode::NOT_FOUND, msg.clone()),
            Error::Validation(msg) => (axum::http::StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            Error::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
            Error::Unauthorized(msg) => (axum::http::StatusCode::UNAUTHORIZED, msg.clone()),
        };

        (status, Json(serde_json::json!({"error": message}))).into_response()
    }
}
