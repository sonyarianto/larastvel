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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_json_response_new() {
        let resp = JsonResponse::new("hello");
        assert_eq!(resp.data, "hello");
        assert!(resp.message.is_none());
    }

    #[test]
    fn test_json_response_with_message() {
        let resp = JsonResponse::new(42).with_message("the answer");
        assert_eq!(resp.data, 42);
        assert_eq!(resp.message, Some("the answer".to_string()));
    }

    #[test]
    fn test_error_not_found() {
        let err = Error::NotFound("route".to_string());
        assert_eq!(err.to_string(), "Not Found: route");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_error_validation() {
        let err = Error::Validation("bad input".to_string());
        assert_eq!(err.to_string(), "Validation Error: bad input");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn test_error_internal() {
        let err = Error::Internal("wow".to_string());
        assert_eq!(err.to_string(), "Internal Error: wow");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_error_unauthorized() {
        let err = Error::Unauthorized("nope".to_string());
        assert_eq!(err.to_string(), "Unauthorized: nope");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_larastvel_result_type() {
        let ok: LarastvelResult<i32> = Ok(1);
        assert!(ok.is_ok());
        let err: LarastvelResult<i32> = Err(Error::Internal("fail".to_string()));
        assert!(err.is_err());
    }
}
