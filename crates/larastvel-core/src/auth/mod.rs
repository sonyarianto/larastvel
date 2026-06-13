mod jwt;
mod middleware;

pub use jwt::Claims;
pub use middleware::auth_middleware;

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

pub(crate) const DEFAULT_SECRET: &str = "larastvel-default-key-change-in-production";

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("User not found")]
    UserNotFound,
    #[error("Token error: {0}")]
    TokenError(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
            AuthError::UserNotFound => (StatusCode::NOT_FOUND, "User not found"),
            AuthError::TokenError(_) => (StatusCode::UNAUTHORIZED, "Invalid token"),
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}

pub fn extract_secret_from_config(config: &crate::config::Config) -> Vec<u8> {
    config
        .app
        .key
        .as_deref()
        .unwrap_or(DEFAULT_SECRET)
        .as_bytes()
        .to_vec()
}

#[derive(Clone)]
pub struct Auth {
    secret: Vec<u8>,
}

impl Auth {
    pub fn new(secret: Vec<u8>) -> Self {
        Self { secret }
    }

    pub fn default() -> Self {
        Self::new(DEFAULT_SECRET.as_bytes().to_vec())
    }

    pub fn from_config(config: &crate::config::Config) -> Self {
        Self::new(extract_secret_from_config(config))
    }

    pub fn create_token(&self, user_id: &str) -> Result<String, AuthError> {
        jwt::create_token(user_id, &self.secret)
            .map_err(|e| AuthError::TokenError(e.to_string()))
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        jwt::verify_token(token, &self.secret)
            .map_err(|e| AuthError::TokenError(e.to_string()))
    }

    pub fn extract_token_from_header(headers: &HeaderMap) -> Option<String> {
        headers
            .get("Authorization")?
            .to_str()
            .ok()?
            .strip_prefix("Bearer ")
            .map(|s| s.to_string())
    }
}

pub struct AuthenticatedUser {
    pub user_id: String,
    pub claims: Claims,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = Auth::extract_token_from_header(&parts.headers)
            .ok_or(AuthError::InvalidCredentials)?;

        let secret = parts
            .extensions
            .get::<Vec<u8>>()
            .cloned()
            .unwrap_or_else(|| {
                b"larastvel-default-key-change-in-production".to_vec()
            });

        let auth = Auth::new(secret);
        let claims = auth.verify_token(&token)?;

        Ok(AuthenticatedUser {
            user_id: claims.sub.clone(),
            claims,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_default() {
        let auth = Auth::default();
        assert!(!auth.secret.is_empty());
    }

    #[test]
    fn test_auth_create_and_verify_token() {
        let auth = Auth::default();
        let token = auth.create_token("user-42").unwrap();
        assert!(!token.is_empty());

        let claims = auth.verify_token(&token).unwrap();
        assert_eq!(claims.sub, "user-42");
    }

    #[test]
    fn test_auth_verify_invalid_token() {
        let auth = Auth::default();
        let result = auth.verify_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_auth_different_secrets() {
        let auth1 = Auth::new(b"secret1".to_vec());
        let auth2 = Auth::new(b"secret2".to_vec());

        let token = auth1.create_token("user-1").unwrap();
        let result = auth2.verify_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_token_from_header() {
        let mut headers = HeaderMap::new();
        assert!(Auth::extract_token_from_header(&headers).is_none());

        headers.insert("Authorization", "Bearer my-token".parse().unwrap());
        assert_eq!(
            Auth::extract_token_from_header(&headers).unwrap(),
            "my-token"
        );
    }

    #[test]
    fn test_extract_token_from_header_missing_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Basic dXNlcjpwYXNz".parse().unwrap());
        assert!(Auth::extract_token_from_header(&headers).is_none());
    }

    #[test]
    fn test_auth_token_expiry() {
        let auth = Auth::default();
        let token = auth.create_token("user-42").unwrap();
        let claims = auth.verify_token(&token).unwrap();

        assert!(claims.iat > 0);
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_authenticated_user_into_response() {
        let err = AuthError::InvalidCredentials;
        let resp = err.into_response();
        assert_eq!(resp.status(), 401);

        let err = AuthError::UserNotFound;
        let resp = err.into_response();
        assert_eq!(resp.status(), 404);

        let err = AuthError::TokenError("bad".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), 401);
    }
}
