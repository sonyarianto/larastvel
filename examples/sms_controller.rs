//! # SmsController Example
//!
//! A REST API for sending SMS notifications via the notifications module's
//! SMS channel, demonstrating how to configure `NotificationSender` with
//! an `SmsSender` and integrate it with Axum route handlers.
//!
//! ## Routes
//!
//! | Method | URI                       | Description                          |
//! |--------|---------------------------|--------------------------------------|
//! | GET    | `/sms`                    | Show SMS dashboard (HTML)            |
//! | POST   | `/api/sms/send`           | Send an SMS notification             |
//! | GET    | `/api/sms/history`        | List sent SMS messages (JSON)        |
//!
//! ## Integration
//!
//! ```ignore
//! use larastvel_core::routing::Registrar;
//!
//! let router = app.router();
//! sms_controller::SmsController::register_routes(&router);
//! ```
//!
//! The controller expects a `SharedSmsStore` and `RateLimiter` in Axum's
//! extensions, and uses `LogSmsSender` for development/testing. In
//! production, swap in `VonageSmsSender` with real API credentials.

#![allow(unused_imports, dead_code)]

use std::sync::Arc;

use larastvel_core::axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    Form, Router,
};
use larastvel_core::notifications::{
    Notifiable, Notification, NotificationChannel, NotificationSender,
};
use larastvel_core::rate_limiter::{RateLimitConfig, RateLimitExceeded, RateLimiter};
use larastvel_core::routing::Registrar;
use larastvel_core::serde::{Deserialize, Serialize};
use larastvel_core::serde_json::{self, json};
use larastvel_core::sms::{LogSmsSender, SmsMessage, SmsSender};
use std::sync::Mutex;

// =============================================================================
// IN-MEMORY SMS STORE
// =============================================================================

/// An entry in the sent-SMS history.
#[derive(Debug, Clone, Serialize)]
pub struct SentSmsEntry {
    pub id: u64,
    pub to: String,
    pub from: String,
    pub message: String,
    pub sent_at: i64,
    pub status: String,
}

/// Thread-safe shared SMS history store.
#[derive(Debug)]
pub struct SmsStore {
    entries: Vec<SentSmsEntry>,
    next_id: u64,
}

impl Default for SmsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SmsStore {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    pub fn add(
        &mut self,
        to: String,
        from: String,
        message: String,
        status: String,
    ) -> SentSmsEntry {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let id = self.next_id;
        self.next_id += 1;

        let entry = SentSmsEntry {
            id,
            to,
            from,
            message,
            sent_at: now,
            status,
        };

        self.entries.push(entry.clone());
        entry
    }

    pub fn all(&self) -> &[SentSmsEntry] {
        &self.entries
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// Shared type for Axum extensions.
pub type SharedSmsStore = Arc<Mutex<SmsStore>>;

/// Create a shared SMS store.
pub fn new_sms_store() -> SharedSmsStore {
    Arc::new(Mutex::new(SmsStore::new()))
}

// =============================================================================
// REQUEST / RESPONSE TYPES
// =============================================================================

/// Payload for sending an SMS via the demo endpoint.
#[derive(Debug, Deserialize)]
pub struct SendSmsRequest {
    pub phone: String,
    pub message: String,
    pub from: Option<String>,
}

/// Query parameters for listing SMS history.
#[derive(Debug, Deserialize)]
pub struct SmsHistoryQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// Paginated SMS history response.
#[derive(Debug, Serialize)]
pub struct PaginatedSmsHistory {
    pub data: Vec<SentSmsEntry>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
}

// =============================================================================
// RATE LIMITER
// =============================================================================

/// Create a rate limiter for the SMS send endpoint.
///
/// Limits to 5 sends per minute per phone number to prevent abuse.
pub fn sms_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimitConfig::per_minute(5).named("sms-send"))
}

// =============================================================================
// NOTIFIABLE HELPERS
// =============================================================================

/// A simple notifiable implementation that routes SMS via phone number.
#[derive(Debug)]
pub struct SmsNotifiable {
    id: String,
    phone: String,
}

impl Notifiable for SmsNotifiable {
    fn notification_id(&self) -> String {
        self.id.clone()
    }

    fn route_phone(&self) -> Option<String> {
        Some(self.phone.clone())
    }
}

// =============================================================================
// NOTIFICATION — SMS DEMO
// =============================================================================

/// A demo notification that sends via the SMS channel.
#[derive(Debug, Clone)]
pub struct SmsDemoNotification {
    pub content: String,
    pub from: Option<String>,
}

impl Notification for SmsDemoNotification {
    fn via(&self) -> Vec<NotificationChannel> {
        vec![NotificationChannel::Sms]
    }

    fn to_sms(&self) -> Option<SmsMessage> {
        let mut msg = SmsMessage::new("", &self.content);
        if let Some(ref from) = self.from {
            msg = msg.from(from);
        }
        Some(msg)
    }
}

// =============================================================================
// CONTROLLER
// =============================================================================

/// Controller for sending SMS notifications via the notification system.
///
/// All handlers extract a `SharedSmsStore` and a `RateLimiter` from Axum's
/// request extensions. The `POST /api/sms/send` handler also configures a
/// `NotificationSender` with a `LogSmsSender` to demonstrate the SMS channel.
///
/// Wire it up in your application boot:
///
/// ```ignore
/// use axum::{Router, Extension};
///
/// let store = sms_controller::new_sms_store();
/// let rate_limiter = sms_controller::sms_rate_limiter();
///
/// let app = Router::new()
///     .layer(Extension(store))
///     .layer(Extension(rate_limiter));
/// ```
pub struct SmsController;

impl SmsController {
    /// Register all SMS routes on the given `Registrar`.
    ///
    /// # Required Extensions
    ///
    /// - `SharedSmsStore` — for tracking sent SMS history
    /// - `RateLimiter` — for rate-limiting the send endpoint
    pub fn register_routes(registrar: &Registrar) {
        registrar.get("/sms", Self::show_dashboard);
        registrar.post("/api/sms/send", Self::send_sms);
        registrar.get("/api/sms/history", Self::list_history);
    }

    // -------------------------------------------------------------------------
    // HANDLERS
    // -------------------------------------------------------------------------

    /// GET /sms — show the SMS dashboard (HTML).
    pub async fn show_dashboard(Extension(store): Extension<SharedSmsStore>) -> Response {
        let history = {
            let store = store.lock().unwrap();
            store
                .all()
                .iter()
                .rev()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
        };
        let total_sent = {
            let store = store.lock().unwrap();
            store.count()
        };

        let recent_rows: String = history
            .iter()
            .map(|entry| {
                let truncated_msg = if entry.message.len() > 60 {
                    format!("{}…", &entry.message[..60])
                } else {
                    entry.message.clone()
                };
                let status_badge = if entry.status == "sent" {
                    r#"<span class="badge sent">Sent</span>"#
                } else {
                    r#"<span class="badge failed">Failed</span>"#
                };
                format!(
                    r#"<tr>
                        <td><span class="sms-id">{0}</span></td>
                        <td class="sms-to">{1}</td>
                        <td class="sms-msg">{2}</td>
                        <td>{3}</td>
                        <td>{4}</td>
                    </tr>"#,
                    entry.id,
                    html_escape(&entry.to),
                    html_escape(&truncated_msg),
                    status_badge,
                    format_timestamp(entry.sent_at),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SMS Dashboard</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
               background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%);
               min-height: 100vh; padding: 2rem; color: #e2e8f0; }}
        .container {{ max-width: 900px; margin: 0 auto; }}
        h1 {{ font-size: 2rem; font-weight: 800; margin-bottom: 0.5rem;
             background: linear-gradient(135deg, #6366f1, #8b5cf6);
             -webkit-background-clip: text; -webkit-text-fill-color: transparent; }}
        .subtitle {{ color: #94a3b8; margin-bottom: 2rem; font-size: 1rem; }}
        .stats {{ display: flex; gap: 1rem; margin-bottom: 2rem; }}
        .stat {{ background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1);
                 border-radius: 8px; padding: 1rem 1.5rem; flex: 1; }}
        .stat-value {{ font-size: 1.75rem; font-weight: 700; color: #f1f5f9; }}
        .stat-label {{ font-size: 0.75rem; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; }}
        .stat.sms .stat-value {{ color: #22d3ee; }}
        .card {{ background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1);
                border-radius: 12px; padding: 1.5rem; backdrop-filter: blur(10px); }}
        table {{ width: 100%; border-collapse: collapse; }}
        th {{ text-align: left; color: #94a3b8; font-size: 0.75rem; text-transform: uppercase;
              letter-spacing: 0.05em; padding: 0.75rem 0.5rem; border-bottom: 1px solid #334155; }}
        td {{ padding: 0.75rem 0.5rem; border-bottom: 1px solid #1e293b; font-size: 0.875rem; }}
        .sms-id {{ color: #64748b; font-family: monospace; font-size: 0.75rem; }}
        .sms-to {{ color: #f1f5f9; font-weight: 600; font-family: monospace; }}
        .sms-msg {{ color: #cbd5e1; max-width: 250px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
        .badge {{ display: inline-block; padding: 0.125rem 0.5rem; border-radius: 999px;
                  font-size: 0.6875rem; font-weight: 600; }}
        .badge.sent {{ background: #064e3b; color: #6ee7b7; }}
        .badge.failed {{ background: #7f1d1d; color: #fca5a5; }}
        .btn {{ padding: 0.5rem 1rem; background: #6366f1; color: #fff; border: none;
                border-radius: 6px; font-size: 0.8125rem; font-weight: 600; cursor: pointer;
                transition: all 0.2s; }}
        .btn:hover {{ background: #4f46e5; transform: translateY(-1px); }}
        .btn-group {{ display: flex; gap: 0.5rem; margin-bottom: 1rem; align-items: center; }}
        .empty {{ text-align: center; color: #64748b; padding: 2rem; font-size: 0.875rem; }}
        label {{ display: block; color: #cbd5e1; font-size: 0.8125rem; font-weight: 600; margin-bottom: 0.25rem; }}
        input, textarea, select {{ width: 100%; padding: 0.625rem; border: 1px solid #475569; border-radius: 6px;
               background: #0f172a; color: #e2e8f0; font-size: 0.875rem; outline: none;
               transition: border-color 0.2s; margin-bottom: 0.75rem; }}
        input:focus, textarea:focus {{ border-color: #6366f1; }}
        textarea {{ resize: vertical; min-height: 80px; font-family: inherit; }}
        .form-card {{ margin-bottom: 1.5rem; }}
        .info-box {{ background: #1e3a5f; color: #93c5fd; padding: 0.75rem; border-radius: 6px;
                    font-size: 0.8125rem; margin-bottom: 1rem; }}
        .hint {{ color: #64748b; font-size: 0.75rem; margin-top: -0.5rem; margin-bottom: 0.75rem; }}
        .refresh {{ color: #6366f1; text-decoration: none; font-size: 0.8125rem; }}
        .refresh:hover {{ text-decoration: underline; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>📱 SMS Notification Dashboard</h1>
        <p class="subtitle">Send and monitor SMS notifications via the notification system</p>

        <div class="stats">
            <div class="stat sms">
                <div class="stat-value">{0}</div>
                <div class="stat-label">SMS Sent</div>
            </div>
            <div class="stat">
                <div class="stat-value">{1}</div>
                <div class="stat-label">Log Sender</div>
            </div>
        </div>

        <div class="card form-card">
            <h2 style="font-size: 1.125rem; margin-bottom: 0.5rem; color: #f1f5f9;">📨 Send SMS Notification</h2>
            <p style="font-size: 0.8125rem; color: #64748b; margin-bottom: 1rem;">
                Sends an SMS via the notification system's <code>SmsSender</code> channel.
                Uses <code>LogSmsSender</code> in this example — swap for <code>VonageSmsSender</code> in production.
            </p>
            <form action="/api/sms/send" method="POST">
                <label for="phone">Phone Number (E.164 format)</label>
                <input type="text" id="phone" name="phone" value="+15551234567" placeholder="+15551234567" required>
                <div class="hint">Use E.164 format: +[country code][number]</div>
                <label for="from">Sender ID (optional)</label>
                <input type="text" id="from" name="from" value="Larastvel" placeholder="Larastvel">
                <div class="hint">Leave empty to use the SMS sender's default</div>
                <label for="message">Message</label>
                <textarea id="message" name="message" placeholder="Type your SMS message here..." required>Hello from Larastvel! Your notification system is working. 🎉</textarea>
                <button type="submit" class="btn">Send SMS</button>
            </form>
        </div>

        <div class="card">
            <div class="btn-group">
                <h2 style="font-size: 1.125rem; color: #f1f5f9; flex: 1;">📋 Sent SMS History</h2>
                <a href="/sms" class="refresh">🔄 Refresh</a>
            </div>
            <table>
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>To</th>
                        <th>Message</th>
                        <th>Status</th>
                        <th>Sent</th>
                    </tr>
                </thead>
                <tbody>
                    {2}
                </tbody>
            </table>
            {3}
        </div>
    </div>
</body>
</html>"#,
            total_sent,
            "LogSmsSender",
            if recent_rows.is_empty() {
                r#"<tr><td colspan="5" class="empty">No SMS messages sent yet. Send one above!</td></tr>"#.to_string()
            } else {
                recent_rows
            },
            if total_sent == 0 {
                String::new()
            } else {
                r#"<p style="text-align:center;margin-top:1rem;color:#64748b;font-size:0.8125rem;">
                    Showing up to 20 recent messages.
                   </p>"#
                    .to_string()
            },
        );
        Html(html).into_response()
    }

    /// POST /api/sms/send — send an SMS notification.
    ///
    /// Rate-limited to 5 sends per minute per phone number.
    pub async fn send_sms(
        Extension(store): Extension<SharedSmsStore>,
        Extension(rate_limiter): Extension<RateLimiter>,
        Form(body): Form<SendSmsRequest>,
    ) -> Response {
        // Validate input
        let phone = body.phone.trim();
        if phone.is_empty() {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Phone number is required"})),
            )
                .into_response();
        }

        if !phone.starts_with('+') {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Phone number must be in E.164 format (e.g. +15551234567)"})),
            )
                .into_response();
        }

        let message = body.message.trim();
        if message.is_empty() {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Message is required"})),
            )
                .into_response();
        }

        if message.len() > 1600 {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Message exceeds 1600 character limit"})),
            )
                .into_response();
        }

        // Rate limit check
        if rate_limiter.too_many_attempts(phone) {
            let retry_after = rate_limiter.retry_after(phone);
            return RateLimitExceeded {
                retry_after,
                limiter_name: "sms-send".to_string(),
            }
            .into_response();
        }
        rate_limiter.hit(phone);

        // Build the sender with LogSmsSender
        let sms_sender: Arc<dyn SmsSender> = Arc::new(LogSmsSender::new());
        let sender = NotificationSender::new()
            .with_sms_sender(sms_sender)
            .with_app_name("Larastvel");

        let notifiable = SmsNotifiable {
            id: phone.to_string(),
            phone: phone.to_string(),
        };

        let from = body
            .from
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let notification = SmsDemoNotification {
            content: message.to_string(),
            from,
        };

        let results = sender.send(&notifiable, notification).await;
        let sms_result = results.get(&NotificationChannel::Sms);

        match sms_result {
            Some(Ok(())) => {
                // Record in history
                let entry = {
                    let mut store = store.lock().unwrap();
                    store.add(
                        phone.to_string(),
                        body.from.clone().unwrap_or_else(|| "Larastvel".to_string()),
                        message.to_string(),
                        "sent".to_string(),
                    )
                };

                Json(json!({
                    "message": "SMS sent successfully",
                    "id": entry.id,
                    "to": phone,
                }))
                .into_response()
            }
            Some(Err(e)) => {
                // Record failure in history
                let _entry = {
                    let mut store = store.lock().unwrap();
                    store.add(
                        phone.to_string(),
                        body.from.clone().unwrap_or_else(|| "Larastvel".to_string()),
                        message.to_string(),
                        format!("failed: {}", e),
                    )
                };

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to send SMS: {}", e)})),
                )
                    .into_response()
            }
            None => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "No SMS channel was used"})),
            )
                .into_response(),
        }
    }

    /// GET /api/sms/history — list sent SMS messages (JSON).
    pub async fn list_history(
        Extension(store): Extension<SharedSmsStore>,
        Query(query): Query<SmsHistoryQuery>,
    ) -> Json<PaginatedSmsHistory> {
        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(20).min(100);

        let all_entries = {
            let store = store.lock().unwrap();
            store.all().iter().rev().cloned().collect::<Vec<_>>()
        };

        let total = all_entries.len();
        let offset = ((page.saturating_sub(1)) * per_page) as usize;
        let data = all_entries
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .collect();

        Json(PaginatedSmsHistory {
            data,
            total,
            page,
            per_page,
        })
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Simple HTML-escaping for strings.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Format a Unix timestamp to a human-readable string.
fn format_timestamp(unix_secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let diff = now - unix_secs;
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

// =============================================================================
// ENTRY POINT
// =============================================================================

fn main() {
    println!("SmsController example — see the source code for route handlers.");
    println!();
    println!("Routes:");
    println!("  GET  /sms                       — show SMS dashboard (HTML)");
    println!("  POST /api/sms/send             — send an SMS notification");
    println!("  GET  /api/sms/history           — list sent SMS messages (JSON)");
    println!();
    println!("Rate limiting: 5 sends/minute per phone number.");
    println!("SMS sender: LogSmsSender (swap for VonageSmsSender in production).");
    println!();
    println!("To register routes:");
    println!("  use sms_controller::SmsController;");
    println!("  SmsController::register_routes(&router);");
    println!();
    println!("Required extensions: SharedSmsStore + RateLimiter");
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
    use larastvel_core::axum::routing;
    use larastvel_core::axum::Router as AxumRouter;
    use tower::ServiceExt;

    /// Build a test router with a shared SMS store and rate limiter.
    async fn test_router() -> AxumRouter {
        let store = new_sms_store();
        let rate_limiter = sms_rate_limiter();

        AxumRouter::new()
            .route("/sms", routing::get(SmsController::show_dashboard))
            .route("/api/sms/send", routing::post(SmsController::send_sms))
            .route(
                "/api/sms/history",
                routing::get(SmsController::list_history),
            )
            .layer(Extension(store))
            .layer(Extension(rate_limiter))
    }

    // -------------------------------------------------------------------------
    // Dashboard
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_dashboard_returns_html() {
        let app = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/sms")
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
    async fn test_dashboard_shows_zero_stats() {
        let app = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/sms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 65_536)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&body_bytes);

        assert!(html.contains("SMS Notification Dashboard"));
        assert!(html.contains("0")); // Zero SMS sent
        assert!(html.contains("LogSmsSender"));
        assert!(html.contains("No SMS messages sent yet"));
    }

    // -------------------------------------------------------------------------
    // Send SMS
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_sms_success() {
        let app = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sms/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "phone=%2B15551234567&message=Hello+from+Larastvel!&from=Larastvel",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["message"], "SMS sent successfully");
        assert_eq!(json["to"], "+15551234567");
        assert!(json["id"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_send_sms_missing_phone() {
        let app = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sms/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("phone=&message=Hello&from=Test"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    #[tokio::test]
    async fn test_send_sms_missing_message() {
        let app = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sms/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("phone=%2B15551234567&message=&from=Test"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    #[tokio::test]
    async fn test_send_sms_invalid_phone_format() {
        let app = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sms/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("phone=not-a-phone&message=Hello&from=Test"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    #[tokio::test]
    async fn test_send_sms_message_too_long() {
        let app = test_router().await;
        let long_msg = "x".repeat(1601);
        let body = format!(
            "phone=%2B15551234567&message={}&from=Test",
            urlencoding(&long_msg)
        );

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sms/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    // -------------------------------------------------------------------------
    // Rate limiting
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_sms_rate_limited() {
        let rate_limiter = sms_rate_limiter();
        // Pre-seed 5 hits (the limit)
        for _ in 0..5 {
            rate_limiter.hit("+15559999999");
        }

        let store = new_sms_store();

        let app = AxumRouter::new()
            .route("/api/sms/send", routing::post(SmsController::send_sms))
            .layer(Extension(store))
            .layer(Extension(rate_limiter));

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sms/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "phone=%2B15559999999&message=Blocked+by+rate+limiter&from=Test",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 429);
    }

    // -------------------------------------------------------------------------
    // History
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_history_empty() {
        let app = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/sms/history")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["total"], 0);
        assert_eq!(json["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_history_with_data() {
        let store = new_sms_store();
        let rate_limiter = sms_rate_limiter();

        // Manually add some entries
        {
            let mut s = store.lock().unwrap();
            s.add(
                "+15551111111".to_string(),
                "App".to_string(),
                "First SMS".to_string(),
                "sent".to_string(),
            );
            s.add(
                "+15552222222".to_string(),
                "App".to_string(),
                "Second SMS".to_string(),
                "sent".to_string(),
            );
        }

        let app = AxumRouter::new()
            .route(
                "/api/sms/history",
                routing::get(SmsController::list_history),
            )
            .layer(Extension(store))
            .layer(Extension(rate_limiter));

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/sms/history")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["total"], 2);
        assert_eq!(json["data"].as_array().unwrap().len(), 2);
        assert_eq!(json["data"][0]["message"], "Second SMS"); // Most recent first
        assert_eq!(json["data"][1]["message"], "First SMS");
    }

    // -------------------------------------------------------------------------
    // Dashboard shows sent count after sending
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_dashboard_reflects_sent_count() {
        let store = new_sms_store();
        let rate_limiter = sms_rate_limiter();

        // Pre-populate with 3 entries
        {
            let mut s = store.lock().unwrap();
            s.add(
                "+15551111111".to_string(),
                "App".to_string(),
                "Msg 1".to_string(),
                "sent".to_string(),
            );
            s.add(
                "+15552222222".to_string(),
                "App".to_string(),
                "Msg 2".to_string(),
                "sent".to_string(),
            );
            s.add(
                "+15553333333".to_string(),
                "App".to_string(),
                "Msg 3".to_string(),
                "sent".to_string(),
            );
        }

        let app = AxumRouter::new()
            .route("/sms", routing::get(SmsController::show_dashboard))
            .layer(Extension(store))
            .layer(Extension(rate_limiter));

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/sms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 65_536)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&body_bytes);

        assert!(html.contains("3")); // Should show 3 SMS sent
        assert!(html.contains("Msg 1"));
        assert!(html.contains("Msg 2"));
        assert!(html.contains("Msg 3"));
    }

    // -------------------------------------------------------------------------
    // History pagination
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_history_pagination() {
        let store = new_sms_store();
        let rate_limiter = sms_rate_limiter();

        // Add 10 entries
        {
            let mut s = store.lock().unwrap();
            for i in 0..10 {
                s.add(
                    format!("+1555{:04}0000", i),
                    "App".to_string(),
                    format!("Message {}", i),
                    "sent".to_string(),
                );
            }
        }

        let app = AxumRouter::new()
            .route(
                "/api/sms/history",
                routing::get(SmsController::list_history),
            )
            .layer(Extension(store))
            .layer(Extension(rate_limiter));

        // Page 1 with 3 per page
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/sms/history?page=1&per_page=3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["total"], 10);
        assert_eq!(json["data"].as_array().unwrap().len(), 3);
        assert_eq!(json["page"], 1);
        assert_eq!(json["per_page"], 3);
    }

    // -------------------------------------------------------------------------
    // Helper tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_html_escape() {
        let input = "<script>alert('xss')</script>";
        let escaped = html_escape(input);
        assert_eq!(escaped, "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;");
    }

    #[test]
    fn test_format_timestamp() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(format_timestamp(now), "just now");

        let old = format_timestamp(now - 7200);
        assert!(old.contains("h ago") || old.contains("m ago"));
    }

    // -------------------------------------------------------------------------
    // SmsStore unit tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sms_store_add_and_count() {
        let mut store = SmsStore::new();
        assert_eq!(store.count(), 0);

        store.add(
            "+15551234567".to_string(),
            "App".to_string(),
            "Test".to_string(),
            "sent".to_string(),
        );
        assert_eq!(store.count(), 1);

        store.add(
            "+15559876543".to_string(),
            "App".to_string(),
            "Test 2".to_string(),
            "sent".to_string(),
        );
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn test_sms_store_auto_increment() {
        let mut store = SmsStore::new();
        let e1 = store.add(
            "+15551111111".to_string(),
            "App".to_string(),
            "A".to_string(),
            "sent".to_string(),
        );
        let e2 = store.add(
            "+15552222222".to_string(),
            "App".to_string(),
            "B".to_string(),
            "sent".to_string(),
        );
        assert_eq!(e1.id, 1);
        assert_eq!(e2.id, 2);
    }

    #[test]
    fn test_sms_store_entry_serialization() {
        let entry = SentSmsEntry {
            id: 1,
            to: "+15551234567".to_string(),
            from: "Larastvel".to_string(),
            message: "Hello!".to_string(),
            sent_at: 1000000,
            status: "sent".to_string(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["to"], "+15551234567");
        assert_eq!(json["from"], "Larastvel");
        assert_eq!(json["message"], "Hello!");
        assert_eq!(json["status"], "sent");
    }

    // -------------------------------------------------------------------------
    // SmsNotifiable tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_sms_notifiable_phone() {
        let notifiable = SmsNotifiable {
            id: "user-1".to_string(),
            phone: "+15551234567".to_string(),
        };
        assert_eq!(notifiable.notification_id(), "user-1");
        assert_eq!(notifiable.route_phone(), Some("+15551234567".to_string()));
    }
}

/// Minimal URL encoder (used in tests).
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
