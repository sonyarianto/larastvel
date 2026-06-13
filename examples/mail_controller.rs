//! # MailController Example
//!
//! Demonstrates how to send transactional emails using Larastvel's mail system.
//! Includes styled HTML templates for common transactional emails (welcome,
//! order confirmation, password changed) and a generic send endpoint.
//!
//! ## Routes
//!
//! | Method | URI                  | Description                            |
//! |--------|----------------------|----------------------------------------|
//! | GET    | `/mail/test`         | Show the mail-testing dashboard        |
//! | POST   | `/mail/send`         | Send a custom transactional email      |
//! | POST   | `/mail/welcome`      | Send a welcome email (pre-built HTML)  |
//! | POST   | `/mail/receipt`      | Send an order receipt (pre-built HTML) |
//!
//! ## Integration
//!
//! ```ignore
//! use larastvel_core::routing::Registrar;
//!
//! let router = app.router();
//! mail_controller::MailController::register_routes(&router);
//! ```
//!
//! The controller expects a `MailManager` in Axum's extensions.
//!
//! ## Quick Start
//!
//! ```ignore
//! use std::sync::Arc;
//! use larastvel_core::mail::{LogMailer, MailManager};
//! use axum::{Router, Extension};
//!
//! let mut mail_manager = MailManager::new("log");
//! mail_manager.register("log", LogMailer::new("log"));
//!
//! let router = Router::new()
//!     .route("/mail/send", axum::routing::post(mail_controller::MailController::send_email))
//!     .layer(Extension(mail_manager));
//! ```

#![allow(unused_imports, dead_code)]

use std::sync::Arc;

use larastvel_core::axum::{
    extract::Extension,
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    Form, Router,
};
use larastvel_core::mail::{LogMailer, MailError, MailManager, Mailer, Mailable};
use larastvel_core::rate_limiter::{RateLimitConfig, RateLimitExceeded, RateLimiter};
use larastvel_core::routing::Registrar;
use larastvel_core::serde::Deserialize;
use larastvel_core::serde_json::{self, json};

// =============================================================================
// REQUEST TYPES
// =============================================================================

/// Payload for the generic send endpoint.
#[derive(Debug, Deserialize)]
pub struct SendEmailRequest {
    pub to: String,
    pub subject: String,
    pub body: String,
    /// Pass `"html"` to send as HTML, anything else for plain text.
    #[serde(default = "default_content_type")]
    pub content_type: String,
    /// Optional CC addresses (comma-separated).
    pub cc: Option<String>,
    /// Optional BCC addresses (comma-separated).
    pub bcc: Option<String>,
}

fn default_content_type() -> String {
    "html".to_string()
}

/// Payload for the welcome email endpoint.
#[derive(Debug, Deserialize)]
pub struct WelcomeEmailRequest {
    pub name: String,
    pub email: String,
}

/// Payload for the order receipt endpoint.
#[derive(Debug, Deserialize)]
pub struct ReceiptEmailRequest {
    pub name: String,
    pub email: String,
    pub order_id: String,
    pub amount: String,
}

// =============================================================================
// RATE LIMITER
// =============================================================================

/// Create a rate limiter for the mail-send endpoints.
///
/// Limits to 10 emails per minute per sender to prevent abuse.
pub fn mail_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimitConfig::per_minute(10).named("mail-send"))
}

// =============================================================================
// HTML EMAIL TEMPLATES
// =============================================================================

/// Styled HTML template for a welcome email.
fn welcome_email_html(name: &str, app_name: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #1a1a2e; margin: 0; padding: 0; background-color: #f8fafc;">
    <div style="max-width: 600px; margin: 40px auto; padding: 0;">
        <div style="background: linear-gradient(135deg, #6366f1, #8b5cf6); padding: 40px 30px; text-align: center; border-radius: 12px 12px 0 0;">
            <div style="font-size: 48px; margin-bottom: 8px;">🎉</div>
            <h1 style="color: #ffffff; margin: 0; font-size: 28px;">Welcome to {0}!</h1>
        </div>
        <div style="background: #ffffff; padding: 36px 30px; border: 1px solid #e2e8f0; border-top: none; border-radius: 0 0 12px 12px;">
            <p style="font-size: 16px; color: #334155;">Hi <strong>{1}</strong>,</p>
            <p style="font-size: 16px; color: #475569;">Thanks for joining {0}! We're thrilled to have you on board.</p>
            <div style="background: #f0fdf4; border: 1px solid #bbf7d0; border-radius: 8px; padding: 20px; margin: 24px 0;">
                <p style="margin: 0 0 8px; color: #166534; font-weight: 600;">✨ Here's what you can do next:</p>
                <ul style="margin: 0; padding-left: 20px; color: #166534;">
                    <li>Complete your profile</li>
                    <li>Explore the dashboard</li>
                    <li>Invite your team members</li>
                </ul>
            </div>
            <p style="text-align: center; margin: 30px 0;">
                <a href="{2}/dashboard"
                   style="display: inline-block; padding: 14px 36px; background: linear-gradient(135deg, #6366f1, #8b5cf6);
                          color: #ffffff; text-decoration: none; border-radius: 8px;
                          font-weight: 600; font-size: 16px;">
                    Go to Dashboard →
                </a>
            </p>
            <p style="font-size: 14px; color: #94a3b8;">If you didn't create an account, you can safely ignore this email.</p>
            <hr style="border: none; border-top: 1px solid #e2e8f0; margin: 24px 0;">
            <p style="font-size: 14px; color: #94a3b8;">Regards,<br>The {0} Team</p>
        </div>
    </div>
</body>
</html>"#,
        app_name,
        name,
        "http://localhost:8080",
    )
}

/// Styled HTML template for an order confirmation/receipt email.
fn receipt_email_html(name: &str, order_id: &str, amount: &str, app_name: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #1a1a2e; margin: 0; padding: 0; background-color: #f8fafc;">
    <div style="max-width: 600px; margin: 40px auto; padding: 0;">
        <div style="background: linear-gradient(135deg, #059669, #10b981); padding: 40px 30px; text-align: center; border-radius: 12px 12px 0 0;">
            <div style="font-size: 48px; margin-bottom: 8px;">✅</div>
            <h1 style="color: #ffffff; margin: 0; font-size: 28px;">Payment Confirmed!</h1>
            <p style="color: #a7f3d0; margin: 8px 0 0; font-size: 15px;">Order #{0}</p>
        </div>
        <div style="background: #ffffff; padding: 36px 30px; border: 1px solid #e2e8f0; border-top: none; border-radius: 0 0 12px 12px;">
            <p style="font-size: 16px; color: #334155;">Hi <strong>{1}</strong>,</p>
            <p style="font-size: 16px; color: #475569;">Your order has been confirmed and is being processed.</p>
            <div style="background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 8px; padding: 20px; margin: 24px 0;">
                <table style="width: 100%; border-collapse: collapse;">
                    <tr>
                        <td style="color: #64748b; font-size: 14px; padding: 8px 0;">Order ID</td>
                        <td style="text-align: right; font-weight: 600; font-size: 14px; padding: 8px 0;">#{0}</td>
                    </tr>
                    <tr>
                        <td style="color: #64748b; font-size: 14px; padding: 8px 0; border-top: 1px solid #e2e8f0;">Amount</td>
                        <td style="text-align: right; font-weight: 600; font-size: 18px; padding: 8px 0; border-top: 1px solid #e2e8f0; color: #059669;">{2}</td>
                    </tr>
                </table>
            </div>
            <p style="font-size: 14px; color: #94a3b8;">You'll receive a shipping confirmation when your order is on its way.</p>
            <hr style="border: none; border-top: 1px solid #e2e8f0; margin: 24px 0;">
            <p style="font-size: 14px; color: #94a3b8;">Regards,<br>The {3} Team</p>
        </div>
    </div>
</body>
</html>"#,
        order_id,
        name,
        amount,
        app_name,
    )
}

// =============================================================================
// CONTROLLER
// =============================================================================

/// Controller for sending transactional emails.
///
/// All handlers extract `MailManager` from Axum's request extensions.
/// Wire it up in your application boot:
///
/// ```ignore
/// use larastvel_core::mail::{LogMailer, MailManager};
/// use axum::{Router, Extension};
///
/// let mut mail_manager = MailManager::new("log");
/// mail_manager.register("log", LogMailer::new("log"));
/// // mail_manager.register("smtp", SmtpMailer::new("smtp", "smtp.example.com", 587, "user", "pass").unwrap());
///
/// let app = Router::new()
///     .layer(Extension(mail_manager))
///     .layer(Extension(mail_controller::mail_rate_limiter()));
/// ```
pub struct MailController;

impl MailController {
    /// Register all mail routes on the given `Registrar`.
    ///
    /// # Required Extensions
    ///
    /// - `MailManager` — manages mailer instances (log, smtp, etc.)
    /// - `RateLimiter` — rate-limits the send endpoints
    pub fn register_routes(registrar: &Registrar) {
        registrar.get("/mail/test", Self::show_dashboard);
        registrar.post("/mail/send", Self::send_email);
        registrar.post("/mail/welcome", Self::send_welcome);
        registrar.post("/mail/receipt", Self::send_receipt);
    }

    // -------------------------------------------------------------------------
    // HANDLERS
    // -------------------------------------------------------------------------

    /// GET /mail/test — show the mail-testing dashboard.
    pub async fn show_dashboard() -> Response {
        Html(EMAIL_DASHBOARD_HTML).into_response()
    }

    /// POST /mail/send — send a custom transactional email.
    ///
    /// Accepts plain text or HTML content. Supports optional CC/BCC.
    pub async fn send_email(
        Extension(mail_manager): Extension<MailManager>,
        Extension(rate_limiter): Extension<RateLimiter>,
        Form(body): Form<SendEmailRequest>,
    ) -> Response {
        let email = body.to.trim().to_lowercase();
        if !email.contains('@') || !email.contains('.') {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Invalid email address"})),
            )
                .into_response();
        }

        // Rate limit check
        if rate_limiter.too_many_attempts(&email) {
            let retry_after = rate_limiter.retry_after(&email);
            return RateLimitExceeded {
                retry_after,
                limiter_name: "mail-send".to_string(),
            }
            .into_response();
        }
        rate_limiter.hit(&email);

        // Build the mailable
        let is_html = body.content_type.to_lowercase() == "html";
        let mut mailable = if is_html {
            Mailable::html(vec![email.clone()], &body.subject, &body.body)
                .from("noreply@example.com")
                .reply_to("support@example.com")
        } else {
            Mailable::new(vec![email.clone()], &body.subject, &body.body)
                .from("noreply@example.com")
                .reply_to("support@example.com")
        };

        // Add CC if provided
        if let Some(cc) = &body.cc {
            let addresses: Vec<String> = cc
                .split(',')
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty())
                .collect();
            if !addresses.is_empty() {
                mailable = mailable.cc(addresses);
            }
        }

        // Add BCC if provided
        if let Some(bcc) = &body.bcc {
            let addresses: Vec<String> = bcc
                .split(',')
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty())
                .collect();
            if !addresses.is_empty() {
                mailable = mailable.bcc(addresses);
            }
        }

        // Send via the default mailer
        match mail_manager.default_mailer() {
            Ok(mailer) => match mailer.send(mailable).await {
                Ok(()) => Json(json!({
                    "message": "Email sent successfully",
                    "to": email,
                    "subject": body.subject,
                    "via": mailer.name(),
                }))
                .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to send: {}", e)})),
                )
                    .into_response(),
            },
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Mailer not configured: {}", e)})),
            )
                .into_response(),
        }
    }

    /// POST /mail/welcome — send a pre-built welcome email.
    ///
    /// Demonstrates a styled HTML transactional email template.
    pub async fn send_welcome(
        Extension(mail_manager): Extension<MailManager>,
        Extension(rate_limiter): Extension<RateLimiter>,
        Form(body): Form<WelcomeEmailRequest>,
    ) -> Response {
        let email = body.email.trim().to_lowercase();
        if !email.contains('@') {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Invalid email address"})),
            )
                .into_response();
        }
        if body.name.trim().is_empty() {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Name is required"})),
            )
                .into_response();
        }

        // Rate limit
        if rate_limiter.too_many_attempts(&email) {
            let retry_after = rate_limiter.retry_after(&email);
            return RateLimitExceeded {
                retry_after,
                limiter_name: "mail-send".to_string(),
            }
            .into_response();
        }
        rate_limiter.hit(&email);

        let html = welcome_email_html(body.name.trim(), "Larastvel");

        let mailable = Mailable::html(vec![email.clone()], "Welcome to Larastvel! 🎉", &html)
            .from("welcome@example.com");

        match mail_manager.default_mailer() {
            Ok(mailer) => match mailer.send(mailable).await {
                Ok(()) => Json(json!({
                    "message": "Welcome email sent",
                    "to": email,
                    "via": mailer.name(),
                }))
                .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to send: {}", e)})),
                )
                    .into_response(),
            },
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Mailer not configured: {}", e)})),
            )
                .into_response(),
        }
    }

    /// POST /mail/receipt — send a pre-built order receipt email.
    ///
    /// Demonstrates a styled HTML transactional email with order details.
    pub async fn send_receipt(
        Extension(mail_manager): Extension<MailManager>,
        Extension(rate_limiter): Extension<RateLimiter>,
        Form(body): Form<ReceiptEmailRequest>,
    ) -> Response {
        let email = body.email.trim().to_lowercase();
        if !email.contains('@') {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Invalid email address"})),
            )
                .into_response();
        }

        // Rate limit
        if rate_limiter.too_many_attempts(&email) {
            let retry_after = rate_limiter.retry_after(&email);
            return RateLimitExceeded {
                retry_after,
                limiter_name: "mail-send".to_string(),
            }
            .into_response();
        }
        rate_limiter.hit(&email);

        let html = receipt_email_html(
            body.name.trim(),
            &body.order_id,
            &body.amount,
            "Larastvel",
        );
        let subject = format!("Order #{} Confirmed ✅", body.order_id);

        let mailable = Mailable::html(vec![email.clone()], &subject, &html)
            .from("orders@example.com")
            .reply_to("support@example.com");

        match mail_manager.default_mailer() {
            Ok(mailer) => match mailer.send(mailable).await {
                Ok(()) => Json(json!({
                    "message": "Order receipt sent",
                    "to": email,
                    "order_id": body.order_id,
                    "via": mailer.name(),
                }))
                .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to send: {}", e)})),
                )
                    .into_response(),
            },
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Mailer not configured: {}", e)})),
            )
                .into_response(),
        }
    }
}

// =============================================================================
// DASHBOARD HTML
// =============================================================================

const EMAIL_DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Mail Test Dashboard</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
               background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%);
               min-height: 100vh; padding: 2rem; color: #e2e8f0; }
        .container { max-width: 800px; margin: 0 auto; }
        h1 { font-size: 2rem; font-weight: 800; margin-bottom: 0.5rem;
             background: linear-gradient(135deg, #f59e0b, #ef4444, #ec4899);
             -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
        .subtitle { color: #94a3b8; margin-bottom: 2rem; font-size: 1rem; }
        .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(340px, 1fr)); gap: 1.5rem; }
        .card { background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1);
                border-radius: 12px; padding: 1.5rem; backdrop-filter: blur(10px); }
        .card h2 { font-size: 1.125rem; margin-bottom: 0.25rem; color: #f1f5f9; }
        .card .desc { font-size: 0.8125rem; color: #64748b; margin-bottom: 1rem; }
        label { display: block; color: #cbd5e1; font-size: 0.8125rem; font-weight: 600; margin-bottom: 0.25rem; }
        input, textarea, select { width: 100%; padding: 0.625rem; border: 1px solid #475569; border-radius: 6px;
               background: #0f172a; color: #e2e8f0; font-size: 0.875rem; outline: none;
               transition: border-color 0.2s; margin-bottom: 0.75rem; }
        input:focus, textarea:focus, select:focus { border-color: #6366f1; }
        textarea { resize: vertical; min-height: 80px; font-family: inherit; }
        button { padding: 0.625rem 1.25rem; background: #6366f1; color: #fff; border: none;
                 border-radius: 6px; font-size: 0.875rem; font-weight: 600; cursor: pointer;
                 transition: all 0.2s; }
        button:hover { background: #4f46e5; transform: translateY(-1px); }
        button.green { background: #059669; }
        button.green:hover { background: #047857; }
        .status { margin-top: 1rem; padding: 0.75rem; border-radius: 6px; font-size: 0.8125rem; display: none; }
        .status.success { display: block; background: #064e3b; color: #6ee7b7; border: 1px solid #065f46; }
        .status.error { display: block; background: #450a0a; color: #fca5a5; border: 1px solid #7f1d1d; }
        .info-box { background: #1e3a5f; color: #93c5fd; padding: 0.75rem; border-radius: 6px;
                    font-size: 0.8125rem; margin-bottom: 1rem; }
    </style>
</head>
<body>
    <div class="container">
        <h1>📧 Mail Dashboard</h1>
        <p class="subtitle">Test and preview transactional emails</p>

        <div class="cards">
            <!-- Custom Send -->
            <div class="card">
                <h2>Custom Email</h2>
                <p class="desc">Send a custom email to any address</p>
                <form action="/mail/send" method="POST">
                    <label for="send-to">To</label>
                    <input type="email" id="send-to" name="to" placeholder="user@example.com" required>
                    <label for="send-subject">Subject</label>
                    <input type="text" id="send-subject" name="subject" placeholder="Hello!" required>
                    <label for="send-body">Body</label>
                    <textarea id="send-body" name="body" placeholder="Email content..."></textarea>
                    <label for="send-type">Content Type</label>
                    <select id="send-type" name="content_type">
                        <option value="html">HTML</option>
                        <option value="text">Plain Text</option>
                    </select>
                    <button type="submit">Send ✉️</button>
                </form>
            </div>

            <!-- Welcome Email -->
            <div class="card">
                <h2>🎉 Welcome Email</h2>
                <p class="desc">Send the pre-built welcome email template</p>
                <div class="info-box">Uses a styled HTML template with gradient header, call-to-action button, and onboarding tips.</div>
                <form action="/mail/welcome" method="POST">
                    <label for="welcome-name">Name</label>
                    <input type="text" id="welcome-name" name="name" placeholder="Jane Doe" required>
                    <label for="welcome-email">Email</label>
                    <input type="email" id="welcome-email" name="email" placeholder="jane@example.com" required>
                    <button type="submit" class="green">Send Welcome 🎉</button>
                </form>
            </div>

            <!-- Order Receipt -->
            <div class="card">
                <h2>🧾 Order Receipt</h2>
                <p class="desc">Send the pre-built order confirmation template</p>
                <div class="info-box">Uses a styled HTML template with order ID, amount breakdown, and green accent theme.</div>
                <form action="/mail/receipt" method="POST">
                    <label for="receipt-name">Name</label>
                    <input type="text" id="receipt-name" name="name" placeholder="John Smith" required>
                    <label for="receipt-email">Email</label>
                    <input type="email" id="receipt-email" name="email" placeholder="john@example.com" required>
                    <label for="receipt-order">Order ID</label>
                    <input type="text" id="receipt-order" name="order_id" placeholder="ORD-2024-1234" required>
                    <label for="receipt-amount">Amount</label>
                    <input type="text" id="receipt-amount" name="amount" placeholder="$49.99" required>
                    <button type="submit" class="green">Send Receipt 🧾</button>
                </form>
            </div>
        </div>
    </div>
</body>
</html>"#;

// =============================================================================
// ENTRY POINT
// =============================================================================

/// Run this example standalone (displays the controller API).
fn main() {
    println!("MailController example — see the source code for route handlers and templates.");
    println!();
    println!("Routes:");
    println!("  GET  /mail/test         — show mail-testing dashboard");
    println!("  POST /mail/send         — send a custom email (form: to, subject, body, content_type)");
    println!("  POST /mail/welcome      — send a welcome email (form: name, email)");
    println!("  POST /mail/receipt      — send an order receipt (form: name, email, order_id, amount)");
    println!();
    println!("Rate limiting: 10 emails/minute per recipient address.");
    println!();
    println!("To register routes:");
    println!("  use mail_controller::MailController;");
    println!("  MailController::register_routes(&router);");
    println!();
    println!("Required extensions: MailManager + RateLimiter");
    println!("Wire up via Application::bind() or Router::layer().");
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use larastvel_core::axum::body::Body;
    use larastvel_core::axum::http::Request;
    use larastvel_core::axum::Router as AxumRouter;
    use larastvel_core::axum::routing;
    use tower::ServiceExt;

    /// Build a test router with MailManager and RateLimiter in extensions.
    fn test_router() -> AxumRouter {
        let mut mail_manager = MailManager::new("log");
        mail_manager.register("log", LogMailer::new("log"));

        let rate_limiter = mail_rate_limiter();

        AxumRouter::new()
            .route(
                "/mail/test",
                routing::get(MailController::show_dashboard),
            )
            .route(
                "/mail/send",
                routing::post(MailController::send_email),
            )
            .route(
                "/mail/welcome",
                routing::post(MailController::send_welcome),
            )
            .route(
                "/mail/receipt",
                routing::post(MailController::send_receipt),
            )
            .layer(Extension(mail_manager))
            .layer(Extension(rate_limiter))
    }

    // -------------------------------------------------------------------------
    // Dashboard
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_dashboard_returns_html() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/mail/test")
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

    // -------------------------------------------------------------------------
    // Custom Send
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_email_success() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "to=user%40example.com&subject=Hello&body=Test+message&content_type=text",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_send_email_invalid_address() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "to=notanemail&subject=Test&body=Test&content_type=text",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    #[tokio::test]
    async fn test_send_email_html_content() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "to=dev%40example.com&subject=HTML+Test&body=%3Ch1%3EHello%3C%2Fh1%3E&content_type=html",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_send_email_with_cc() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "to=primary%40example.com&subject=CC+Test&body=With+CC&content_type=text&cc=cc%40example.com",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
    }

    // -------------------------------------------------------------------------
    // Welcome Email
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_welcome_success() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/welcome")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("name=Alice&email=alice%40example.com"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_send_welcome_missing_name() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/welcome")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("name=&email=alice%40example.com"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    // -------------------------------------------------------------------------
    // Order Receipt
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_receipt_success() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/receipt")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "name=Bob&email=bob%40example.com&order_id=ORD-001&amount=%2449.99",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
    }

    // -------------------------------------------------------------------------
    // Rate Limiting
    // -------------------------------------------------------------------------

    #[test]
    fn test_mail_rate_limiter_direct() {
        let limiter = mail_rate_limiter();
        assert!(!limiter.too_many_attempts("heavy@example.com"));
        for _ in 0..10 {
            limiter.hit("heavy@example.com");
        }
        assert!(limiter.too_many_attempts("heavy@example.com"));
    }

    #[tokio::test]
    async fn test_mail_not_rate_limited_when_fresh() {
        let mut mail_manager = MailManager::new("log");
        mail_manager.register("log", LogMailer::new("log"));
        let rate_limiter = mail_rate_limiter();

        let app = AxumRouter::new()
            .route("/mail/send", routing::post(MailController::send_email))
            .layer(Extension(mail_manager))
            .layer(Extension(rate_limiter));

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "to=fresh%40example.com&subject=Hi&body=test&content_type=text",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200, "Fresh limiter should allow first request");
    }

    #[tokio::test]
    async fn test_mail_rate_limit_exceeded() {
        // Pre-seed the rate limiter with 10 hits (the limit), then verify
        // the handler returns 429 for the next attempt.
        let rate_limiter = mail_rate_limiter();
        for _ in 0..10 {
            rate_limiter.hit("ratelimited@example.com");
        }

        let mut mail_manager = MailManager::new("log");
        mail_manager.register("log", LogMailer::new("log"));

        let app = AxumRouter::new()
            .route("/mail/send", routing::post(MailController::send_email))
            .layer(Extension(mail_manager))
            .layer(Extension(rate_limiter));

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mail/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "to=ratelimited%40example.com&subject=Test&body=test&content_type=text",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 429);
    }

    // -------------------------------------------------------------------------
    // Template Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_welcome_template_contains_name() {
        let html = welcome_email_html("TestUser", "MyApp");
        assert!(html.contains("TestUser"));
        assert!(html.contains("MyApp"));
        assert!(html.contains("Go to Dashboard"));
    }

    #[test]
    fn test_receipt_template_contains_order_details() {
        let html = receipt_email_html("Customer", "ORD-42", "$19.99", "Shop");
        assert!(html.contains("Customer"));
        assert!(html.contains("ORD-42"));
        assert!(html.contains("$19.99"));
        assert!(html.contains("Shop"));
    }

    // -------------------------------------------------------------------------
    // HTML Structure / Form Elements
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_dashboard_contains_forms() {
        let app = test_router();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/mail/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 65_536)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&body_bytes);

        // All three forms present
        assert!(html.contains(r#"action="/mail/send""#));
        assert!(html.contains(r#"action="/mail/welcome""#));
        assert!(html.contains(r#"action="/mail/receipt""#));

        // Form fields
        assert!(html.contains(r#"name="to""#));
        assert!(html.contains(r#"name="name""#));
        assert!(html.contains(r#"name="email""#));
        assert!(html.contains(r#"name="order_id""#));
        assert!(html.contains(r#"name="amount""#));
    }
}
