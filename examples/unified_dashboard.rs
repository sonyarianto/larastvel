//! # Unified Dashboard Example
//!
//! A complete end-to-end application that demonstrates **Mail**, **SMS**, and
//! **Database Notification** channels working together in a single Axum app
//! with a central dashboard, shared state, and cross-controller tests.
//!
//! This example inlines all three controllers (MailController, SmsController,
//! NotificationController) to be self-contained — no dependencies on sibling
//! example files.
//!
//! ## Routes
//!
//! | Method | URI                             | Description                      |
//! |--------|---------------------------------|----------------------------------|
//! | GET    | `/`                             | Home dashboard                   |
//! | GET    | `/mail/test`                    | Mail dashboard (HTML)            |
//! | POST   | `/mail/send`                    | Send custom email                |
//! | POST   | `/mail/welcome`                 | Send welcome email               |
//! | POST   | `/mail/receipt`                 | Send order receipt               |
//! | GET    | `/broadcast`                    | Broadcast dashboard (HTML)       |
//! | POST   | `/api/broadcast/send`           | Broadcast an event (driver: log/pusher/ably) |
//! | GET    | `/api/broadcast/log`            | List broadcast events (JSON)     |
//! | GET    | `/sms`                          | SMS dashboard (HTML)             |
//! | POST   | `/api/sms/send`                 | Send an SMS                      |
//! | GET    | `/api/sms/history`              | List sent SMS (JSON)             |
//! | GET    | `/notifications`                | Notification dashboard (HTML)    |
//! | GET    | `/api/notifications`            | List all notifications (JSON)    |
//! | GET    | `/api/notifications/unread`     | List unread notifications (JSON) |
//! | POST   | `/api/notifications/send`       | Send a notification              |
//! | POST   | `/api/notifications/{id}/read`  | Mark notification as read        |
//! | POST   | `/api/notifications/read-all`   | Mark all notifications as read   |
//!
//! ## Architecture
//!
//! ```text
//! Extension(sea_orm::DatabaseConnection)   → Notification routes
//! Extension(SharedSmsStore)                 → SMS routes
//! Extension(MailManager)                    → Mail routes
//! Extension(RateLimiter "notif-send")       → Notification send endpoint
//! Extension(RateLimiter "sms-send")         → SMS send endpoint
//! Extension(RateLimiter "mail-send")        → Mail send endpoints
//! ```
//!
//! Each rate limiter is independent — hitting one doesn't affect the others.

#![allow(unused_imports, dead_code)]

use std::sync::Arc;
use std::sync::Mutex;

use larastvel_core::axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    Form, Router,
};
use larastvel_core::axum::routing;
use larastvel_core::broadcasting::ably::AblyBroadcaster;
use larastvel_core::broadcasting::log::LogBroadcaster;
use larastvel_core::broadcasting::pusher::PusherBroadcaster;
use larastvel_core::broadcasting::BroadcastManager;
use larastvel_core::broadcasting::Broadcaster;
use larastvel_core::mail::{LogMailer, MailError, MailManager, Mailer, Mailable};
use larastvel_core::notifications::{
    Notification, NotificationChannel, NotificationError, NotificationSender, Notifiable,
};
use larastvel_core::rate_limiter::{RateLimitConfig, RateLimitExceeded, RateLimiter};
use larastvel_core::sea_orm;
use larastvel_core::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use larastvel_core::serde::{Deserialize, Serialize};
use larastvel_core::serde_json::{self, json};
use larastvel_core::sms::{LogSmsSender, SmsMessage, SmsSender};

// =============================================================================
// SHARED STATE
// =============================================================================

/// Holds references to shared state for test inspection.
#[derive(Debug, Clone)]
pub struct AppState {
    pub db: sea_orm::DatabaseConnection,
    pub sms_store: SharedSmsStore,
    pub broadcast_log: SharedBroadcastLog,
}

// =============================================================================
// APPLICATION BUILDER
// =============================================================================

/// Build the complete unified Axum application with all controllers
/// registered and all extensions wired up.
pub async fn build_app() -> (Router, AppState) {
    // --- Database for notifications ---
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    let notification_sender = NotificationSender::new()
        .with_database(db.clone());
    notification_sender
        .ensure_notifications_table()
        .await
        .expect("Failed to create notifications table");

    // --- SMS store ---
    let sms_store = new_sms_store();

    // --- Mail manager ---
    let mut mail_manager = MailManager::new("log");
    mail_manager.register("log", LogMailer::new("log"));

    // --- Independent rate limiters ---
    let notif_limiter = notification_rate_limiter();
    let sms_limiter = sms_rate_limiter();
    let mail_limiter = mail_rate_limiter();

    let broadcast_log = new_broadcast_log();

    // --- Broadcast drivers (Log, Pusher, Ably) ---
    let mut broadcast_manager = BroadcastManager::new("log");
    broadcast_manager.register("log", LogBroadcaster::new("log"));
    broadcast_manager.register("pusher", PusherBroadcaster::new("pusher", "APP_ID", "KEY", "SECRET", "mt1"));
    broadcast_manager.register("ably", AblyBroadcaster::new("ably", "APP_ID:API_KEY"));

    let broadcast_limiter = broadcast_rate_limiter();

    // --- Build router ---
    let router = Router::<()>::new()
        // Home
        .route("/", routing::get(home_dashboard))
        // Mail routes
        .route("/mail/test", routing::get(mail_dashboard))
        .route("/mail/send", routing::post(mail_send))
        .route("/mail/welcome", routing::post(mail_welcome))
        .route("/mail/receipt", routing::post(mail_receipt))
        // Broadcast routes
        .route("/broadcast", routing::get(broadcast_dashboard))
        .route("/api/broadcast/send", routing::post(broadcast_send))
        .route("/api/broadcast/log", routing::get(broadcast_log_list))
        // SMS routes
        .route("/sms", routing::get(sms_dashboard))
        .route("/api/sms/send", routing::post(sms_send))
        .route("/api/sms/history", routing::get(sms_history))
        // Notification routes
        .route("/notifications", routing::get(notif_dashboard))
        .route("/api/notifications", routing::get(notif_list))
        .route("/api/notifications/unread", routing::get(notif_list_unread))
        .route("/api/notifications/send", routing::post(notif_send))
        .route("/api/notifications/{id}/read", routing::post(notif_mark_read))
        .route("/api/notifications/read-all", routing::post(notif_mark_all_read))
        // Extensions layer
        .layer(Extension(db.clone()))
        .layer(Extension(sms_store.clone()))
        .layer(Extension(broadcast_log.clone()))
        .layer(Extension(mail_manager))
        .layer(Extension(broadcast_manager))
        .layer(Extension(broadcast_limiter))
        .layer(Extension(notif_limiter))
        .layer(Extension(sms_limiter))
        .layer(Extension(mail_limiter));

    let state = AppState { db, sms_store, broadcast_log };
    (router, state)
}

// =============================================================================
// RATE LIMITERS
// =============================================================================

fn notification_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimitConfig::per_minute(5).named("notif-send"))
}

fn sms_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimitConfig::per_minute(5).named("sms-send"))
}

fn broadcast_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimitConfig::per_minute(10).named("broadcast-send"))
}

fn mail_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimitConfig::per_minute(10).named("mail-send"))
}

// =============================================================================
// SMS STORE
// =============================================================================

/// An entry in the sent-SMS history.
#[derive(Debug, Clone, Serialize)]
struct SentSmsEntry {
    id: u64,
    to: String,
    from: String,
    message: String,
    sent_at: i64,
    status: String,
}

/// Thread-safe shared SMS history store.
#[derive(Debug)]
struct SmsStore {
    entries: Vec<SentSmsEntry>,
    next_id: u64,
}

impl SmsStore {
    fn new() -> Self {
        Self { entries: Vec::new(), next_id: 1 }
    }

    fn add(&mut self, to: String, from: String, message: String, status: String) -> SentSmsEntry {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let id = self.next_id;
        self.next_id += 1;
        let entry = SentSmsEntry { id, to, from, message, sent_at: now, status };
        self.entries.push(entry.clone());
        entry
    }

    fn all(&self) -> &[SentSmsEntry] {
        &self.entries
    }

    fn count(&self) -> usize {
        self.entries.len()
    }
}

type SharedSmsStore = Arc<Mutex<SmsStore>>;

fn new_sms_store() -> SharedSmsStore {
    Arc::new(Mutex::new(SmsStore::new()))
}

// =============================================================================
// BROADCAST STORE
// =============================================================================

/// An entry in the broadcast event log.
#[derive(Debug, Clone, Serialize)]
struct BroadcastLogEntry {
    id: u64,
    channel: String,
    event: String,
    data: String,
    driver: String,
    sent_at: i64,
}

/// Thread-safe broadcast event log.
#[derive(Debug)]
struct BroadcastLog {
    entries: Vec<BroadcastLogEntry>,
    next_id: u64,
}

impl BroadcastLog {
    fn new() -> Self {
        Self { entries: Vec::new(), next_id: 1 }
    }

    fn add(&mut self, channel: String, event: String, data: String, driver: String) -> BroadcastLogEntry {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let id = self.next_id;
        self.next_id += 1;
        let entry = BroadcastLogEntry { id, channel, event, data, driver, sent_at: now };
        self.entries.push(entry.clone());
        entry
    }

    fn all(&self) -> &[BroadcastLogEntry] {
        &self.entries
    }

    fn count(&self) -> usize {
        self.entries.len()
    }
}

type SharedBroadcastLog = Arc<Mutex<BroadcastLog>>;

fn new_broadcast_log() -> SharedBroadcastLog {
    Arc::new(Mutex::new(BroadcastLog::new()))
}

// =============================================================================
// HOME DASHBOARD
// =============================================================================

async fn home_dashboard() -> Response {
    Html(HOME_HTML).into_response()
}

const HOME_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Larastvel Notification Hub</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:linear-gradient(135deg,#0f172a 0%,#1e293b 100%);min-height:100vh;padding:2rem;color:#e2e8f0}
.container{max-width:960px;margin:0 auto}
.header{text-align:center;margin-bottom:3rem}
.header h1{font-size:2.5rem;font-weight:800;margin-bottom:0.5rem;background:linear-gradient(135deg,#6366f1,#8b5cf6,#ec4899);-webkit-background-clip:text;-webkit-text-fill-color:transparent}
.header .subtitle{color:#94a3b8;font-size:1.125rem}
.header .badge{display:inline-block;background:rgba(99,102,241,0.2);color:#a5b4fc;padding:0.25rem 0.75rem;border-radius:999px;font-size:0.75rem;font-weight:600;margin-top:0.5rem;border:1px solid rgba(99,102,241,0.3)}
.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:1.5rem;margin-bottom:2rem}
.card{background:rgba(255,255,255,0.04);border:1px solid rgba(255,255,255,0.08);border-radius:16px;padding:2rem;backdrop-filter:blur(10px);transition:all 0.3s ease;position:relative;overflow:hidden}
.card:hover{transform:translateY(-4px);border-color:rgba(255,255,255,0.15);box-shadow:0 12px 40px rgba(0,0,0,0.3)}
.card .icon{font-size:2.5rem;margin-bottom:1rem}
.card h2{font-size:1.25rem;font-weight:700;color:#f1f5f9;margin-bottom:0.5rem}
.card p{color:#94a3b8;font-size:0.875rem;line-height:1.6;margin-bottom:1.25rem}
.card .route-list{list-style:none;padding:0;margin:0 0 1.25rem}
.card .route-list li{font-size:0.75rem;color:#64748b;padding:0.25rem 0;font-family:monospace}
.card .route-list li::before{content:"→ ";color:#6366f1}
.card .btn{display:inline-block;padding:0.5rem 1.25rem;border-radius:8px;font-size:0.8125rem;font-weight:600;text-decoration:none;transition:all 0.2s}
.btn-mail{background:#f59e0b;color:#fff}.btn-mail:hover{background:#d97706}
.btn-sms{background:#22d3ee;color:#0f172a}.btn-sms:hover{background:#06b6d4}
.btn-notif{background:#8b5cf6;color:#fff}.btn-notif:hover{background:#7c3aed}
.footer{text-align:center;color:#475569;font-size:0.75rem;margin-top:2rem}
</style></head>
<body><div class="container">
<div class="header"><h1>🔔 Larastvel Notification Hub</h1><p class="subtitle">Unified interface for Mail · SMS · Database notifications</p><span class="badge">17 routes · 4 channels · 1 app</span></div>
<div class="cards">
<div class="card"><div class="icon">📧</div><h2>Mail Controller</h2><p>Send transactional emails with styled HTML templates (welcome, receipts, custom). Rate-limited to 10/min.</p>
<ul class="route-list"><li>GET /mail/test — Dashboard</li><li>POST /mail/send — Custom</li><li>POST /mail/welcome — Welcome</li><li>POST /mail/receipt — Receipt</li></ul>
<a href="/mail/test" class="btn btn-mail">Open Mail Dashboard →</a></div>
<div class="card"><div class="icon">📡</div><h2>Broadcast Controller</h2><p>Send real-time WebSocket events via Log (dev), Pusher (real), or Ably (real) with event log tracking.</p>
<ul class="route-list"><li>GET /broadcast — Dashboard</li><li>POST /api/broadcast/send — Send event</li><li>GET /api/broadcast/log — Event log</li></ul>
<a href="/broadcast" class="btn" style="background:#ec4899;color:#fff">Open Broadcast Dashboard →</a></div>
<div class="card"><div class="icon">📱</div><h2>SMS Controller</h2><p>Send SMS via LogSmsSender with in-memory history. E.164 validation and 5 sends/min rate limit.</p>
<ul class="route-list"><li>GET /sms — Dashboard</li><li>POST /api/sms/send — Send</li><li>GET /api/sms/history — History</li></ul>
<a href="/sms" class="btn btn-sms">Open SMS Dashboard →</a></div>
<div class="card"><div class="icon">🔔</div><h2>Notification Controller</h2><p>Full CRUD for database-backed notifications with pagination, read tracking, and styled dashboard.</p>
<ul class="route-list"><li>GET /notifications — Dashboard</li><li>GET /api/notifications — List</li><li>POST /api/notifications/send — Send</li></ul>
<a href="/notifications" class="btn btn-notif">Open Notifications Dashboard →</a></div>
</div>
<div class="footer"><p>Built with Larastvel · Axum + SeaORM</p></div>
</div></body></html>"#;

// =============================================================================
// MAIL CONTROLLER (inlined)
// =============================================================================

#[derive(Debug, Deserialize)]
struct SendEmailRequest {
    to: String,
    subject: String,
    body: String,
    #[serde(default = "default_html")]
    content_type: String,
    cc: Option<String>,
    bcc: Option<String>,
}

fn default_html() -> String { "html".to_string() }

#[derive(Debug, Deserialize)]
struct WelcomeEmailRequest {
    name: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct ReceiptEmailRequest {
    name: String,
    email: String,
    order_id: String,
    amount: String,
}

fn welcome_email_html(name: &str, app_name: &str) -> String {
    format!(
        r#"<div style="font-family:sans-serif;max-width:600px;margin:0 auto;">
<div style="background:linear-gradient(135deg,#6366f1,#8b5cf6);padding:32px;text-align:center;border-radius:12px 12px 0 0;">
<h1 style="color:#fff;margin:0;font-size:24px;">🎉 Welcome to {0}!</h1></div>
<div style="background:#fff;padding:32px;border:1px solid #e2e8f0;border-radius:0 0 12px 12px;">
<p style="font-size:16px;color:#334155;">Hi <strong>{1}</strong>,</p>
<p style="font-size:16px;color:#475569;">Thanks for joining {0}! We're thrilled to have you.</p>
<div style="background:#f0fdf4;border:1px solid #bbf7d0;border-radius:8px;padding:20px;margin:24px 0;">
<p style="margin:0 0 8px;color:#166534;font-weight:600;">✨ What's next:</p>
<ul style="margin:0;padding-left:20px;color:#166534;"><li>Complete your profile</li><li>Explore the dashboard</li></ul></div>
<p style="text-align:center;margin:30px 0;">
<a href="http://localhost:8080/dashboard" style="display:inline-block;padding:14px 36px;background:linear-gradient(135deg,#6366f1,#8b5cf6);color:#fff;text-decoration:none;border-radius:8px;font-weight:600;font-size:16px;">Go to Dashboard →</a></p>
<hr style="border:none;border-top:1px solid #e2e8f0;margin:24px 0;">
<p style="font-size:14px;color:#94a3b8;">Regards,<br>The {0} Team</p></div></div>"#,
        app_name, name
    )
}

fn receipt_email_html(name: &str, order_id: &str, amount: &str, app_name: &str) -> String {
    format!(
        r#"<div style="font-family:sans-serif;max-width:600px;margin:0 auto;">
<div style="background:linear-gradient(135deg,#059669,#10b981);padding:32px;text-align:center;border-radius:12px 12px 0 0;">
<h1 style="color:#fff;margin:0;font-size:24px;">✅ Payment Confirmed!</h1>
<p style="color:#a7f3d0;margin:8px 0 0;font-size:15px;">Order #{0}</p></div>
<div style="background:#fff;padding:32px;border:1px solid #e2e8f0;border-radius:0 0 12px 12px;">
<p style="font-size:16px;color:#334155;">Hi <strong>{1}</strong>,</p>
<p style="font-size:16px;color:#475569;">Your order has been confirmed.</p>
<div style="background:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;padding:20px;margin:24px 0;">
<table style="width:100%;border-collapse:collapse;">
<tr><td style="color:#64748b;font-size:14px;padding:8px 0;">Order ID</td>
<td style="text-align:right;font-weight:600;font-size:14px;padding:8px 0;">#{0}</td></tr>
<tr><td style="color:#64748b;font-size:14px;padding:8px 0;border-top:1px solid #e2e8f0;">Amount</td>
<td style="text-align:right;font-weight:700;font-size:18px;padding:8px 0;border-top:1px solid #e2e8f0;color:#059669;">{2}</td></tr></table></div>
<hr style="border:none;border-top:1px solid #e2e8f0;margin:24px 0;">
<p style="font-size:14px;color:#94a3b8;">Regards,<br>The {3} Team</p></div></div>"#,
        order_id, name, amount, app_name
    )
}

const MAIL_DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Mail Dashboard</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:linear-gradient(135deg,#0f172a 0%,#1e293b 100%);min-height:100vh;padding:2rem;color:#e2e8f0}
.container{max-width:800px;margin:0 auto}
h1{font-size:2rem;font-weight:800;margin-bottom:0.5rem;background:linear-gradient(135deg,#f59e0b,#ef4444,#ec4899);-webkit-background-clip:text;-webkit-text-fill-color:transparent}
.subtitle{color:#94a3b8;margin-bottom:2rem;font-size:1rem}
.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(340px,1fr));gap:1.5rem}
.card{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:12px;padding:1.5rem;backdrop-filter:blur(10px)}
.card h2{font-size:1.125rem;margin-bottom:0.25rem;color:#f1f5f9}
.card .desc{font-size:0.8125rem;color:#64748b;margin-bottom:1rem}
label{display:block;color:#cbd5e1;font-size:0.8125rem;font-weight:600;margin-bottom:0.25rem}
input,textarea,select{width:100%;padding:0.625rem;border:1px solid #475569;border-radius:6px;background:#0f172a;color:#e2e8f0;font-size:0.875rem;outline:none;transition:border-color 0.2s;margin-bottom:0.75rem}
input:focus,textarea:focus,select:focus{border-color:#6366f1}
textarea{resize:vertical;min-height:80px;font-family:inherit}
button{padding:0.625rem 1.25rem;background:#6366f1;color:#fff;border:none;border-radius:6px;font-size:0.875rem;font-weight:600;cursor:pointer;transition:all 0.2s}
button:hover{background:#4f46e5;transform:translateY(-1px)}
button.green{background:#059669}
button.green:hover{background:#047857}
.info-box{background:#1e3a5f;color:#93c5fd;padding:0.75rem;border-radius:6px;font-size:0.8125rem;margin-bottom:1rem}
.back{display:block;margin-top:1.5rem;color:#6366f1;text-decoration:none;font-size:0.8125rem}
.back:hover{text-decoration:underline}
</style></head>
<body><div class="container">
<a href="/" class="back">← Home</a>
<h1>📧 Mail Dashboard</h1><p class="subtitle">Send transactional emails via LogMailer</p>
<div class="cards">
<div class="card"><h2>Custom Email</h2><p class="desc">Send to any address</p>
<form action="/mail/send" method="POST">
<label>To</label><input type="email" name="to" placeholder="user@example.com" required>
<label>Subject</label><input type="text" name="subject" placeholder="Hello!" required>
<label>Body</label><textarea name="body" placeholder="Email content..."></textarea>
<label>Type</label><select name="content_type"><option value="html">HTML</option><option value="text">Plain</option></select>
<button type="submit">Send ✉️</button></form></div>
<div class="card"><h2>🎉 Welcome</h2><p class="desc">Pre-built welcome template</p><div class="info-box">Styled HTML with gradient header and CTA button.</div>
<form action="/mail/welcome" method="POST">
<label>Name</label><input type="text" name="name" placeholder="Jane Doe" required>
<label>Email</label><input type="email" name="email" placeholder="jane@example.com" required>
<button type="submit" class="green">Send Welcome 🎉</button></form></div>
<div class="card"><h2>🧾 Receipt</h2><p class="desc">Order confirmation template</p><div class="info-box">Styled with order details and green accent.</div>
<form action="/mail/receipt" method="POST">
<label>Name</label><input type="text" name="name" placeholder="John" required>
<label>Email</label><input type="email" name="email" placeholder="john@example.com" required>
<label>Order ID</label><input type="text" name="order_id" placeholder="ORD-1234" required>
<label>Amount</label><input type="text" name="amount" placeholder="$49.99" required>
<button type="submit" class="green">Send Receipt 🧾</button></form></div>
</div></div></body></html>"#;

async fn mail_dashboard() -> Response {
    Html(MAIL_DASHBOARD_HTML).into_response()
}

async fn mail_send(
    Extension(mail_manager): Extension<MailManager>,
    Extension(rate_limiter): Extension<RateLimiter>,
    Form(body): Form<SendEmailRequest>,
) -> Response {
    let email = body.to.trim().to_lowercase();
    if !email.contains('@') {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Invalid email"}))).into_response();
    }
    if rate_limiter.too_many_attempts(&email) {
        return RateLimitExceeded { retry_after: rate_limiter.retry_after(&email), limiter_name: "mail-send".to_string() }.into_response();
    }
    rate_limiter.hit(&email);

    let is_html = body.content_type.to_lowercase() == "html";
    let mut mailable = if is_html {
        Mailable::html(vec![email.clone()], &body.subject, &body.body)
    } else {
        Mailable::new(vec![email.clone()], &body.subject, &body.body)
    }.from("noreply@example.com").reply_to("support@example.com");

    if let Some(cc) = &body.cc {
        let addrs: Vec<_> = cc.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
        if !addrs.is_empty() { mailable = mailable.cc(addrs); }
    }
    if let Some(bcc) = &body.bcc {
        let addrs: Vec<_> = bcc.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
        if !addrs.is_empty() { mailable = mailable.bcc(addrs); }
    }

    match mail_manager.default_mailer() {
        Ok(mailer) => match mailer.send(mailable).await {
            Ok(()) => Json(json!({"message":"Email sent","to":email,"via":mailer.name()})).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("Send failed: {}",e)}))).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("Mailer: {}",e)}))).into_response(),
    }
}

async fn mail_welcome(
    Extension(mail_manager): Extension<MailManager>,
    Extension(rate_limiter): Extension<RateLimiter>,
    Form(body): Form<WelcomeEmailRequest>,
) -> Response {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Invalid email"}))).into_response(); }
    if body.name.trim().is_empty() { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Name required"}))).into_response(); }
    if rate_limiter.too_many_attempts(&email) {
        return RateLimitExceeded { retry_after: rate_limiter.retry_after(&email), limiter_name: "mail-send".to_string() }.into_response();
    }
    rate_limiter.hit(&email);
    let html = welcome_email_html(body.name.trim(), "Larastvel");
    let mailable = Mailable::html(vec![email.clone()], "Welcome to Larastvel! 🎉", &html).from("welcome@example.com");
    match mail_manager.default_mailer() {
        Ok(mailer) => match mailer.send(mailable).await {
            Ok(()) => Json(json!({"message":"Welcome email sent","to":email,"via":mailer.name()})).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("Send failed: {}",e)}))).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("Mailer: {}",e)}))).into_response(),
    }
}

async fn mail_receipt(
    Extension(mail_manager): Extension<MailManager>,
    Extension(rate_limiter): Extension<RateLimiter>,
    Form(body): Form<ReceiptEmailRequest>,
) -> Response {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Invalid email"}))).into_response(); }
    if rate_limiter.too_many_attempts(&email) {
        return RateLimitExceeded { retry_after: rate_limiter.retry_after(&email), limiter_name: "mail-send".to_string() }.into_response();
    }
    rate_limiter.hit(&email);
    let html = receipt_email_html(body.name.trim(), &body.order_id, &body.amount, "Larastvel");
    let subject = format!("Order #{} Confirmed ✅", body.order_id);
    let mailable = Mailable::html(vec![email.clone()], &subject, &html).from("orders@example.com").reply_to("support@example.com");
    match mail_manager.default_mailer() {
        Ok(mailer) => match mailer.send(mailable).await {
            Ok(()) => Json(json!({"message":"Receipt sent","to":email,"order_id":body.order_id,"via":mailer.name()})).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("Send failed: {}",e)}))).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("Mailer: {}",e)}))).into_response(),
    }
}

// =============================================================================
// SMS CONTROLLER (inlined)
// =============================================================================

#[derive(Debug, Deserialize)]
struct SendSmsRequest {
    phone: String,
    message: String,
    from: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SmsHistoryQuery {
    page: Option<u32>,
    per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
struct PaginatedSms {
    data: Vec<SentSmsEntry>,
    total: usize,
    page: u32,
    per_page: u32,
}

#[derive(Debug)]
struct SmsNotifiable {
    id: String,
    phone: String,
}

impl Notifiable for SmsNotifiable {
    fn notification_id(&self) -> String { self.id.clone() }
    fn route_phone(&self) -> Option<String> { Some(self.phone.clone()) }
}

#[derive(Debug, Clone)]
struct SmsDemoNotification {
    content: String,
    from: Option<String>,
}

impl Notification for SmsDemoNotification {
    fn via(&self) -> Vec<NotificationChannel> { vec![NotificationChannel::Sms] }
    fn to_sms(&self) -> Option<SmsMessage> {
        let mut msg = SmsMessage::new("", &self.content);
        if let Some(ref from) = self.from { msg = msg.from(from); }
        Some(msg)
    }
}

async fn sms_dashboard(Extension(store): Extension<SharedSmsStore>) -> Response {
    let (history, total_sent) = {
        let s = store.lock().unwrap();
        (s.all().iter().rev().take(20).cloned().collect::<Vec<_>>(), s.count())
    };

    let rows: String = history.iter().map(|e| {
        let msg = if e.message.len() > 60 { format!("{}…", &e.message[..60]) } else { e.message.clone() };
        let badge = if e.status == "sent" { r#"<span class="badge sent">Sent</span>"# } else { r#"<span class="badge failed">Failed</span>"# };
        format!(r#"<tr><td class="sid">{0}</td><td class="sto">{1}</td><td class"smsg">{2}</td><td>{3}</td><td>{4}</td></tr>"#,
            e.id, html_escape(&e.to), html_escape(&msg), badge, fmt_ts(e.sent_at))
    }).collect::<Vec<_>>().join("\n");

    let html = format!(r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>SMS Dashboard</title><style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:linear-gradient(135deg,#0f172a 0%,#1e293b 100%);min-height:100vh;padding:2rem;color:#e2e8f0}}
.container{{max-width:900px;margin:0 auto}}
h1{{font-size:2rem;font-weight:800;margin-bottom:0.5rem;background:linear-gradient(135deg,#22d3ee,#06b6d4);-webkit-background-clip:text;-webkit-text-fill-color:transparent}}
.subtitle{{color:#94a3b8;margin-bottom:2rem;font-size:1rem}}
.stats{{display:flex;gap:1rem;margin-bottom:2rem}}
.stat{{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:8px;padding:1rem 1.5rem;flex:1}}
.stat-value{{font-size:1.75rem;font-weight:700;color:#f1f5f9}}
.stat-label{{font-size:0.75rem;color:#64748b;text-transform:uppercase;letter-spacing:0.05em}}
.card{{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:12px;padding:1.5rem;backdrop-filter:blur(10px)}}
table{{width:100%;border-collapse:collapse}}
th{{text-align:left;color:#94a3b8;font-size:0.75rem;text-transform:uppercase;letter-spacing:0.05em;padding:0.75rem 0.5rem;border-bottom:1px solid #334155}}
td{{padding:0.75rem 0.5rem;border-bottom:1px solid #1e293b;font-size:0.875rem}}
.sid{{color:#64748b;font-family:monospace;font-size:0.75rem}}
.sto{{color:#f1f5f9;font-weight:600;font-family:monospace}}
.badge{{display:inline-block;padding:0.125rem 0.5rem;border-radius:999px;font-size:0.6875rem;font-weight:600}}
.badge.sent{{background:#064e3b;color:#6ee7b7}}
.badge.failed{{background:#7f1d1d;color:#fca5a5}}
label{{display:block;color:#cbd5e1;font-size:0.8125rem;font-weight:600;margin-bottom:0.25rem}}
input,textarea{{width:100%;padding:0.625rem;border:1px solid #475569;border-radius:6px;background:#0f172a;color:#e2e8f0;font-size:0.875rem;outline:none;transition:border-color 0.2s;margin-bottom:0.75rem}}
input:focus,textarea:focus{{border-color:#22d3ee}}
textarea{{resize:vertical;min-height:80px;font-family:inherit}}
.btn{{padding:0.5rem 1rem;background:#22d3ee;color:#0f172a;border:none;border-radius:6px;font-size:0.8125rem;font-weight:600;cursor:pointer;transition:all 0.2s}}
.btn:hover{{background:#06b6d4;transform:translateY(-1px)}}
.empty{{text-align:center;color:#64748b;padding:2rem;font-size:0.875rem}}
.hint{{color:#64748b;font-size:0.75rem;margin-top:-0.5rem;margin-bottom:0.75rem}}
.back{{color:#22d3ee;text-decoration:none;font-size:0.8125rem}}
.back:hover{{text-decoration:underline}}
</style></head><body><div class="container">
<a href="/" class="back">← Home</a>
<h1>📱 SMS Dashboard</h1><p class="subtitle">Send and monitor SMS via LogSmsSender</p>
<div class="stats"><div class="stat"><div class="stat-value">{0}</div><div class="stat-label">SMS Sent</div></div>
<div class="stat"><div class="stat-value">LogSmsSender</div><div class="stat-label">Sender</div></div></div>
<div class="card" style="margin-bottom:1.5rem">
<h2 style="font-size:1.125rem;margin-bottom:1rem;color:#f1f5f9;">📨 Send SMS</h2>
<form action="/api/sms/send" method="POST">
<label>Phone (E.164)</label><input type="text" name="phone" value="+15551234567" required>
<div class="hint">Format: +[country][number]</div>
<label>Sender ID</label><input type="text" name="from" value="Larastvel">
<label>Message</label><textarea name="message" required>Hello from Larastvel!</textarea>
<button type="submit" class="btn">Send SMS</button></form></div>
<div class="card"><h2 style="font-size:1.125rem;margin-bottom:1rem;color:#f1f5f9;">📋 History</h2>
<table><thead><tr><th>ID</th><th>To</th><th>Message</th><th>Status</th><th>Sent</th></tr></thead>
<tbody>{1}</tbody></table>{2}</div></div></body></html>"#,
        total_sent,
        if rows.is_empty() { r#"<tr><td colspan="5" class="empty">No SMS sent yet.</td></tr>"#.to_string() } else { rows },
        if total_sent == 0 { String::new() } else { r#"<p style="text-align:center;margin-top:1rem;color:#64748b;font-size:0.8125rem;">Showing recent 20</p>"#.to_string() }
    );
    Html(html).into_response()
}

async fn sms_send(
    Extension(store): Extension<SharedSmsStore>,
    Extension(rate_limiter): Extension<RateLimiter>,
    Form(body): Form<SendSmsRequest>,
) -> Response {
    let phone = body.phone.trim();
    if phone.is_empty() { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Phone required"}))).into_response(); }
    if !phone.starts_with('+') { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"E.164 format required: +15551234567"}))).into_response(); }
    let message = body.message.trim();
    if message.is_empty() { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Message required"}))).into_response(); }
    if message.len() > 1600 { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Message too long (max 1600)"}))).into_response(); }

    if rate_limiter.too_many_attempts(phone) {
        return RateLimitExceeded { retry_after: rate_limiter.retry_after(phone), limiter_name: "sms-send".to_string() }.into_response();
    }
    rate_limiter.hit(phone);

    let sms_sender: Arc<dyn SmsSender> = Arc::new(LogSmsSender::new());
    let sender = NotificationSender::new().with_sms_sender(sms_sender);
    let notifiable = SmsNotifiable { id: phone.to_string(), phone: phone.to_string() };
    let from = body.from.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let notification = SmsDemoNotification { content: message.to_string(), from };

    match sender.send(&notifiable, notification).await.get(&NotificationChannel::Sms) {
        Some(Ok(())) => {
            let entry = { let mut s = store.lock().unwrap(); s.add(phone.to_string(), body.from.clone().unwrap_or_else(||"Larastvel".to_string()), message.to_string(), "sent".to_string()) };
            Json(json!({"message":"SMS sent","id":entry.id,"to":phone})).into_response()
        }
        Some(Err(e)) => {
            let _ = { let mut s = store.lock().unwrap(); s.add(phone.to_string(), body.from.clone().unwrap_or_else(||"Larastvel".to_string()), message.to_string(), format!("failed: {}",e)) };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("SMS failed: {}",e)}))).into_response()
        }
        None => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"No SMS channel used"}))).into_response(),
    }
}

async fn sms_history(
    Extension(store): Extension<SharedSmsStore>,
    Query(query): Query<SmsHistoryQuery>,
) -> Json<PaginatedSms> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let all = { let s = store.lock().unwrap(); s.all().iter().rev().cloned().collect::<Vec<_>>() };
    let total = all.len();
    let offset = ((page.saturating_sub(1)) * per_page) as usize;
    let data = all.into_iter().skip(offset).take(per_page as usize).collect();
    Json(PaginatedSms { data, total, page, per_page })
}

// =============================================================================
// BROADCAST CONTROLLER (inlined)
// =============================================================================

#[derive(Debug, Deserialize)]
struct SendBroadcastRequest {
    channel: String,
    event: String,
    data: String,
    driver: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BroadcastLogQuery {
    page: Option<u32>,
    per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
struct PaginatedBroadcastLog {
    data: Vec<BroadcastLogEntry>,
    total: usize,
    page: u32,
    per_page: u32,
}

#[derive(Debug)]
struct BroadcastNotifiable {
    id: String,
}

impl Notifiable for BroadcastNotifiable {
    fn notification_id(&self) -> String { self.id.clone() }
    fn route_broadcast_channels(&self) -> Vec<String> { vec![self.id.clone()] }
}

#[derive(Debug, Clone)]
struct BroadcastDemoNotification {
    event: String,
    data: serde_json::Value,
}

impl Notification for BroadcastDemoNotification {
    fn via(&self) -> Vec<NotificationChannel> { vec![NotificationChannel::Broadcast] }
    fn to_broadcast(&self) -> Option<larastvel_core::notifications::BroadcastPayload> {
        Some(larastvel_core::notifications::BroadcastPayload {
            event: self.event.clone(),
            data: self.data.clone(),
        })
    }
}

async fn broadcast_dashboard(Extension(log): Extension<SharedBroadcastLog>) -> Response {
    let entries = { let l = log.lock().unwrap(); l.all().iter().rev().take(20).cloned().collect::<Vec<_>>() };
    let total = { let l = log.lock().unwrap(); l.count() };

    let rows: String = entries.iter().map(|e| {
        let data_trunc = if e.data.len() > 50 { format!("{}…", &e.data[..50]) } else { e.data.clone() };
        let driver_badge = match e.driver.as_str() {
            "pusher" => r#"<span class="badge pusher">Pusher</span>"#,
            "ably" => r#"<span class="badge ably">Ably</span>"#,
            _ => r#"<span class="badge log">Log</span>"#,
        };
        format!(r#"<tr><td>{0}</td><td>{1}</td><td><span class="badge event">{2}</span></td><td>{3}</td><td>{4}</td><td>{5}</td></tr>"#,
            e.id, html_escape(&e.channel), html_escape(&e.event), html_escape(&data_trunc), driver_badge, fmt_ts(e.sent_at))
    }).collect::<Vec<_>>().join("\n");

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Broadcast Dashboard</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:linear-gradient(135deg,#0f172a 0%,#1e293b 100%);min-height:100vh;padding:2rem;color:#e2e8f0}}
.container{{max-width:900px;margin:0 auto}}
h1{{font-size:2rem;font-weight:800;margin-bottom:0.5rem;background:linear-gradient(135deg,#f43f5e,#ec4899);-webkit-background-clip:text;-webkit-text-fill-color:transparent}}
.subtitle{{color:#94a3b8;margin-bottom:2rem;font-size:1rem}}
.stats{{display:flex;gap:1rem;margin-bottom:2rem}}
.stat{{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:8px;padding:1rem 1.5rem;flex:1}}
.stat-value{{font-size:1.75rem;font-weight:700;color:#f1f5f9}}
.stat-label{{font-size:0.75rem;color:#64748b;text-transform:uppercase;letter-spacing:0.05em}}
.card{{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:12px;padding:1.5rem;backdrop-filter:blur(10px)}}
table{{width:100%;border-collapse:collapse}}
th{{text-align:left;color:#94a3b8;font-size:0.75rem;text-transform:uppercase;letter-spacing:0.05em;padding:0.75rem 0.5rem;border-bottom:1px solid #334155}}
td{{padding:0.75rem 0.5rem;border-bottom:1px solid #1e293b;font-family:monospace;font-size:0.8125rem}}
select{{width:100%;padding:0.625rem;border:1px solid #475569;border-radius:6px;background:#0f172a;color:#e2e8f0;font-size:0.875rem;outline:none;transition:border-color 0.2s;margin-bottom:0.75rem;cursor:pointer}}
select:focus{{border-color:#ec4899}}
.badge.event{{display:inline-block;padding:0.125rem 0.5rem;border-radius:999px;font-size:0.6875rem;font-weight:600;background:#312e81;color:#a5b4fc}}
.badge.log{{display:inline-block;padding:0.125rem 0.5rem;border-radius:999px;font-size:0.6875rem;font-weight:600;background:#1e293b;color:#94a3b8}}
.badge.pusher{{display:inline-block;padding:0.125rem 0.5rem;border-radius:999px;font-size:0.6875rem;font-weight:600;background:#1e3a5f;color:#60a5fa}}
.badge.ably{{display:inline-block;padding:0.125rem 0.5rem;border-radius:999px;font-size:0.6875rem;font-weight:600;background:#3b0764;color:#c084fc}}
label{{display:block;color:#cbd5e1;font-size:0.8125rem;font-weight:600;margin-bottom:0.25rem}}
input,textarea{{width:100%;padding:0.625rem;border:1px solid #475569;border-radius:6px;background:#0f172a;color:#e2e8f0;font-size:0.875rem;outline:none;transition:border-color 0.2s;margin-bottom:0.75rem}}
input:focus,textarea:focus{{border-color:#ec4899}}
textarea{{resize:vertical;min-height:60px;font-family:inherit}}
.btn{{padding:0.5rem 1rem;background:#ec4899;color:#fff;border:none;border-radius:6px;font-size:0.8125rem;font-weight:600;cursor:pointer;transition:all 0.2s}}
.btn:hover{{background:#db2777;transform:translateY(-1px)}}
.empty{{text-align:center;color:#64748b;padding:2rem;font-size:0.875rem}}
.hint{{color:#64748b;font-size:0.75rem;margin-top:-0.5rem;margin-bottom:0.75rem}}
.back{{color:#f43f5e;text-decoration:none;font-size:0.8125rem}}.back:hover{{text-decoration:underline}}
</style></head>
<body><div class="container">
<a href="/" class="back">← Home</a>
<h1>📡 Broadcast Dashboard</h1>
<p class="subtitle">Send real-time events with driver selection</p>
<div class="stats"><div class="stat"><div class="stat-value">{0}</div><div class="stat-label">Events Sent</div></div>
<div class="stat"><div class="stat-value">Log / Pusher / Ably</div><div class="stat-label">Drivers</div></div></div>
<div class="card" style="margin-bottom:1.5rem">
<h2 style="font-size:1.125rem;margin-bottom:1rem;color:#f1f5f9;">📨 Send Broadcast Event</h2>
<form action="/api/broadcast/send" method="POST">
<label>Channel</label><input type="text" name="channel" value="user.1" placeholder="user.1">
<label>Event Name</label><input type="text" name="event" value="order.shipped" placeholder="order.shipped">
<label>Data (JSON)</label><textarea name="data">{{{{"order_id": "ORD-1234", "status": "shipped"}}}}</textarea>
<label>Broadcast Driver</label>
<select name="driver">
<option value="log" selected>🔌 LogBroadcaster (dev)</option>
<option value="pusher">📡 PusherBroadcaster (real)</option>
<option value="ably">⚡ AblyBroadcaster (real)</option>
</select>
<div class="hint">LogBroadcaster logs events. Pusher/Ably send via their REST APIs (configure APP_ID/KEY in build_app).</div>
<button type="submit" class="btn">Broadcast Event 📡</button></form></div>
<div class="card"><h2 style="font-size:1.125rem;margin-bottom:1rem;color:#f1f5f9;">📋 Event Log</h2>
<table><thead><tr><th>ID</th><th>Channel</th><th>Event</th><th>Data</th><th>Driver</th><th>Sent</th></tr></thead>
<tbody>{1}</tbody></table>{2}</div></div></body></html>"#,
        total,
        if rows.is_empty() { r#"<tr><td colspan="6" class="empty">No events broadcast yet.</td></tr>"#.to_string() } else { rows },
        if total == 0 { String::new() } else { r#"<p style="text-align:center;margin-top:1rem;color:#64748b;font-size:0.8125rem;">Showing recent 20</p>"#.to_string() }
    );
    Html(html).into_response()
}

async fn broadcast_send(
    Extension(manager): Extension<BroadcastManager>,
    Extension(log): Extension<SharedBroadcastLog>,
    Extension(rate_limiter): Extension<RateLimiter>,
    Form(body): Form<SendBroadcastRequest>,
) -> Response {
    let channel = body.channel.trim().to_string();
    if channel.is_empty() { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Channel required"}))).into_response(); }
    let event = body.event.trim().to_string();
    if event.is_empty() { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Event required"}))).into_response(); }

    if rate_limiter.too_many_attempts(&channel) {
        return RateLimitExceeded { retry_after: rate_limiter.retry_after(&channel), limiter_name: "broadcast-send".to_string() }.into_response();
    }
    rate_limiter.hit(&channel);

    let data: serde_json::Value = match serde_json::from_str(&body.data) {
        Ok(v) => v,
        Err(_) => serde_json::json!({ "text": body.data }),
    };

    // Select the broadcast driver (default: "log")
    let driver_name = body.driver.as_deref().unwrap_or("log");
    let broadcaster = match manager.broadcaster(driver_name) {
        Ok(b) => b,
        Err(_) => {
            let available = manager.broadcaster_names().join(", ");
            return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({
                "error": format!("Unknown driver '{}'. Available: {}", driver_name, available)
            }))).into_response();
        }
    };

    let notifiable = BroadcastNotifiable { id: channel.clone() };
    let notification = BroadcastDemoNotification { event: event.clone(), data: data.clone() };
    let sender = NotificationSender::new().with_broadcaster(broadcaster);

    let results = sender.send(&notifiable, notification).await;
    match results.get(&NotificationChannel::Broadcast) {
        Some(Ok(())) => {
            let entry = { let mut l = log.lock().unwrap(); l.add(channel.clone(), event.clone(), data.to_string(), driver_name.to_string()) };
            Json(json!({"message":"Event broadcast","id":entry.id,"event":event,"channel":channel,"driver":driver_name})).into_response()
        }
        Some(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("Broadcast failed: {}",e)}))).into_response(),
        None => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"No broadcast channel used"}))).into_response(),
    }
}

async fn broadcast_log_list(
    Extension(log): Extension<SharedBroadcastLog>,
    Query(query): Query<BroadcastLogQuery>,
) -> Json<PaginatedBroadcastLog> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let all = { let l = log.lock().unwrap(); l.all().iter().rev().cloned().collect::<Vec<_>>() };
    let total = all.len();
    let offset = ((page.saturating_sub(1)) * per_page) as usize;
    let data = all.into_iter().skip(offset).take(per_page as usize).collect();
    Json(PaginatedBroadcastLog { data, total, page, per_page })
}

// =============================================================================
// NOTIFICATION CONTROLLER (inlined)
// =============================================================================

#[derive(Debug, Deserialize)]
struct SendNotifRequest {
    notifiable_id: String,
    title: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct ListNotifQuery {
    page: Option<u32>,
    per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
struct NotifResponse {
    id: String,
    notifiable_id: String,
    notifiable_type: String,
    notification_type: String,
    data: serde_json::Value,
    read_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
struct PaginatedNotifs {
    data: Vec<NotifResponse>,
    total: i64,
    page: u32,
    per_page: u32,
    unread_count: i64,
}

#[derive(Debug)]
struct ApiNotifiable {
    id: String,
}

impl Notifiable for ApiNotifiable {
    fn notification_id(&self) -> String { self.id.clone() }
}

#[derive(Debug, Clone)]
struct DemoNotif {
    data: serde_json::Value,
}

impl Notification for DemoNotif {
    fn via(&self) -> Vec<NotificationChannel> { vec![NotificationChannel::Database] }
    fn to_database(&self) -> Option<serde_json::Value> { Some(self.data.clone()) }
}

async fn notif_dashboard(Extension(db): Extension<sea_orm::DatabaseConnection>) -> Response {
    let total = fetch_total(&db).await;
    let unread = fetch_unread(&db).await;
    let recent = fetch_notifications(&db, 1, 10).await;

    let rows: String = recent.iter().map(|n| {
        let title = n.data.get("title").and_then(|v|v.as_str()).unwrap_or("(no title)");
        let badge = if n.read_at.is_some() { r#"<span class="badge read">Read</span>"# } else { r#"<span class="badge unread">Unread</span>"# };
        let short_id: String = n.id.chars().take(8).collect();
        format!(r#"<tr><td class="nid">{0}…</td><td class="ntitle">{1}</td><td>{2}</td><td>{3}</td><td><form action="/api/notifications/{0}/read" method="POST" style="display:inline"><button class="btn-sm">Read</button></form></td></tr>"#,
            short_id, html_escape(title), badge, fmt_ts(n.created_at))
    }).collect::<Vec<_>>().join("\n");

    let html = format!(r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Notifications</title><style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:linear-gradient(135deg,#0f172a 0%,#1e293b 100%);min-height:100vh;padding:2rem;color:#e2e8f0}}
.container{{max-width:900px;margin:0 auto}}
h1{{font-size:2rem;font-weight:800;margin-bottom:0.5rem;background:linear-gradient(135deg,#6366f1,#8b5cf6);-webkit-background-clip:text;-webkit-text-fill-color:transparent}}
.subtitle{{color:#94a3b8;margin-bottom:2rem;font-size:1rem}}
.stats{{display:flex;gap:1rem;margin-bottom:2rem}}
.stat{{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:8px;padding:1rem 1.5rem;flex:1}}
.stat-value{{font-size:1.75rem;font-weight:700;color:#f1f5f9}}
.stat-label{{font-size:0.75rem;color:#64748b;text-transform:uppercase;letter-spacing:0.05em}}
.stat.unread .stat-value{{color:#f59e0b}}
.card{{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:12px;padding:1.5rem;backdrop-filter:blur(10px)}}
table{{width:100%;border-collapse:collapse}}
th{{text-align:left;color:#94a3b8;font-size:0.75rem;text-transform:uppercase;letter-spacing:0.05em;padding:0.75rem 0.5rem;border-bottom:1px solid #334155}}
td{{padding:0.75rem 0.5rem;border-bottom:1px solid #1e293b;font-size:0.875rem}}
.nid{{color:#64748b;font-family:monospace;font-size:0.75rem}}
.ntitle{{color:#e2e8f0}}
.badge{{display:inline-block;padding:0.125rem 0.5rem;border-radius:999px;font-size:0.6875rem;font-weight:600}}
.badge.read{{background:#064e3b;color:#6ee7b7}}
.badge.unread{{background:#451a03;color:#fbbf24}}
.btn-sm{{padding:0.25rem 0.625rem;background:#6366f1;color:#fff;border:none;border-radius:4px;font-size:0.75rem;cursor:pointer;transition:background 0.2s}}
.btn-sm:hover{{background:#4f46e5}}
.btn{{padding:0.5rem 1rem;background:#6366f1;color:#fff;border:none;border-radius:6px;font-size:0.8125rem;font-weight:600;cursor:pointer;transition:all 0.2s}}
.btn:hover{{background:#4f46e5;transform:translateY(-1px)}}
.btn-group{{display:flex;gap:0.5rem;margin-bottom:1rem;align-items:center}}
label{{display:block;color:#cbd5e1;font-size:0.8125rem;font-weight:600;margin-bottom:0.25rem}}
input,textarea{{width:100%;padding:0.625rem;border:1px solid #475569;border-radius:6px;background:#0f172a;color:#e2e8f0;font-size:0.875rem;outline:none;transition:border-color 0.2s;margin-bottom:0.75rem}}
input:focus,textarea:focus{{border-color:#6366f1}}
textarea{{resize:vertical;min-height:60px;font-family:inherit}}
.empty{{text-align:center;color:#64748b;padding:2rem;font-size:0.875rem}}
.back{{color:#8b5cf6;text-decoration:none;font-size:0.8125rem}}
.back:hover{{text-decoration:underline}}
</style></head><body><div class="container">
<a href="/" class="back">← Home</a>
<h1>🔔 Notifications</h1><p class="subtitle">Database-backed notifications</p>
<div class="stats"><div class="stat"><div class="stat-value">{1}</div><div class="stat-label">Total</div></div>
<div class="stat unread"><div class="stat-value">{0}</div><div class="stat-label">Unread</div></div></div>
<div class="card" style="margin-bottom:1.5rem">
<h2 style="font-size:1.125rem;margin-bottom:1rem;color:#f1f5f9;">📨 Send</h2>
<form action="/api/notifications/send" method="POST">
<label>Notifiable ID</label><input type="text" name="notifiable_id" value="user-1" required>
<label>Title</label><input type="text" name="title" placeholder="Welcome!" required>
<label>Body</label><textarea name="body" placeholder="Notification text..."></textarea>
<button type="submit" class="btn">Send</button></form></div>
<div class="card"><div class="btn-group"><h2 style="font-size:1.125rem;color:#f1f5f9;flex:1">📋 Recent</h2>
<form action="/api/notifications/read-all" method="POST"><button class="btn">Mark All Read</button></form></div>
<table><thead><tr><th>ID</th><th>Title</th><th>Status</th><th>Created</th><th>Action</th></tr></thead>
<tbody>{2}</tbody></table>{3}</div></div></body></html>"#,
        unread, total,
        if rows.is_empty() { r#"<tr><td colspan="5" class="empty">No notifications yet.</td></tr>"#.to_string() } else { rows },
        if recent.is_empty() { String::new() } else { r#"<p style="text-align:center;margin-top:1rem;color:#64748b;font-size:0.8125rem;">Showing 10 recent</p>"#.to_string() }
    );
    Html(html).into_response()
}

async fn notif_list(
    Extension(db): Extension<sea_orm::DatabaseConnection>,
    Query(query): Query<ListNotifQuery>,
) -> Json<PaginatedNotifs> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    Json(fetch_paginated(&db, page, per_page).await)
}

async fn notif_list_unread(
    Extension(db): Extension<sea_orm::DatabaseConnection>,
    Query(query): Query<ListNotifQuery>,
) -> Json<PaginatedNotifs> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    Json(fetch_paginated_unread(&db, page, per_page).await)
}

async fn notif_send(
    Extension(db): Extension<sea_orm::DatabaseConnection>,
    Extension(rate_limiter): Extension<RateLimiter>,
    Form(body): Form<SendNotifRequest>,
) -> Response {
    if body.title.trim().is_empty() { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"Title required"}))).into_response(); }
    if body.notifiable_id.trim().is_empty() { return (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error":"ID required"}))).into_response(); }
    if rate_limiter.too_many_attempts(&body.notifiable_id) {
        return RateLimitExceeded { retry_after: rate_limiter.retry_after(&body.notifiable_id), limiter_name: "notif-send".to_string() }.into_response();
    }
    rate_limiter.hit(&body.notifiable_id);
    let sender = NotificationSender::new().with_database(db);
    let data = json!({"title": body.title, "body": body.body});
    match sender.send(&ApiNotifiable { id: body.notifiable_id.clone() }, DemoNotif { data }).await.get(&NotificationChannel::Database) {
        Some(Ok(())) => Json(json!({"message":"Notification sent","notifiable_id":body.notifiable_id})).into_response(),
        Some(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("Failed: {}",e)}))).into_response(),
        None => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"No DB channel"}))).into_response(),
    }
}

async fn notif_mark_read(
    Extension(db): Extension<sea_orm::DatabaseConnection>,
    Path(id): Path<String>,
) -> Response {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    match db.execute(Statement::from_sql_and_values(DatabaseBackend::Sqlite, "UPDATE notifications SET read_at=?1,updated_at=?1 WHERE id=?2", [now.into(), id.clone().into()])).await {
        Ok(r) if r.rows_affected() > 0 => Json(json!({"message":"Marked as read","id":id})).into_response(),
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({"error":"Not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("DB error: {}",e)}))).into_response(),
    }
}

async fn notif_mark_all_read(Extension(db): Extension<sea_orm::DatabaseConnection>) -> Response {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    match db.execute(Statement::from_sql_and_values(DatabaseBackend::Sqlite, "UPDATE notifications SET read_at=?1,updated_at=?1 WHERE read_at IS NULL", [now.into()])).await {
        Ok(r) => Json(json!({"message":format!("{} marked read", r.rows_affected()), "count": r.rows_affected()})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":format!("DB error: {}",e)}))).into_response(),
    }
}

// --- Notification DB helpers ---

async fn fetch_total(db: &sea_orm::DatabaseConnection) -> i64 {
    db.query_one(Statement::from_string(DatabaseBackend::Sqlite, "SELECT COUNT(*) FROM notifications")).await
        .ok().flatten().and_then(|r| r.try_get_by_index::<i64>(0).ok()).unwrap_or(0)
}

async fn fetch_unread(db: &sea_orm::DatabaseConnection) -> i64 {
    db.query_one(Statement::from_string(DatabaseBackend::Sqlite, "SELECT COUNT(*) FROM notifications WHERE read_at IS NULL")).await
        .ok().flatten().and_then(|r| r.try_get_by_index::<i64>(0).ok()).unwrap_or(0)
}

async fn fetch_notifications(db: &sea_orm::DatabaseConnection, page: u32, per_page: u32) -> Vec<NotifResponse> {
    let offset = ((page.saturating_sub(1)) * per_page) as i64;
    let sql = format!("SELECT id,notifiable_id,notifiable_type,notification_type,data,read_at,created_at,updated_at FROM notifications ORDER BY created_at DESC LIMIT {} OFFSET {}", per_page, offset);
    query_notifs(db, &sql).await
}

async fn fetch_paginated(db: &sea_orm::DatabaseConnection, page: u32, per_page: u32) -> PaginatedNotifs {
    let total = fetch_total(db).await;
    let unread = fetch_unread(db).await;
    let offset = ((page.saturating_sub(1)) * per_page) as i64;
    let sql = format!("SELECT id,notifiable_id,notifiable_type,notification_type,data,read_at,created_at,updated_at FROM notifications ORDER BY created_at DESC LIMIT {} OFFSET {}", per_page, offset);
    let data = query_notifs(db, &sql).await;
    PaginatedNotifs { data, total, page, per_page, unread_count: unread }
}

async fn fetch_paginated_unread(db: &sea_orm::DatabaseConnection, page: u32, per_page: u32) -> PaginatedNotifs {
    let unread = fetch_unread(db).await;
    let offset = ((page.saturating_sub(1)) * per_page) as i64;
    let sql = format!("SELECT id,notifiable_id,notifiable_type,notification_type,data,read_at,created_at,updated_at FROM notifications WHERE read_at IS NULL ORDER BY created_at DESC LIMIT {} OFFSET {}", per_page, offset);
    let data = query_notifs(db, &sql).await;
    PaginatedNotifs { data, total: unread, page, per_page, unread_count: unread }
}

async fn query_notifs(db: &sea_orm::DatabaseConnection, sql: &str) -> Vec<NotifResponse> {
    match db.query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string())).await {
        Ok(rows) => rows.iter().map(|row| {
            let ds: String = row.try_get_by_index(4).unwrap_or_default();
            NotifResponse {
                id: row.try_get_by_index(0).unwrap_or_default(),
                notifiable_id: row.try_get_by_index(1).unwrap_or_default(),
                notifiable_type: row.try_get_by_index(2).unwrap_or_default(),
                notification_type: row.try_get_by_index(3).unwrap_or_default(),
                data: serde_json::from_str(&ds).unwrap_or(serde_json::Value::Null),
                read_at: row.try_get_by_index::<Option<i64>>(5).ok().flatten(),
                created_at: row.try_get_by_index(6).unwrap_or(0),
                updated_at: row.try_get_by_index(7).unwrap_or(0),
            }
        }).collect(),
        Err(_) => vec![],
    }
}

// =============================================================================
// HELPERS
// =============================================================================

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn fmt_ts(unix_secs: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let diff = now - unix_secs;
    if diff < 60 { "just now".to_string() } else if diff < 3600 { format!("{}m ago", diff/60) } else if diff < 86400 { format!("{}h ago", diff/3600) } else { format!("{}d ago", diff/86400) }
}

// =============================================================================
// ENTRY POINT
// =============================================================================

fn main() {
    println!("Unified Dashboard — combines Mail, Broadcast, SMS, and Notification controllers.");
    println!();
    println!("17 routes across 4 channels. Run tests: cargo test --example unified_dashboard");
    println!();
    println!("Architecture:");
    println!("  DB (SQLite in-memory) → notifications");
    println!("  SMS Store (Arc<Mutex>) → SMS history");
    println!("  Broadcast Log (Arc<Mutex>) → event log");
    println!("  MailManager (LogMailer) → email");
    println!("  BroadcastManager → LogBroadcaster / PusherBroadcaster / AblyBroadcaster");
    println!("  Independent rate limiters per channel");
    println!();
    println!("To build and serve:");
    println!("  let (app, _state) = unified_dashboard::build_app().await;");
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use larastvel_core::axum::body::Body;
    use larastvel_core::axum::http::Request;
    use tower::ServiceExt;

    // -------------------------------------------------------------------------
    // App initialization
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_build_app_creates_tables() {
        let (_app, state) = build_app().await;
        let row = state.db.query_one(Statement::from_string(DatabaseBackend::Sqlite, "SELECT name FROM sqlite_master WHERE type='table' AND name='notifications'")).await.unwrap();
        assert!(row.is_some(), "notifications table should exist");
        assert_eq!(state.sms_store.lock().unwrap().count(), 0);
        assert_eq!(state.broadcast_log.lock().unwrap().count(), 0);
    }

    // -------------------------------------------------------------------------
    // Home dashboard
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_home_returns_html_with_links() {
        let (app, _) = build_app().await;
        let resp = app.clone().oneshot(Request::builder().method("GET").uri("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp.headers().get("content-type").and_then(|v|v.to_str().ok()).unwrap_or("").contains("text/html"));

        let bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let html = String::from_utf8_lossy(&bytes);
        assert!(html.contains("/mail/test"));
        assert!(html.contains("/sms"));
        assert!(html.contains("/notifications"));
        assert!(html.contains("Notification Hub"));
    }

    // -------------------------------------------------------------------------
    // All dashboard routes accessible
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_all_dashboards_accessible() {
        let (app, _) = build_app().await;
        for route in ["/", "/mail/test", "/broadcast", "/sms", "/notifications"] {
            let resp = app.clone().oneshot(Request::builder().method("GET").uri(route).body(Body::empty()).unwrap()).await.unwrap();
            assert_eq!(resp.status(), 200, "GET {} should return 200", route);
        }
    }

    // -------------------------------------------------------------------------
    // SMS and database notifications are independent
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sms_does_not_create_db_notifications() {
        let (app, state) = build_app().await;
        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/sms/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("phone=%2B15551234567&message=Test&from=App")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);

        assert_eq!(state.sms_store.lock().unwrap().count(), 1);
        let cnt: i64 = state.db.query_one(Statement::from_string(DatabaseBackend::Sqlite, "SELECT COUNT(*) FROM notifications")).await.unwrap().unwrap().try_get_by_index(0).unwrap();
        assert_eq!(cnt, 0, "SMS should not create DB notifications");
    }

    #[tokio::test]
    async fn test_db_notification_does_not_affect_sms() {
        let (app, state) = build_app().await;
        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/notifications/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("notifiable_id=u1&title=Test&body=Hello")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);

        assert_eq!(state.sms_store.lock().unwrap().count(), 0, "DB notif should not affect SMS store");
        let cnt: i64 = state.db.query_one(Statement::from_string(DatabaseBackend::Sqlite, "SELECT COUNT(*) FROM notifications")).await.unwrap().unwrap().try_get_by_index(0).unwrap();
        assert_eq!(cnt, 1);
    }

    // -------------------------------------------------------------------------
    // Rate limiters are independent
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_rate_limiters_independent() {
        use larastvel_core::axum::Router as AxRouter;
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        NotificationSender::new().with_database(db.clone()).ensure_notifications_table().await.unwrap();
        let sms_store = new_sms_store();
        let mut mm = MailManager::new("log");
        mm.register("log", LogMailer::new("log"));

        let nl = notification_rate_limiter();
        let sl = sms_rate_limiter();
        let ml = mail_rate_limiter();

        for _ in 0..5 { nl.hit("blocked-user"); }

        let app = AxRouter::new()
            .route("/api/notifications/send", routing::post(notif_send))
            .route("/api/sms/send", routing::post(sms_send))
            .route("/mail/send", routing::post(mail_send))
            .layer(Extension(db)).layer(Extension(sms_store))
            .layer(Extension(mm)).layer(Extension(nl)).layer(Extension(sl)).layer(Extension(ml));

        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/notifications/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("notifiable_id=blocked-user&title=X&body=Y")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 429, "Notif blocked");

        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/sms/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("phone=%2B15551234567&message=Ok&from=T")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200, "SMS allowed (independent)");

        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/mail/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("to=test%40example.com&subject=Hi&body=test&content_type=text")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200, "Mail allowed (independent)");
    }

    // -------------------------------------------------------------------------
    // Full notification lifecycle
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_notification_lifecycle() {
        let (app, _) = build_app().await;
        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/notifications/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("notifiable_id=lc&title=Lifecycle&body=Test")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);

        let resp = app.clone().oneshot(Request::builder().method("GET").uri("/api/notifications").body(Body::empty()).unwrap()).await.unwrap();
        let body = larastvel_core::axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list["total"], 1);
        let nid = list["data"][0]["id"].as_str().unwrap().to_string();

        let resp = app.clone().oneshot(Request::builder().method("POST").uri(&format!("/api/notifications/{}/read", nid)).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);

        let resp = app.clone().oneshot(Request::builder().method("GET").uri("/api/notifications/unread").body(Body::empty()).unwrap()).await.unwrap();
        let body = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let unread: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(unread["total"], 0);
    }

    // -------------------------------------------------------------------------
    // All three channels work simultaneously
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_all_three_channels() {
        let (app, state) = build_app().await;

        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/mail/welcome")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("name=Alice&email=alice%40example.com")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);

        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/sms/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("phone=%2B15551234567&message=Hi&from=App")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);

        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/notifications/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("notifiable_id=u1&title=Done&body=All")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);

        assert_eq!(state.sms_store.lock().unwrap().count(), 1);
        let cnt: i64 = state.db.query_one(Statement::from_string(DatabaseBackend::Sqlite, "SELECT COUNT(*) FROM notifications")).await.unwrap().unwrap().try_get_by_index(0).unwrap();
        assert_eq!(cnt, 1);
    }

    // -------------------------------------------------------------------------
    // SMS store persists across requests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sms_store_persistence() {
        let (app, state) = build_app().await;
        for i in 0..3 {
            let body = format!("phone=%2B1555{:04}0000&message=msg{}&from=T", i, i);
            let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/sms/send")
                .header("content-type","application/x-www-form-urlencoded").body(Body::from(body)).unwrap()).await.unwrap();
            assert_eq!(resp.status(), 200);
        }
        assert_eq!(state.sms_store.lock().unwrap().count(), 3);

        let resp = app.clone().oneshot(Request::builder().method("GET").uri("/api/sms/history").body(Body::empty()).unwrap()).await.unwrap();
        let body = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 3);
    }

    // -------------------------------------------------------------------------
    // Broadcast channel
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_broadcast_dashboard_html() {
        let (app, _) = build_app().await;
        let resp = app.clone().oneshot(Request::builder().method("GET").uri("/broadcast").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp.headers().get("content-type").and_then(|v|v.to_str().ok()).unwrap_or("").contains("text/html"));
    }

    #[tokio::test]
    async fn test_broadcast_send_event() {
        let (app, state) = build_app().await;
        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/broadcast/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("channel=user.1&event=order.shipped&data=%7B%22order_id%22%3A%22ORD-1%22%7D")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(state.broadcast_log.lock().unwrap().count(), 1);
    }

    #[tokio::test]
    async fn test_broadcast_event_log() {
        let (app, state) = build_app().await;
        // Send 3 events
        for i in 0..3 {
            let data = format!("%7B%22i%22%3A{}%7D", i);
            let body = format!("channel=user.{}&event=test&data={}", i, data);
            app.clone().oneshot(Request::builder().method("POST").uri("/api/broadcast/send")
                .header("content-type","application/x-www-form-urlencoded").body(Body::from(body)).unwrap()).await.unwrap();
        }
        assert_eq!(state.broadcast_log.lock().unwrap().count(), 3);

        let resp = app.clone().oneshot(Request::builder().method("GET").uri("/api/broadcast/log").body(Body::empty()).unwrap()).await.unwrap();
        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["total"], 3);
    }

    #[tokio::test]
    async fn test_broadcast_does_not_affect_other_stores() {
        let (app, state) = build_app().await;
        let resp = app.clone().oneshot(Request::builder().method("POST").uri("/api/broadcast/send")
            .header("content-type","application/x-www-form-urlencoded")
            .body(Body::from("channel=test&event=ping&data=%7B%7D")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 200);

        assert_eq!(state.broadcast_log.lock().unwrap().count(), 1);
        assert_eq!(state.sms_store.lock().unwrap().count(), 0, "Broadcast should not affect SMS store");
        let cnt: i64 = state.db.query_one(Statement::from_string(DatabaseBackend::Sqlite, "SELECT COUNT(*) FROM notifications")).await.unwrap().unwrap().try_get_by_index(0).unwrap();
        assert_eq!(cnt, 0, "Broadcast should not create DB notifications");
    }

    // -------------------------------------------------------------------------
    // 404 for unknown routes
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_unknown_route_returns_404() {
        let (app, _) = build_app().await;
        let resp = app.clone().oneshot(Request::builder().method("GET").uri("/nonexistent").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 404);
    }

    // -------------------------------------------------------------------------
    // Mail templates
    // -------------------------------------------------------------------------

    #[test]
    fn test_mail_templates() {
        let w = welcome_email_html("TestUser", "MyApp");
        assert!(w.contains("TestUser") && w.contains("MyApp"));

        let r = receipt_email_html("Customer", "ORD-1", "$10", "Shop");
        assert!(r.contains("ORD-1") && r.contains("$10"));
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>alert('x')</script>"), "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;");
    }

    #[test]
    fn test_fmt_ts() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        assert_eq!(fmt_ts(now), "just now");
    }
}
