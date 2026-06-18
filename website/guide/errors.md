# Error Handling

Larastvel uses Rust's `Result` types and Axum's error handling patterns.

## HTTP Errors

```rust
use larastvel_core::http::{Error as HttpError, LarastvelResult};

async fn handler() -> Result<Json<Value>, HttpError> {
    let user = find_user(id).ok_or(HttpError::not_found("User not found"))?;
    Ok(Json(user))
}
```

## Custom Error Responses

Implement `IntoResponse` for your error types:

```rust
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;

enum AppError {
    NotFound(String),
    Unauthorized,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".into()),
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}
```

## Validation Errors

Validation failures automatically return 422 responses with error details via the `IntoResponse` implementation on `ValidationErrors`.
