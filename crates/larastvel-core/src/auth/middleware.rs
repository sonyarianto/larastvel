use axum::{
    extract::Request,
    middleware::Next,
    response::{Json, Response},
    http::StatusCode,
};
use serde_json::json;

use super::{Auth, DEFAULT_SECRET};

pub async fn auth_middleware(
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let token = Auth::extract_token_from_header(req.headers()).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Missing or invalid authorization header"})),
        )
    })?;

    let secret = req
        .extensions()
        .get::<Vec<u8>>()
        .cloned()
        .unwrap_or_else(|| DEFAULT_SECRET.as_bytes().to_vec());

    let auth = Auth::new(secret);
    let claims = auth.verify_token(&token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid or expired token"})),
        )
    })?;

    req.extensions_mut().insert(claims);
    req.extensions_mut().insert(auth);

    Ok(next.run(req).await)
}
