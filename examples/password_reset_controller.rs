//! # PasswordResetController Example
//!
//! A full forgot/reset password flow built on Axum, demonstrating how to
//! wire up `PasswordResetBroker` with request validation, JSON responses,
//! and HTML form rendering.
//!
//! ## Routes
//!
//! | Method | URI                        | Description                           |
//! |--------|----------------------------|---------------------------------------|
//! | GET    | `/password/forgot`         | Show forgot-password form             |
//! | POST   | `/password/forgot`         | Send reset-link email                 |
//! | GET    | `/password/reset/{token}`  | Show reset-password form (query: email)|
//! | POST   | `/password/reset`          | Execute the password reset            |
//!
//! ## Integration
//!
//! ```ignore
//! // In your main.rs or a service provider:
//! use larastvel_core::routing::Registrar;
//!
//! let router = app.router();
//! password_reset_controller::PasswordResetController::register_routes(&router);
//! ```
//!
//! The controller expects `PasswordResetBroker` to be available in Axum's
//! request extensions (inserted via middleware or a Tower layer).

#![allow(unused_imports, dead_code)]

use std::sync::Arc;

use larastvel_core::auth::{PasswordResetBroker, PasswordResetConfig, PasswordResetError};
use larastvel_core::axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    Form, Router,
};
use larastvel_core::mail::LogMailer;
use larastvel_core::rate_limiter::{RateLimitConfig, RateLimitExceeded, RateLimiter};
use larastvel_core::routing::Registrar;
use larastvel_core::sea_orm;
use larastvel_core::serde::{Deserialize, Serialize};
use larastvel_core::serde_json::{self, json};

// =============================================================================
// REQUEST / RESPONSE TYPES
// =============================================================================

/// Payload for the "forgot password" endpoint.
#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// Payload for the "reset password" endpoint.
#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub email: String,
    pub token: String,
    pub password: String,
    /// Optional: password confirmation (validated by the frontend or app).
    pub password_confirmation: Option<String>,
}

/// Query parameters for the GET reset form.
#[derive(Debug, Deserialize)]
pub struct ResetFormQuery {
    pub email: String,
}

/// Generic JSON envelope for success responses.
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// =============================================================================
// RATE LIMITER
// =============================================================================

/// Create a rate limiter for the forgot-password endpoint.
///
/// Limits to 3 requests per minute per email address, which prevents
/// brute-force token generation while remaining usable for legitimate users.
pub fn forgot_password_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimitConfig::per_minute(3).named("forgot-password"))
}

// =============================================================================
// CONTROLLER
// =============================================================================

/// Controller handling the full forgot/reset password flow.
///
/// All handlers extract `PasswordResetBroker` from Axum's request extensions.
/// The `POST /password/forgot` handler also expects a `RateLimiter` extension
/// to prevent abuse (3 attempts per minute per email).
///
/// Wire it up in your application boot (see `examples/auth_service_provider.rs`).
pub struct PasswordResetController;

impl PasswordResetController {
    /// Register all password-reset routes on the given `Registrar`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let router = app.router();
    /// PasswordResetController::register_routes(&router);
    /// ```
    ///
    /// # Required Extensions
    ///
    /// Make sure the following are available in the request extensions:
    /// - `PasswordResetBroker` — for sending reset links and performing resets
    /// - `RateLimiter` — for rate-limiting the `POST /password/forgot` endpoint
    ///
    /// ```ignore
    /// use axum::Extension;
    /// use password_reset_controller::forgot_password_rate_limiter;
    ///
    /// let broker = PasswordResetBroker::new(...);
    /// let rate_limiter = forgot_password_rate_limiter();
    ///
    /// Router::new()
    ///     .route(...)
    ///     .layer(Extension(broker))
    ///     .layer(Extension(rate_limiter));
    /// ```
    pub fn register_routes(registrar: &Registrar) {
        registrar.get("/password/forgot", Self::show_forgot_form);
        registrar.post("/password/forgot", Self::send_reset_link);
        registrar.get("/password/reset/{token}", Self::show_reset_form);
        registrar.post("/password/reset", Self::perform_reset);
    }

    // -------------------------------------------------------------------------
    // HANDLERS
    // -------------------------------------------------------------------------

    /// GET /password/forgot — display the forgot-password form.
    ///
    /// In a full-stack app, this would render a view/template. Here we return
    /// a simple HTML page with a form that posts to the same endpoint.
    pub async fn show_forgot_form() -> Response {
        Html(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Forgot Password</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
               background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%);
               min-height: 100vh; display: flex; align-items: center; justify-content: center; }
        .card { background: #1e293b; border: 1px solid #334155; border-radius: 12px;
                padding: 2.5rem; width: 100%; max-width: 420px; }
        h1 { color: #f1f5f9; font-size: 1.5rem; margin-bottom: 0.5rem; }
        p  { color: #94a3b8; font-size: 0.875rem; margin-bottom: 1.5rem; line-height: 1.5; }
        label { display: block; color: #cbd5e1; font-size: 0.875rem; font-weight: 600; margin-bottom: 0.375rem; }
        input { width: 100%; padding: 0.75rem; border: 1px solid #475569; border-radius: 6px;
                background: #0f172a; color: #e2e8f0; font-size: 0.9375rem; outline: none;
                transition: border-color 0.2s; }
        input:focus { border-color: #6366f1; }
        button { width: 100%; padding: 0.75rem; background: #6366f1; color: #fff;
                 border: none; border-radius: 6px; font-size: 0.9375rem; font-weight: 600;
                 cursor: pointer; transition: background 0.2s; margin-top: 0.25rem; }
        button:hover { background: #4f46e5; }
        .back { display: block; text-align: center; margin-top: 1rem;
                color: #64748b; font-size: 0.8125rem; text-decoration: none; }
        .back:hover { color: #94a3b8; }
    </style>
</head>
<body>
    <div class="card">
        <h1>🔐 Forgot Password</h1>
        <p>Enter your email address and we'll send you a link to reset your password.</p>
        <form action="/password/forgot" method="POST">
            <label for="email">Email Address</label>
            <input type="email" id="email" name="email" placeholder="you@example.com" required autofocus>
            <button type="submit">Send Reset Link</button>
        </form>
        <a href="/" class="back">← Back to home</a>
    </div>
</body>
</html>"#,
        )
        .into_response()
    }

    /// POST /password/forgot — validate the email and send a reset link.
    ///
    /// Always returns a success message to prevent email enumeration attacks
    /// (even if the email doesn't exist in the user table — the broker handles
    /// this transparently).
    ///
    /// ## Rate Limiting
    ///
    /// This endpoint is protected by a rate limiter (3 requests per minute per
    /// email). When exceeded, the handler returns HTTP 429 with a `Retry-After`
    /// header and a JSON error body.
    pub async fn send_reset_link(
        Extension(broker): Extension<PasswordResetBroker>,
        Extension(rate_limiter): Extension<RateLimiter>,
        Form(body): Form<ForgotPasswordRequest>,
    ) -> Response {
        // Validate email format
        let email = body.email.trim().to_lowercase();
        if !email.contains('@') || !email.contains('.') {
            return PasswordResetError::InvalidEmail.into_response();
        }

        // --- Rate limit check ---
        if rate_limiter.too_many_attempts(&email) {
            let retry_after = rate_limiter.retry_after(&email);
            return RateLimitExceeded {
                retry_after,
                limiter_name: "forgot-password".to_string(),
            }
            .into_response();
        }

        // Record the attempt (even if it fails, to prevent abuse)
        rate_limiter.hit(&email);

        // Attempt to send the reset link. The broker handles token storage,
        // throttling, and email dispatch.
        //
        // If the email doesn't correspond to a user, `send_reset_link` still
        // returns `Ok(())` (prevents enumeration) — but you can check with
        // the `update_password` callback if needed.
        match broker.send_reset_link(&email).await {
            Ok(()) => Json(json!({
                "message": "If that email address is registered, you will receive a password reset link shortly."
            }))
            .into_response(),
            Err(e) => e.into_response(),
        }
    }

    /// GET /password/reset/{token} — display the reset-password form.
    ///
    /// Expects `?email=...` as a query parameter so the user can reset the
    /// password for the correct account.
    pub async fn show_reset_form(
        Path(token): Path<String>,
        Query(query): Query<ResetFormQuery>,
    ) -> Response {
        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Reset Password</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
               background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%);
               min-height: 100vh; display: flex; align-items: center; justify-content: center; }}
        .card {{ background: #1e293b; border: 1px solid #334155; border-radius: 12px;
                padding: 2.5rem; width: 100%; max-width: 420px; }}
        h1 {{ color: #f1f5f9; font-size: 1.5rem; margin-bottom: 0.5rem; }}
        p  {{ color: #94a3b8; font-size: 0.875rem; margin-bottom: 1.5rem; line-height: 1.5; }}
        label {{ display: block; color: #cbd5e1; font-size: 0.875rem; font-weight: 600; margin-bottom: 0.375rem; }}
        input {{ width: 100%; padding: 0.75rem; border: 1px solid #475569; border-radius: 6px;
                background: #0f172a; color: #e2e8f0; font-size: 0.9375rem; outline: none;
                transition: border-color 0.2s; }}
        input:focus {{ border-color: #6366f1; }}
        button {{ width: 100%; padding: 0.75rem; background: #6366f1; color: #fff;
                 border: none; border-radius: 6px; font-size: 0.9375rem; font-weight: 600;
                 cursor: pointer; transition: background 0.2s; margin-top: 0.25rem; }}
        button:hover {{ background: #4f46e5; }}
        .back {{ display: block; text-align: center; margin-top: 1rem;
                color: #64748b; font-size: 0.8125rem; text-decoration: none; }}
        .back:hover {{ color: #94a3b8; }}
        .info {{ background: #312e81; color: #a5b4fc; padding: 0.75rem; border-radius: 6px;
                font-size: 0.8125rem; margin-bottom: 1.25rem; }}
    </style>
</head>
<body>
    <div class="card">
        <h1>🔑 Reset Password</h1>
        <div class="info">Resetting password for <strong>{0}</strong></div>
        <form action="/password/reset" method="POST">
            <input type="hidden" name="token" value="{1}">
            <input type="hidden" name="email" value="{0}">

            <label for="password">New Password</label>
            <input type="password" id="password" name="password"
                   placeholder="Enter your new password" required minlength="8">

            <label for="password_confirmation" style="margin-top: 1rem;">Confirm Password</label>
            <input type="password" id="password_confirmation" name="password_confirmation"
                   placeholder="Confirm your new password" required minlength="8">

            <button type="submit" style="margin-top: 1rem;">Reset Password</button>
        </form>
        <a href="/password/forgot" class="back">← Request a new link</a>
    </div>
</body>
</html>"#,
            query.email, token,
        );
        Html(html).into_response()
    }

    /// POST /password/reset — validate the token and update the user's password.
    ///
    /// The `PasswordResetBroker::reset()` method:
    /// 1. Looks up the token in the database
    /// 2. Checks expiration
    /// 3. Calls the `update_password` closure to persist the new hash
    /// 4. Deletes the used token
    ///
    /// You must provide the password-update closure that matches your app's
    /// user storage strategy (SeaORM, raw SQL, etc.).
    pub async fn perform_reset(
        Extension(broker): Extension<PasswordResetBroker>,
        Form(body): Form<ResetPasswordRequest>,
    ) -> Result<Json<serde_json::Value>, PasswordResetError> {
        let email = body.email.trim().to_lowercase();

        // Validate password confirmation
        if let Some(ref confirmation) = body.password_confirmation {
            if body.password != *confirmation {
                return Err(PasswordResetError::InvalidEmail); // Re-use variant for validation error
            }
        }

        if body.password.len() < 8 {
            return Err(PasswordResetError::InvalidEmail);
        }

        // Execute the reset. The closure runs *after* token validation, so
        // you can safely update the database knowing the token is legitimate.
        //
        // In a real app, replace this closure with your actual password-hashing
        // and database-update logic. For example:
        //
        // ```ignore
        // |email, password| {
        //     let hash = bcrypt::hash(password).map_err(|e| ...)?;
        //     // UPDATE users SET password = ?1 WHERE email = ?2
        //     db.execute(Statement::from_sql_and_values(...)).await
        //         .map_err(|e| PasswordResetError::Database(e.to_string()))?;
        //     Ok(())
        // }
        // ```
        broker
            .reset(&email, &body.token, &body.password, |email, _password| {
                // --- SIMULATED: replace with real DB logic ---
                tracing::info!(
                    "Password reset for {} — would update password hash here",
                    email,
                );
                Ok(())
            })
            .await?;

        Ok(Json(json!({
            "message": "Your password has been reset successfully. You can now log in with your new password.",
            "email": email,
        })))
    }
}

// =============================================================================
// ENTRY POINT
// =============================================================================

/// Run this example standalone (displays the controller API).
fn main() {
    println!("PasswordResetController example — see the source code for route handlers.");
    println!();
    println!("Routes:");
    println!("  GET  /password/forgot         — display forgot-password form");
    println!("  POST /password/forgot         — send reset-link email");
    println!("  GET  /password/reset/{{token}}  — display reset-password form (?email=...)");
    println!("  POST /password/reset          — execute password reset");
    println!();
    println!("To register routes:");
    println!("  use password_reset_controller::PasswordResetController;");
    println!("  PasswordResetController::register_routes(&router);");
    println!();
    println!("The controller expects PasswordResetBroker + RateLimiter in Axum extensions.");
    println!("Wire it up via Application::bind() or a Tower layer.");
    println!();
    println!("Rate limiting: POST /password/forgot is limited to 3 attempts/min per email.");
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use larastvel_core::axum::body::Body;
    use larastvel_core::axum::http::Request;
    use larastvel_core::axum::routing;
    use larastvel_core::axum::Router;
    use larastvel_core::sea_orm::ConnectionTrait;
    use tower::ServiceExt;

    /// Build a test router with the controller registered and both
    /// `PasswordResetBroker` and `RateLimiter` inserted into extensions.
    async fn test_router() -> Router {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let broker = PasswordResetBroker::new(
            db.clone(),
            PasswordResetConfig::default(),
            Arc::new(LogMailer::new("log")),
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
        );
        broker.ensure_table_exists().await.unwrap();

        let rate_limiter = forgot_password_rate_limiter();

        Router::new()
            .route(
                "/password/forgot",
                routing::get(PasswordResetController::show_forgot_form)
                    .post(PasswordResetController::send_reset_link),
            )
            .route(
                "/password/reset/{token}",
                routing::get(PasswordResetController::show_reset_form),
            )
            .route(
                "/password/reset",
                routing::post(PasswordResetController::perform_reset),
            )
            .layer(Extension(broker))
            .layer(Extension(rate_limiter))
    }

    /// Test that the rate limiter works correctly at the unit level.
    #[test]
    fn test_rate_limiter_direct() {
        let limiter = forgot_password_rate_limiter();

        assert!(!limiter.too_many_attempts("test@example.com"));
        assert_eq!(limiter.hit("test@example.com"), 1);
        assert!(!limiter.too_many_attempts("test@example.com"));
        assert_eq!(limiter.hit("test@example.com"), 2);
        assert!(!limiter.too_many_attempts("test@example.com"));
        assert_eq!(limiter.hit("test@example.com"), 3);
        assert!(limiter.too_many_attempts("test@example.com"));

        // Different email is not affected
        assert!(!limiter.too_many_attempts("other@example.com"));
    }

    /// Test that the handler accepts requests when rate limit is not exceeded.
    #[tokio::test]
    async fn test_send_reset_link_no_rate_limit() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect");
        let broker = PasswordResetBroker::new(
            db,
            PasswordResetConfig::default(),
            Arc::new(LogMailer::new("log")),
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
        );
        broker.ensure_table_exists().await.unwrap();

        let rate_limiter = forgot_password_rate_limiter();
        let req = Form(ForgotPasswordRequest {
            email: "fresh@example.com".to_string(),
        });
        let resp = PasswordResetController::send_reset_link(
            Extension(broker),
            Extension(rate_limiter),
            req,
        )
        .await;

        assert_eq!(
            resp.status(),
            200,
            "Fresh limiter should allow the first request"
        );
    }

    /// Test that the handler returns 429 when rate limit is exceeded.
    #[tokio::test]
    async fn test_send_reset_link_rate_limited() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect");
        let broker = PasswordResetBroker::new(
            db,
            PasswordResetConfig::default(),
            Arc::new(LogMailer::new("log")),
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
        );
        broker.ensure_table_exists().await.unwrap();

        // Pre-seed the rate limiter with 3 hits so the next one gets blocked
        let rate_limiter = forgot_password_rate_limiter();
        for _ in 0..3 {
            rate_limiter.hit("blocked@example.com");
        }

        let req = Form(ForgotPasswordRequest {
            email: "blocked@example.com".to_string(),
        });
        let resp = PasswordResetController::send_reset_link(
            Extension(broker),
            Extension(rate_limiter),
            req,
        )
        .await;

        assert_eq!(resp.status(), 429);
        assert!(resp.headers().get("Retry-After").is_some());
    }

    /// Test that rate limiting is per-email (not shared across emails).
    #[tokio::test]
    async fn test_rate_limiter_per_email() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect");
        let broker = PasswordResetBroker::new(
            db,
            PasswordResetConfig::default(),
            Arc::new(LogMailer::new("log")),
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
        );
        broker.ensure_table_exists().await.unwrap();

        let rate_limiter = forgot_password_rate_limiter();

        // Exhaust attempts for user-a
        for _ in 0..3 {
            rate_limiter.hit("user-a@example.com");
        }

        // user-b should still be allowed
        let req = Form(ForgotPasswordRequest {
            email: "user-b@example.com".to_string(),
        });
        let resp = PasswordResetController::send_reset_link(
            Extension(broker),
            Extension(rate_limiter),
            req,
        )
        .await;

        assert_eq!(
            resp.status(),
            200,
            "Different email should not be rate-limited"
        );
    }

    #[tokio::test]
    async fn test_show_forgot_form_returns_html() {
        let app = test_router().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/password/forgot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        assert!(content_type.contains("text/html"));
    }

    #[tokio::test]
    async fn test_send_reset_link_returns_success() {
        let app = test_router().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/password/forgot")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("email=user%40example.com"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_send_reset_link_invalid_email() {
        let app = test_router().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/password/forgot")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("email=notanemail"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    #[tokio::test]
    async fn test_show_reset_form_returns_html() {
        let app = test_router().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/password/reset/abc123?email=user%40example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        assert!(content_type.contains("text/html"));
    }

    #[tokio::test]
    async fn test_perform_reset_invalid_token() {
        let app = test_router().await;

        let body = format!(
            "email={}&token={}&password={}&password_confirmation={}",
            urlencoding("user@example.com"),
            "invalid-token",
            "new-secure-pass-123!",
            "new-secure-pass-123!",
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/password/reset")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Invalid token should return 400
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    async fn test_perform_reset_full_flow_send_and_check() {
        let app = test_router().await;

        // 1. Send a reset link
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/password/forgot")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("email=flow%40example.com"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // 2. Verify the send-reset-link endpoint returns the expected message
        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert!(json["message"]
            .as_str()
            .unwrap()
            .contains("If that email address is registered"));

        // Note: A full E2E test (send → peek token from DB → POST /password/reset)
        // requires sharing the DatabaseConnection between the router and test code.
        // See test_perform_reset_e2e_with_shared_db for the full flow.
    }

    /// Integration-style test that constructs a shared DB and verifies the
    /// full send→peek→reset flow end-to-end.
    #[tokio::test]
    async fn test_perform_reset_e2e_with_shared_db() {
        // Create a shared in-memory database for both the router and test code
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let broker = PasswordResetBroker::new(
            db.clone(),
            PasswordResetConfig::default(),
            Arc::new(LogMailer::new("log")),
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
        );
        broker.ensure_table_exists().await.unwrap();

        let rate_limiter = forgot_password_rate_limiter();

        let router = Router::new()
            .route(
                "/password/forgot",
                routing::get(PasswordResetController::show_forgot_form)
                    .post(PasswordResetController::send_reset_link),
            )
            .route(
                "/password/reset/{token}",
                routing::get(PasswordResetController::show_reset_form),
            )
            .route(
                "/password/reset",
                routing::post(PasswordResetController::perform_reset),
            )
            .layer(Extension(broker.clone()))
            .layer(Extension(rate_limiter));

        // 1. Send a reset link
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/password/forgot")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("email=e2e%40example.com"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // 2. Peek at the token table (simulating the user checking their email)
        let sql = "SELECT token FROM password_reset_tokens WHERE email = ?1";
        let row = db
            .query_one(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                sql,
                ["e2e@example.com".into()],
            ))
            .await
            .unwrap()
            .expect("Token should exist after send_reset_link");
        let token: String = row.try_get_by_index(0).unwrap();

        // 3. Use the token to reset the password
        let body = format!(
            "email={}&token={}&password={}&password_confirmation={}",
            urlencoding("e2e@example.com"),
            token,
            "NewSecurePass123!",
            "NewSecurePass123!",
        );

        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/password/reset")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        // 4. Verify the token was deleted
        let row = db
            .query_one(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                sql,
                ["e2e@example.com".into()],
            ))
            .await
            .unwrap();
        assert!(
            row.is_none(),
            "Token should be deleted after successful reset"
        );
    }

    #[tokio::test]
    async fn test_perform_reset_password_mismatch() {
        let app = test_router().await;

        let body = format!(
            "email={}&token={}&password={}&password_confirmation={}",
            urlencoding("user@example.com"),
            "some-token",
            "password-one",
            "password-two", // mismatch
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/password/reset")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    #[tokio::test]
    async fn test_perform_reset_password_too_short() {
        let app = test_router().await;

        let body = format!(
            "email={}&token={}&password={}&password_confirmation={}",
            urlencoding("user@example.com"),
            "some-token",
            "short",
            "short",
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/password/reset")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    #[tokio::test]
    async fn test_forgot_form_contains_form_elements() {
        let app = test_router().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/password/forgot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 32_768)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&body_bytes);

        // The form should post to /password/forgot
        assert!(html.contains(r#"action="/password/forgot""#));
        assert!(html.contains(r#"name="email""#));
        assert!(html.contains(r#"type="email""#));
    }

    #[tokio::test]
    async fn test_reset_form_contains_hidden_fields() {
        let app = test_router().await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/password/reset/my-token-123?email=user%40example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 32_768)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&body_bytes);

        assert!(html.contains(r#"name="token""#));
        assert!(html.contains(r#"name="email""#));
        assert!(html.contains(r#"action="/password/reset""#));
    }
}

/// Minimal URL encoder for email addresses (used in tests).
fn urlencoding(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b'@' => result.push_str("%40"),
            b'!' => result.push_str("%21"),
            b'#' => result.push_str("%23"),
            b'$' => result.push_str("%24"),
            b'%' => result.push_str("%25"),
            b'^' => result.push_str("%5E"),
            b'&' => result.push_str("%26"),
            b'*' => result.push_str("%2A"),
            b'(' => result.push_str("%28"),
            b')' => result.push_str("%29"),
            b'+' => result.push_str("%2B"),
            b'=' => result.push_str("%3D"),
            b'{' => result.push_str("%7B"),
            b'}' => result.push_str("%7D"),
            b'|' => result.push_str("%7C"),
            b'\\' => result.push_str("%5C"),
            b':' => result.push_str("%3A"),
            b';' => result.push_str("%3B"),
            b'"' => result.push_str("%22"),
            b'\'' => result.push_str("%27"),
            b'/' => result.push_str("%2F"),
            b'?' => result.push_str("%3F"),
            b'>' => result.push_str("%3E"),
            b'<' => result.push_str("%3C"),
            b',' => result.push_str("%2C"),
            b'`' => result.push_str("%60"),
            b' ' => result.push_str("%20"),
            other => {
                result.push_str(&format!("%{:02X}", other));
            }
        }
    }
    result
}
