use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::mail::{Mailable, Mailer};

/// Error type for email verification operations.
#[derive(Debug, Error)]
pub enum EmailVerificationError {
    #[error("User ID mismatch")]
    UserIdMismatch,
    #[error("Invalid or expired verification token")]
    InvalidToken,
    #[error("Token has expired")]
    TokenExpired,
    #[error("Mail error: {0}")]
    Mail(String),
    #[error("Email already verified")]
    AlreadyVerified,
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Email not verified")]
    NotVerified,
    #[error("Token error: {0}")]
    Token(String),
}

impl IntoResponse for EmailVerificationError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            EmailVerificationError::UserIdMismatch => (StatusCode::BAD_REQUEST, "User ID mismatch"),
            EmailVerificationError::InvalidToken => (
                StatusCode::BAD_REQUEST,
                "Invalid or expired verification token",
            ),
            EmailVerificationError::TokenExpired => {
                (StatusCode::BAD_REQUEST, "Verification token has expired")
            }
            EmailVerificationError::Mail(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send verification email",
            ),
            EmailVerificationError::AlreadyVerified => {
                (StatusCode::BAD_REQUEST, "Email already verified")
            }
            EmailVerificationError::NotAuthenticated => {
                (StatusCode::UNAUTHORIZED, "Not authenticated")
            }
            EmailVerificationError::NotVerified => (StatusCode::FORBIDDEN, "Email not verified"),
            EmailVerificationError::Token(_) => (StatusCode::BAD_REQUEST, "Invalid token"),
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}

/// Claims embedded in the email verification JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VerificationClaims {
    /// User ID.
    sub: String,
    /// Email address being verified.
    email: String,
    /// Purpose — always "email_verification".
    purpose: String,
    /// Expiry timestamp.
    exp: usize,
    /// Issued-at timestamp.
    iat: usize,
}

/// A verified user — extracted after successful email verification check.
///
/// Use this as an extractor in handler functions to require a verified email:
///
/// ```ignore
/// async fn dashboard(user: VerifiedUser) -> Json<Value> {
///     json!({ "user_id": user.user_id })
/// }
/// ```
#[derive(Debug, Clone)]
pub struct VerifiedUser {
    pub user_id: String,
}

/// Check whether a user's email is verified.
///
/// The application provides this closure at startup so the verification
/// middleware can query the users table however it needs to.
pub type VerificationChecker = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// Mark a user's email as verified in the database.
///
/// Called after a verification token is confirmed. The application
/// provides this closure at startup.
pub type MarkVerifiedCallback =
    Arc<dyn Fn(&str) -> Result<(), EmailVerificationError> + Send + Sync>;

/// Manages email verification — sending verification emails and checking status.
#[derive(Clone)]
pub struct EmailVerificationBroker {
    secret: Vec<u8>,
    mailer: Arc<dyn Mailer>,
    from_address: String,
    app_url: String,
    app_name: String,
    check_verified: VerificationChecker,
    mark_verified: MarkVerifiedCallback,
    token_expiry_seconds: u64,
}

impl std::fmt::Debug for EmailVerificationBroker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailVerificationBroker")
            .field("from_address", &self.from_address)
            .field("app_url", &self.app_url)
            .field("app_name", &self.app_name)
            .field("token_expiry_seconds", &self.token_expiry_seconds)
            .field("mailer", &"<dyn Mailer>")
            .field("check_verified", &"<closure>")
            .finish()
    }
}

impl EmailVerificationBroker {
    /// Create a new email verification broker.
    ///
    /// - `secret`: JWT signing secret (use the app's key).
    /// - `mailer`: The mailer to send verification emails.
    /// - `from_address`: The "From" address for verification emails.
    /// - `app_url`: Base URL of the application (e.g. `http://localhost:8080`).
    /// - `app_name`: Application name (used in email subject/body).
    /// - `check_verified`: Async closure that takes a `user_id` and returns whether the email is verified.
    /// - `token_expiry_seconds`: How long the verification token is valid (default: 60 min).
    /// - `mark_verified`: Callback that marks a user's email as verified in the database.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        secret: &[u8],
        mailer: Arc<dyn Mailer>,
        from_address: &str,
        app_url: &str,
        app_name: &str,
        check_verified: VerificationChecker,
        mark_verified: MarkVerifiedCallback,
        token_expiry_seconds: u64,
    ) -> Self {
        Self {
            secret: secret.to_vec(),
            mailer,
            from_address: from_address.to_string(),
            app_url: app_url.to_string(),
            app_name: app_name.to_string(),
            check_verified,
            mark_verified,
            token_expiry_seconds,
        }
    }

    fn now() -> usize {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize
    }

    /// Create a signed verification token (JWT) for the given user.
    fn create_verification_token(
        &self,
        user_id: &str,
        email: &str,
    ) -> Result<String, EmailVerificationError> {
        let now = Self::now();
        let claims = VerificationClaims {
            sub: user_id.to_string(),
            email: email.to_string(),
            purpose: "email_verification".to_string(),
            exp: now + self.token_expiry_seconds as usize,
            iat: now,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&self.secret),
        )
        .map_err(|e| EmailVerificationError::Token(e.to_string()))
    }

    /// Verify a verification token and return the user_id and email.
    pub fn verify_token(&self, token: &str) -> Result<(String, String), EmailVerificationError> {
        let token_data = decode::<VerificationClaims>(
            token,
            &DecodingKey::from_secret(&self.secret),
            &Validation::default(),
        )
        .map_err(|_| EmailVerificationError::InvalidToken)?;

        let claims = token_data.claims;

        if claims.purpose != "email_verification" {
            return Err(EmailVerificationError::InvalidToken);
        }

        let now = Self::now();
        if claims.exp <= now {
            return Err(EmailVerificationError::TokenExpired);
        }

        Ok((claims.sub, claims.email))
    }

    /// Send a verification email to the user.
    ///
    /// The generated verification link is:
    /// `{app_url}/email/verify/{user_id}?token={jwt_token}`
    pub async fn send_verification_email(
        &self,
        user_id: &str,
        email: &str,
    ) -> Result<(), EmailVerificationError> {
        // Don't send if already verified
        if (self.check_verified)(user_id) {
            return Err(EmailVerificationError::AlreadyVerified);
        }

        let token = self.create_verification_token(user_id, email)?;

        let verify_url = format!(
            "{}/email/verify/{}?token={}",
            self.app_url.trim_end_matches('/'),
            user_id,
            token,
        );

        let subject = format!("{} - Verify Email Address", self.app_name);
        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head><meta charset="UTF-8"></head>
<body style="font-family: Arial, Helvetica, sans-serif; line-height: 1.6; color: #1a1a2e; margin: 0; padding: 0;">
    <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
        <div style="background-color: #059669; padding: 30px; border-radius: 8px 8px 0 0; text-align: center;">
            <h1 style="color: #ffffff; margin: 0; font-size: 24px;">Verify Email</h1>
        </div>
        <div style="background-color: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px;">
            <p style="font-size: 16px;">Hello!</p>
            <p style="font-size: 16px;">Please click the button below to verify your email address.</p>
            <p style="text-align: center; margin: 30px 0;">
                <a href="{}"
                   style="display: inline-block; padding: 14px 32px; background-color: #059669;
                          color: #ffffff; text-decoration: none; border-radius: 6px;
                          font-weight: bold; font-size: 16px;">
                    Verify Email Address
                </a>
            </p>
            <p style="font-size: 14px; color: #6b7280;">This verification link will expire in {} minutes.</p>
            <p style="font-size: 14px; color: #6b7280;">If you did not create an account, no further action is required.</p>
            <hr style="border: none; border-top: 1px solid #e5e7eb; margin: 24px 0;">
            <p style="font-size: 14px; color: #6b7280;">Regards,<br>{} Team</p>
        </div>
    </div>
</body>
</html>"#,
            verify_url,
            self.token_expiry_seconds / 60,
            self.app_name,
        );

        let mailable =
            Mailable::html(vec![email.to_string()], &subject, &html_body).from(&self.from_address);

        self.mailer
            .send(mailable)
            .await
            .map_err(|e| EmailVerificationError::Mail(e.to_string()))?;

        tracing::debug!(
            target: "larastvel::auth::email_verification",
            "Verification email sent to {} for user {}",
            email,
            user_id,
        );

        Ok(())
    }

    /// Check if the given user has a verified email.
    pub fn is_verified(&self, user_id: &str) -> bool {
        (self.check_verified)(user_id)
    }

    /// Mark a user's email as verified in the database.
    ///
    /// This calls the `mark_verified` callback provided at construction.
    /// Typically invoked after the verification token is confirmed.
    pub fn mark_verified(&self, user_id: &str) -> Result<(), EmailVerificationError> {
        (self.mark_verified)(user_id)
    }

    /// Resend the verification email (convenience wrapper).
    pub async fn resend_verification_email(
        &self,
        user_id: &str,
        email: &str,
    ) -> Result<(), EmailVerificationError> {
        self.send_verification_email(user_id, email).await
    }

    /// Verify a token and confirm the token's user_id matches the expected user.
    pub fn confirm_verification(
        &self,
        token: &str,
        expected_user_id: &str,
    ) -> Result<(String, String), EmailVerificationError> {
        let (user_id, email) = self.verify_token(token)?;
        if user_id != expected_user_id {
            return Err(EmailVerificationError::UserIdMismatch);
        }
        Ok((user_id, email))
    }
}

/// Axum middleware that rejects requests from unverified users.
///
/// Requires `AuthenticatedUser` and `EmailVerificationBroker` to be present
/// in request extensions (inserted by upstream middleware).
///
/// # Example
///
/// ```ignore
/// use axum::{Router, middleware};
/// use larastvel_core::auth::require_verified_email;
///
/// let app = Router::new()
///     .route("/dashboard", get(dashboard))
///     .route_layer(middleware::from_fn(require_verified_email));
/// ```
pub async fn require_verified_email(
    req: Request,
    next: Next,
) -> Result<Response, EmailVerificationError> {
    let user = req
        .extensions()
        .get::<crate::auth::AuthenticatedUser>()
        .cloned()
        .ok_or(EmailVerificationError::NotAuthenticated)?;

    let broker = req
        .extensions()
        .get::<EmailVerificationBroker>()
        .cloned()
        .ok_or_else(|| {
            EmailVerificationError::Token(
                "EmailVerificationBroker not initialized in extensions".to_string(),
            )
        })?;

    if !broker.is_verified(&user.user_id) {
        return Err(EmailVerificationError::NotVerified);
    }

    Ok(next.run(req).await)
}

/// Axum extractor for verified users.
///
/// Requires `AuthenticatedUser` and `EmailVerificationBroker` to be present
/// in request extensions.
///
/// # Example
///
/// ```ignore
/// async fn dashboard(user: VerifiedUser) -> Json<Value> {
///     json!({ "user_id": user.user_id, "email": user.email })
/// }
/// ```
impl<S> FromRequestParts<S> for VerifiedUser
where
    S: Send + Sync,
{
    type Rejection = EmailVerificationError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user = parts
            .extensions
            .get::<crate::auth::AuthenticatedUser>()
            .cloned()
            .ok_or(EmailVerificationError::NotAuthenticated)?;

        let broker = parts
            .extensions
            .get::<EmailVerificationBroker>()
            .cloned()
            .ok_or_else(|| {
                EmailVerificationError::Token(
                    "EmailVerificationBroker not initialized in extensions".to_string(),
                )
            })?;

        if !broker.is_verified(&user.user_id) {
            return Err(EmailVerificationError::NotVerified);
        }

        Ok(VerifiedUser {
            user_id: user.user_id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthenticatedUser;
    use crate::auth::Claims;
    use crate::mail::LogMailer;
    use axum::body::Body;
    use std::sync::Arc;

    fn test_secret() -> Vec<u8> {
        b"test-verification-secret-key".to_vec()
    }

    fn verified_checker(verified: bool) -> VerificationChecker {
        let v = std::sync::atomic::AtomicBool::new(verified);
        Arc::new(move |_uid: &str| v.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn setup_broker(verified: bool) -> EmailVerificationBroker {
        let mailer = Arc::new(LogMailer::new("log"));
        let mark_verified: MarkVerifiedCallback = Arc::new(|_uid: &str| Ok(()));
        EmailVerificationBroker::new(
            &test_secret(),
            mailer,
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
            verified_checker(verified),
            mark_verified,
            3600,
        )
    }

    #[tokio::test]
    async fn test_create_and_verify_token() {
        let broker = setup_broker(false);
        let token = broker
            .create_verification_token("user-42", "user@example.com")
            .unwrap();

        let (user_id, email) = broker.verify_token(&token).unwrap();
        assert_eq!(user_id, "user-42");
        assert_eq!(email, "user@example.com");
    }

    #[tokio::test]
    async fn test_verify_invalid_token() {
        let broker = setup_broker(false);
        let result = broker.verify_token("invalid-token");
        assert!(result.is_err());
        match result {
            Err(EmailVerificationError::InvalidToken) => {}
            _ => panic!("Expected InvalidToken error"),
        }
    }

    #[tokio::test]
    async fn test_confirm_verification_matches_user_id() {
        let broker = setup_broker(false);
        let token = broker
            .create_verification_token("user-42", "user@example.com")
            .unwrap();

        let result = broker.confirm_verification(&token, "user-42");
        assert!(result.is_ok());

        let result = broker.confirm_verification(&token, "user-99");
        assert!(result.is_err());
        match result {
            Err(EmailVerificationError::UserIdMismatch) => {}
            _ => panic!("Expected UserIdMismatch error"),
        }
    }

    #[tokio::test]
    async fn test_is_verified() {
        let broker_verified = setup_broker(true);
        assert!(broker_verified.is_verified("user-1"));

        let broker_unverified = setup_broker(false);
        assert!(!broker_unverified.is_verified("user-1"));
    }

    #[tokio::test]
    async fn test_send_verification_email_to_unverified_user() {
        let broker = setup_broker(false);
        let result = broker
            .send_verification_email("user-42", "unverified@example.com")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_verification_email_to_verified_user_fails() {
        let broker = setup_broker(true);
        let result = broker
            .send_verification_email("user-42", "verified@example.com")
            .await;
        assert!(result.is_err());
        match result {
            Err(EmailVerificationError::AlreadyVerified) => {}
            _ => panic!("Expected AlreadyVerified error"),
        }
    }

    #[tokio::test]
    async fn test_resend_verification_email() {
        let broker = setup_broker(false);
        let result = broker
            .resend_verification_email("user-42", "resend@example.com")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verified_user_extractor_accepts_verified() {
        let broker = setup_broker(true);

        let (mut parts, _) = axum::http::Request::new(Body::empty()).into_parts();
        parts.extensions.insert(AuthenticatedUser {
            user_id: "user-42".to_string(),
            claims: Claims {
                sub: "user-42".to_string(),
                exp: 9999999999,
                iat: 0,
            },
        });
        parts.extensions.insert(broker);

        let result = VerifiedUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().user_id, "user-42");
    }

    #[tokio::test]
    async fn test_verified_user_extractor_rejects_unverified() {
        let broker = setup_broker(false);

        let (mut parts, _) = axum::http::Request::new(Body::empty()).into_parts();
        parts.extensions.insert(AuthenticatedUser {
            user_id: "user-42".to_string(),
            claims: Claims {
                sub: "user-42".to_string(),
                exp: 9999999999,
                iat: 0,
            },
        });
        parts.extensions.insert(broker);

        let result = VerifiedUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
        match result {
            Err(EmailVerificationError::NotVerified) => {}
            _ => panic!("Expected NotVerified error"),
        }
    }

    #[tokio::test]
    async fn test_verified_user_extractor_rejects_no_auth() {
        let broker = setup_broker(true);

        let (mut parts, _) = axum::http::Request::new(Body::empty()).into_parts();
        parts.extensions.insert(broker);

        let result = VerifiedUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
        match result {
            Err(EmailVerificationError::NotAuthenticated) => {}
            _ => panic!("Expected NotAuthenticated error"),
        }
    }

    #[tokio::test]
    async fn test_verified_user_extractor_rejects_no_broker() {
        let (mut parts, _) = axum::http::Request::new(Body::empty()).into_parts();
        parts.extensions.insert(AuthenticatedUser {
            user_id: "user-42".to_string(),
            claims: Claims {
                sub: "user-42".to_string(),
                exp: 9999999999,
                iat: 0,
            },
        });

        let result = VerifiedUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_token_purpose_check() {
        use jsonwebtoken::{encode, EncodingKey, Header};
        let now = EmailVerificationBroker::now();

        // Create a token with wrong purpose
        let claims = serde_json::json!({
            "sub": "user-42",
            "email": "user@example.com",
            "purpose": "password_reset",  // wrong purpose
            "exp": now + 3600,
            "iat": now,
        });

        let bad_token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&test_secret()),
        )
        .unwrap();

        let broker = setup_broker(false);
        let result = broker.verify_token(&bad_token);
        assert!(result.is_err());
        match result {
            Err(EmailVerificationError::InvalidToken) => {}
            _ => panic!("Expected InvalidToken error for wrong purpose"),
        }
    }

    #[tokio::test]
    async fn test_token_expiry() {
        let broker = EmailVerificationBroker::new(
            &test_secret(),
            Arc::new(LogMailer::new("log")),
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
            verified_checker(false),
            Arc::new(|_uid: &str| Ok(())),
            0, // Expire immediately
        );

        // Create a token that expired 1 second ago
        let claims = VerificationClaims {
            sub: "user-42".to_string(),
            email: "user@example.com".to_string(),
            purpose: "email_verification".to_string(),
            exp: EmailVerificationBroker::now() - 1,
            iat: EmailVerificationBroker::now() - 1000,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&test_secret()),
        )
        .unwrap();

        let result = broker.verify_token(&token);
        assert!(result.is_err());
        match result {
            Err(EmailVerificationError::TokenExpired) => {}
            _ => panic!("Expected TokenExpired error"),
        }
    }

    #[tokio::test]
    async fn test_error_into_response() {
        let err = EmailVerificationError::NotVerified;
        let resp = err.into_response();
        assert_eq!(resp.status(), 403);

        let err = EmailVerificationError::NotAuthenticated;
        let resp = err.into_response();
        assert_eq!(resp.status(), 401);

        let err = EmailVerificationError::AlreadyVerified;
        let resp = err.into_response();
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    async fn test_token_generation_uniqueness() {
        let broker = setup_broker(false);
        let t1 = broker
            .create_verification_token("user-1", "a@b.com")
            .unwrap();
        let t2 = broker
            .create_verification_token("user-2", "c@d.com")
            .unwrap();
        assert_ne!(t1, t2);
    }

    #[tokio::test]
    async fn test_mark_verified() {
        let marked = Arc::new(std::sync::Mutex::new(false));
        let m = marked.clone();
        let mark_verified: MarkVerifiedCallback = Arc::new(move |uid: &str| {
            assert_eq!(uid, "user-42");
            let mut m = m.lock().unwrap();
            *m = true;
            Ok(())
        });

        let mailer = Arc::new(LogMailer::new("log"));
        let broker = EmailVerificationBroker::new(
            &test_secret(),
            mailer,
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
            verified_checker(false),
            mark_verified,
            3600,
        );

        broker.mark_verified("user-42").unwrap();
        assert!(*marked.lock().unwrap());
    }
}
