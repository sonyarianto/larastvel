//! # NotificationController Example
//!
//! A REST API for managing database-backed notifications, demonstrating how
//! to send, list, and mark notifications as read using the notifications
//! module with a `notifications` database table.
//!
//! ## Routes
//!
//! | Method | URI                             | Description                          |
//! |--------|---------------------------------|--------------------------------------|
//! | GET    | `/notifications`                | Show notification dashboard (HTML)   |
//! | GET    | `/api/notifications`            | List all notifications (JSON)        |
//! | GET    | `/api/notifications/unread`     | List unread notifications (JSON)     |
//! | POST   | `/api/notifications/send`       | Send a notification (demo endpoint)  |
//! | POST   | `/api/notifications/{id}/read`  | Mark a notification as read          |
//! | POST   | `/api/notifications/read-all`   | Mark all notifications as read       |
//!
//! ## Integration
//!
//! ```ignore
//! use larastvel_core::routing::Registrar;
//!
//! let router = app.router();
//! notification_controller::NotificationController::register_routes(&router);
//! ```
//!
//! The controller expects a `sea_orm::DatabaseConnection` in Axum's
//! extensions, and requires the `notifications` table to exist (call
//! `ensure_notifications_table()` during boot, or it auto-creates on
//! the first send).

#![allow(unused_imports, dead_code)]

use std::sync::Arc;

use larastvel_core::axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    Form, Router,
};
use larastvel_core::notifications::{
    BroadcastPayload, DatabaseNotification, Notifiable, Notification, NotificationChannel,
    NotificationError, NotificationSender,
};
use larastvel_core::rate_limiter::{RateLimitConfig, RateLimitExceeded, RateLimiter};
use larastvel_core::routing::Registrar;
use larastvel_core::sea_orm;
use larastvel_core::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use larastvel_core::serde::{Deserialize, Serialize};
use larastvel_core::serde_json::{self, json};

// =============================================================================
// REQUEST / RESPONSE TYPES
// =============================================================================

/// Payload for sending a notification via the demo endpoint.
#[derive(Debug, Deserialize)]
pub struct SendNotificationRequest {
    pub notifiable_id: String,
    pub notifiable_type: Option<String>,
    pub title: String,
    pub body: String,
}

/// Query parameters for listing notifications.
#[derive(Debug, Deserialize)]
pub struct ListNotificationsQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// A JSON-serializable notification for API responses.
#[derive(Debug, Serialize)]
pub struct NotificationResponse {
    pub id: String,
    pub notifiable_id: String,
    pub notifiable_type: String,
    pub notification_type: String,
    pub data: serde_json::Value,
    pub read_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Paginated list response.
#[derive(Debug, Serialize)]
pub struct PaginatedNotifications {
    pub data: Vec<NotificationResponse>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
    pub unread_count: i64,
}

/// Success message envelope.
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// =============================================================================
// RATE LIMITER
// =============================================================================

/// Create a rate limiter for the notification send endpoint.
///
/// Limits to 5 sends per minute per notifiable to prevent abuse.
pub fn notification_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimitConfig::per_minute(5).named("notification-send"))
}

// =============================================================================
// NOTIFIABLE HELPERS
// =============================================================================

/// A simple notifiable implementation used by this controller's demo endpoint.
#[derive(Debug)]
pub struct ApiNotifiable {
    id: String,
    email: Option<String>,
}

impl Notifiable for ApiNotifiable {
    fn notification_id(&self) -> String {
        self.id.clone()
    }

    fn route_email(&self) -> Option<String> {
        self.email.clone()
    }
}

// =============================================================================
// CONTROLLER
// =============================================================================

/// Controller for managing database-backed notifications.
///
/// All API handlers extract a `sea_orm::DatabaseConnection` from Axum's
/// request extensions to query the `notifications` table directly. The
/// `POST /api/notifications/send` handler also expects a `RateLimiter`
/// extension.
///
/// Wire it up in your application boot:
///
/// ```ignore
/// use axum::{Router, Extension};
///
/// let db = sea_orm::Database::connect("sqlite:db.sqlite").await?;
/// NotificationSender::new()
///     .with_database(db.clone())
///     .ensure_notifications_table().await?;
///
/// let app = Router::new()
///     .layer(Extension(db))
///     .layer(Extension(notification_controller::notification_rate_limiter()));
/// ```
pub struct NotificationController;

impl NotificationController {
    /// Register all notification routes on the given `Registrar`.
    ///
    /// # Required Extensions
    ///
    /// - `sea_orm::DatabaseConnection` — for querying the notifications table
    /// - `RateLimiter` — for rate-limiting the send endpoint
    pub fn register_routes(registrar: &Registrar) {
        registrar.get("/notifications", Self::show_dashboard);
        registrar.get("/api/notifications", Self::list_notifications);
        registrar.get("/api/notifications/unread", Self::list_unread);
        registrar.post("/api/notifications/send", Self::send_notification);
        registrar.post("/api/notifications/{id}/read", Self::mark_as_read);
        registrar.post("/api/notifications/read-all", Self::mark_all_read);
    }

    // -------------------------------------------------------------------------
    // HANDLERS
    // -------------------------------------------------------------------------

    /// GET /notifications — show the notification dashboard (HTML).
    pub async fn show_dashboard(Extension(db): Extension<sea_orm::DatabaseConnection>) -> Response {
        let total = Self::fetch_total(&db).await;
        let unread_count = Self::fetch_unread_count(&db).await;
        let recent = Self::fetch_notifications(&db, 1, 10)
            .await
            .unwrap_or_default();

        let recent_rows: String = recent
            .iter()
            .map(|n| {
                let title = n.data.get("title").and_then(|v| v.as_str()).unwrap_or("(no title)");
                let read_badge = if n.read_at.is_some() {
                    r#"<span class="badge read">Read</span>"#
                } else {
                    r#"<span class="badge unread">Unread</span>"#
                };
                format!(
                    r#"<tr>
                        <td><span class="notif-id">{0}</span></td>
                        <td class="notif-title">{1}</td>
                        <td>{2}</td>
                        <td>{3}</td>
                        <td>
                            <form action="/api/notifications/{0}/read" method="POST" style="display:inline">
                                <button type="submit" class="btn-sm">Mark Read</button>
                            </form>
                        </td>
                    </tr>"#,
                    n.id.chars().take(8).collect::<String>() + "…",
                    html_escape(&title),
                    read_badge,
                    format_timestamp(n.created_at),
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
    <title>Notification Dashboard</title>
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
        .stat.unread .stat-value {{ color: #f59e0b; }}
        .card {{ background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1);
                border-radius: 12px; padding: 1.5rem; backdrop-filter: blur(10px); }}
        table {{ width: 100%; border-collapse: collapse; }}
        th {{ text-align: left; color: #94a3b8; font-size: 0.75rem; text-transform: uppercase;
              letter-spacing: 0.05em; padding: 0.75rem 0.5rem; border-bottom: 1px solid #334155; }}
        td {{ padding: 0.75rem 0.5rem; border-bottom: 1px solid #1e293b; font-size: 0.875rem; }}
        .notif-id {{ color: #64748b; font-family: monospace; font-size: 0.75rem; }}
        .notif-title {{ color: #e2e8f0; }}
        .badge {{ display: inline-block; padding: 0.125rem 0.5rem; border-radius: 999px;
                  font-size: 0.6875rem; font-weight: 600; }}
        .badge.read {{ background: #064e3b; color: #6ee7b7; }}
        .badge.unread {{ background: #451a03; color: #fbbf24; }}
        .btn-sm {{ padding: 0.25rem 0.625rem; background: #6366f1; color: #fff; border: none;
                  border-radius: 4px; font-size: 0.75rem; cursor: pointer;
                  transition: background 0.2s; }}
        .btn-sm:hover {{ background: #4f46e5; }}
        .btn {{ padding: 0.5rem 1rem; background: #6366f1; color: #fff; border: none;
                border-radius: 6px; font-size: 0.8125rem; font-weight: 600; cursor: pointer;
                transition: all 0.2s; }}
        .btn:hover {{ background: #4f46e5; transform: translateY(-1px); }}
        .btn-group {{ display: flex; gap: 0.5rem; margin-bottom: 1rem; align-items: center; }}
        .empty {{ text-align: center; color: #64748b; padding: 2rem; font-size: 0.875rem; }}
        label {{ display: block; color: #cbd5e1; font-size: 0.8125rem; font-weight: 600; margin-bottom: 0.25rem; }}
        input, textarea {{ width: 100%; padding: 0.625rem; border: 1px solid #475569; border-radius: 6px;
               background: #0f172a; color: #e2e8f0; font-size: 0.875rem; outline: none;
               transition: border-color 0.2s; margin-bottom: 0.75rem; }}
        input:focus, textarea:focus {{ border-color: #6366f1; }}
        textarea {{ resize: vertical; min-height: 60px; font-family: inherit; }}
        .form-card {{ margin-bottom: 1.5rem; }}
        .info-box {{ background: #1e3a5f; color: #93c5fd; padding: 0.75rem; border-radius: 6px;
                    font-size: 0.8125rem; margin-bottom: 1rem; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🔔 Notification Dashboard</h1>
        <p class="subtitle">Manage and preview database-backed notifications</p>

        <div class="stats">
            <div class="stat">
                <div class="stat-value">{1}</div>
                <div class="stat-label">Total Notifications</div>
            </div>
            <div class="stat unread">
                <div class="stat-value">{0}</div>
                <div class="stat-label">Unread</div>
            </div>
            <div class="stat">
                <div class="stat-value">{2}</div>
                <div class="stat-label">Page</div>
            </div>
        </div>

        <div class="card form-card">
            <h2 style="font-size: 1.125rem; margin-bottom: 0.5rem; color: #f1f5f9;">📨 Send Test Notification</h2>
            <p style="font-size: 0.8125rem; color: #64748b; margin-bottom: 1rem;">
                Creates a database notification for a notifiable ID.
            </p>
            <form action="/api/notifications/send" method="POST">
                <label for="notifiable-id">Notifiable ID</label>
                <input type="text" id="notifiable-id" name="notifiable_id" value="user-1" required>
                <label for="title">Title</label>
                <input type="text" id="title" name="title" placeholder="Welcome!" required>
                <label for="body">Body</label>
                <textarea id="body" name="body" placeholder="Notification body text..."></textarea>
                <button type="submit" class="btn">Send Notification</button>
            </form>
        </div>

        <div class="card">
            <div class="btn-group">
                <h2 style="font-size: 1.125rem; color: #f1f5f9; flex: 1;">📋 Recent Notifications</h2>
                <form action="/api/notifications/read-all" method="POST" style="display:inline">
                    <button type="submit" class="btn">Mark All Read</button>
                </form>
            </div>

            <table>
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Title</th>
                        <th>Status</th>
                        <th>Created</th>
                        <th>Action</th>
                    </tr>
                </thead>
                <tbody>
                    {3}
                </tbody>
            </table>
            {4}
        </div>
    </div>
</body>
</html>"#,
            unread_count,
            total,
            1,
            if recent_rows.is_empty() {
                r#"<tr><td colspan="5" class="empty">No notifications yet. Send one above!</td></tr>"#.to_string()
            } else {
                recent_rows
            },
            if recent.is_empty() {
                String::new()
            } else {
                r#"<p style="text-align:center;margin-top:1rem;color:#64748b;font-size:0.8125rem;">
                    Showing up to 10 recent notifications.
                   </p>"#
                    .to_string()
            },
        );
        Html(html).into_response()
    }

    /// GET /api/notifications — list all notifications with pagination.
    pub async fn list_notifications(
        Extension(db): Extension<sea_orm::DatabaseConnection>,
        Query(query): Query<ListNotificationsQuery>,
    ) -> Json<PaginatedNotifications> {
        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(20).min(100);

        Json(Self::fetch_paginated(&db, page, per_page).await)
    }

    /// GET /api/notifications/unread — list unread notifications.
    pub async fn list_unread(
        Extension(db): Extension<sea_orm::DatabaseConnection>,
        Query(query): Query<ListNotificationsQuery>,
    ) -> Json<PaginatedNotifications> {
        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(20).min(100);

        Json(Self::fetch_paginated_unread(&db, page, per_page).await)
    }

    /// POST /api/notifications/send — create a notification (demo endpoint).
    ///
    /// Rate-limited to 5 per minute per notifiable.
    pub async fn send_notification(
        Extension(db): Extension<sea_orm::DatabaseConnection>,
        Extension(rate_limiter): Extension<RateLimiter>,
        Form(body): Form<SendNotificationRequest>,
    ) -> Response {
        // Validate input
        if body.title.trim().is_empty() {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Title is required"})),
            )
                .into_response();
        }
        if body.notifiable_id.trim().is_empty() {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Notifiable ID is required"})),
            )
                .into_response();
        }

        // Rate limit check
        if rate_limiter.too_many_attempts(&body.notifiable_id) {
            let retry_after = rate_limiter.retry_after(&body.notifiable_id);
            return RateLimitExceeded {
                retry_after,
                limiter_name: "notification-send".to_string(),
            }
            .into_response();
        }
        rate_limiter.hit(&body.notifiable_id);

        let notifiable = ApiNotifiable {
            id: body.notifiable_id.clone(),
            email: None,
        };

        let data = json!({
            "title": body.title,
            "body": body.body,
        });

        let notification = DemoNotification { data };

        // Create a sender with just the database channel
        let sender = NotificationSender::new().with_database(db);

        let results = sender.send(&notifiable, notification).await;
        let db_result = results.get(&NotificationChannel::Database);

        match db_result {
            Some(Ok(())) => Json(json!({
                "message": "Notification sent successfully",
                "notifiable_id": body.notifiable_id,
                "title": body.title,
            }))
            .into_response(),
            Some(Err(e)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to store notification: {}", e)})),
            )
                .into_response(),
            None => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "No database channel was used"})),
            )
                .into_response(),
        }
    }

    /// POST /api/notifications/{id}/read — mark a single notification as read.
    pub async fn mark_as_read(
        Extension(db): Extension<sea_orm::DatabaseConnection>,
        Path(id): Path<String>,
    ) -> Response {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sql = "UPDATE notifications SET read_at = ?1, updated_at = ?1 WHERE id = ?2";
        let result = db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                [now.into(), id.clone().into()],
            ))
            .await;

        match result {
            Ok(rows_affected) => {
                if rows_affected.rows_affected() > 0 {
                    Json(json!({
                        "message": "Notification marked as read",
                        "id": id,
                    }))
                    .into_response()
                } else {
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "Notification not found"})),
                    )
                        .into_response()
                }
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
                .into_response(),
        }
    }

    /// POST /api/notifications/read-all — mark all notifications as read.
    pub async fn mark_all_read(Extension(db): Extension<sea_orm::DatabaseConnection>) -> Response {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sql = "UPDATE notifications SET read_at = ?1, updated_at = ?1 WHERE read_at IS NULL";
        let result = db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                [now.into()],
            ))
            .await;

        match result {
            Ok(rows_affected) => {
                let count = rows_affected.rows_affected();
                Json(json!({
                    "message": format!("{} notifications marked as read", count),
                    "count": count,
                }))
                .into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
                .into_response(),
        }
    }

    // -------------------------------------------------------------------------
    // DATABASE HELPERS
    // -------------------------------------------------------------------------

    /// Fetch paginated notifications from the database.
    async fn fetch_paginated(
        db: &sea_orm::DatabaseConnection,
        page: u32,
        per_page: u32,
    ) -> PaginatedNotifications {
        let total = Self::fetch_total(db).await;
        let unread_count = Self::fetch_unread_count(db).await;
        let offset = ((page.saturating_sub(1)) * per_page) as i64;

        let sql = format!(
            "SELECT id, notifiable_id, notifiable_type, notification_type, data, \
             read_at, created_at, updated_at FROM notifications \
             ORDER BY created_at DESC LIMIT {} OFFSET {}",
            per_page, offset
        );

        let data = Self::query_notifications(db, &sql).await;

        PaginatedNotifications {
            data,
            total,
            page,
            per_page,
            unread_count,
        }
    }

    /// Fetch paginated unread notifications.
    async fn fetch_paginated_unread(
        db: &sea_orm::DatabaseConnection,
        page: u32,
        per_page: u32,
    ) -> PaginatedNotifications {
        let total = Self::fetch_unread_count(db).await;
        let unread_count = total;
        let offset = ((page.saturating_sub(1)) * per_page) as i64;

        let sql = format!(
            "SELECT id, notifiable_id, notifiable_type, notification_type, data, \
             read_at, created_at, updated_at FROM notifications \
             WHERE read_at IS NULL \
             ORDER BY created_at DESC LIMIT {} OFFSET {}",
            per_page, offset
        );

        let data = Self::query_notifications(db, &sql).await;

        PaginatedNotifications {
            data,
            total,
            page,
            per_page,
            unread_count,
        }
    }

    /// Fetch total notification count.
    async fn fetch_total(db: &sea_orm::DatabaseConnection) -> i64 {
        let sql = "SELECT COUNT(*) as cnt FROM notifications";
        match db
            .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
        {
            Ok(Some(row)) => row.try_get_by_index::<i64>(0).unwrap_or(0),
            _ => 0,
        }
    }

    /// Fetch unread notification count.
    async fn fetch_unread_count(db: &sea_orm::DatabaseConnection) -> i64 {
        let sql = "SELECT COUNT(*) as cnt FROM notifications WHERE read_at IS NULL";
        match db
            .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
        {
            Ok(Some(row)) => row.try_get_by_index::<i64>(0).unwrap_or(0),
            _ => 0,
        }
    }

    /// Fetch recent notifications (limited).
    async fn fetch_notifications(
        db: &sea_orm::DatabaseConnection,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<NotificationResponse>, ()> {
        let offset = ((page.saturating_sub(1)) * per_page) as i64;
        let sql = format!(
            "SELECT id, notifiable_id, notifiable_type, notification_type, data, \
             read_at, created_at, updated_at FROM notifications \
             ORDER BY created_at DESC LIMIT {} OFFSET {}",
            per_page, offset
        );
        Ok(Self::query_notifications(db, &sql).await)
    }

    /// Execute a SELECT query and map rows to `NotificationResponse`.
    async fn query_notifications(
        db: &sea_orm::DatabaseConnection,
        sql: &str,
    ) -> Vec<NotificationResponse> {
        let stmt = Statement::from_string(DatabaseBackend::Sqlite, sql.to_string());

        match db.query_all(stmt).await {
            Ok(rows) => {
                let mut results = Vec::with_capacity(rows.len());
                for row in rows {
                    let data_str: String = row.try_get_by_index(4).unwrap_or_default();
                    let notification = NotificationResponse {
                        id: row.try_get_by_index(0).unwrap_or_default(),
                        notifiable_id: row.try_get_by_index(1).unwrap_or_default(),
                        notifiable_type: row.try_get_by_index(2).unwrap_or_default(),
                        notification_type: row.try_get_by_index(3).unwrap_or_default(),
                        data: serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null),
                        read_at: row.try_get_by_index::<Option<i64>>(5).ok().flatten(),
                        created_at: row.try_get_by_index(6).unwrap_or(0),
                        updated_at: row.try_get_by_index(7).unwrap_or(0),
                    };
                    results.push(notification);
                }
                results
            }
            Err(_) => vec![],
        }
    }
}

// =============================================================================
// DEMO NOTIFICATION
// =============================================================================

/// A simple notification used by the demo send endpoint.
#[derive(Debug, Clone)]
struct DemoNotification {
    data: serde_json::Value,
}

impl Notification for DemoNotification {
    fn via(&self) -> Vec<NotificationChannel> {
        vec![NotificationChannel::Database]
    }

    fn to_database(&self) -> Option<serde_json::Value> {
        Some(self.data.clone())
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
    // Simple relative-time formatting
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

/// Run this example standalone.
fn main() {
    println!("NotificationController example — see the source code for route handlers.");
    println!();
    println!("Routes:");
    println!("  GET  /notifications                     — show notification dashboard (HTML)");
    println!("  GET  /api/notifications                 — list all notifications (JSON)");
    println!("  GET  /api/notifications/unread          — list unread notifications (JSON)");
    println!("  POST /api/notifications/send            — send a notification (demo)");
    println!("  POST /api/notifications/{{id}}/read      — mark a notification as read");
    println!("  POST /api/notifications/read-all        — mark all notifications as read");
    println!();
    println!("Rate limiting: 5 sends/minute per notifiable.");
    println!();
    println!("To register routes:");
    println!("  use notification_controller::NotificationController;");
    println!("  NotificationController::register_routes(&router);");
    println!();
    println!("Required extensions: DatabaseConnection + RateLimiter");
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use tower::ServiceExt;

    /// Counter for generating unique notification IDs in tests.
    static NOTIF_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Build a test router with a shared in-memory SQLite database and
    /// everything wired up.
    async fn test_router() -> (AxumRouter, sea_orm::DatabaseConnection) {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        // Ensure the notifications table exists
        let sender = NotificationSender::new().with_database(db.clone());
        sender.ensure_notifications_table().await.unwrap();

        let rate_limiter = notification_rate_limiter();

        let router = AxumRouter::new()
            .route(
                "/notifications",
                routing::get(NotificationController::show_dashboard),
            )
            .route(
                "/api/notifications",
                routing::get(NotificationController::list_notifications),
            )
            .route(
                "/api/notifications/unread",
                routing::get(NotificationController::list_unread),
            )
            .route(
                "/api/notifications/send",
                routing::post(NotificationController::send_notification),
            )
            .route(
                "/api/notifications/{id}/read",
                routing::post(NotificationController::mark_as_read),
            )
            .route(
                "/api/notifications/read-all",
                routing::post(NotificationController::mark_all_read),
            )
            .layer(Extension(db.clone()))
            .layer(Extension(rate_limiter));

        (router, db)
    }

    /// Helper: insert a notification directly into the database for testing.
    async fn insert_test_notification(
        db: &sea_orm::DatabaseConnection,
        notifiable_id: &str,
        title: &str,
        body: &str,
        read: bool,
    ) -> String {
        let counter = NOTIF_COUNTER.fetch_add(1, Ordering::SeqCst);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let id = format!("notif-{}-{}-{}", notifiable_id, now, counter);
        let data = serde_json::json!({ "title": title, "body": body });
        let data_json = serde_json::to_string(&data).unwrap();

        let sql = "INSERT INTO notifications \
             (id, notifiable_id, notifiable_type, notification_type, data, \
              read_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)";

        let read_at: Option<i64> = if read { Some(now) } else { None };
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql,
            [
                id.clone().into(),
                notifiable_id.to_string().into(),
                "user".into(),
                "DemoNotification".into(),
                data_json.into(),
                read_at.into(),
                now.into(),
            ],
        ))
        .await
        .unwrap();

        id
    }

    // -------------------------------------------------------------------------
    // Dashboard
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_dashboard_returns_html() {
        let (app, _db) = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/notifications")
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
    // List notifications (JSON API)
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_notifications_empty() {
        let (app, _db) = test_router().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/notifications")
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
    async fn test_list_notifications_with_data() {
        let (app, db) = test_router().await;
        insert_test_notification(&db, "user-1", "Hello!", "Test body", false).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/notifications")
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
        assert_eq!(json["total"], 1);
        assert_eq!(json["data"].as_array().unwrap().len(), 1);
        assert_eq!(json["data"][0]["data"]["title"], "Hello!");
        assert_eq!(json["data"][0]["notifiable_id"], "user-1");
    }

    // -------------------------------------------------------------------------
    // Unread notifications
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_unread_filters_read() {
        let (app, db) = test_router().await;
        // Insert one read and one unread notification
        insert_test_notification(&db, "user-1", "Read notif", "", true).await;
        insert_test_notification(&db, "user-1", "Unread notif", "", false).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/notifications/unread")
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
        assert_eq!(json["total"], 1, "Should only count unread notifications");
        assert_eq!(json["data"].as_array().unwrap().len(), 1);
        assert_eq!(json["data"][0]["data"]["title"], "Unread notif");
        assert!(json["data"][0]["read_at"].is_null());
    }

    // -------------------------------------------------------------------------
    // Send notification
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_notification_success() {
        let (app, db) = test_router().await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/notifications/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "notifiable_id=user-1&title=Welcome!&body=Hello+there",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        // Verify the notification was stored
        let sql = "SELECT COUNT(*) as cnt FROM notifications WHERE notifiable_id = ?1";
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["user-1".into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let count: i64 = row.try_get_by_index(0).unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_send_notification_missing_title() {
        let (app, _db) = test_router().await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/notifications/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("notifiable_id=user-1&title=&body=test"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
    }

    #[tokio::test]
    async fn test_send_notification_missing_id() {
        let (app, _db) = test_router().await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/notifications/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("notifiable_id=&title=Test&body=test"))
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
    async fn test_send_notification_rate_limited() {
        let rate_limiter = notification_rate_limiter();
        // Pre-seed 5 hits (the limit)
        for _ in 0..5 {
            rate_limiter.hit("ratelimited-user");
        }

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let sender = NotificationSender::new().with_database(db.clone());
        sender.ensure_notifications_table().await.unwrap();

        let app = AxumRouter::new()
            .route(
                "/api/notifications/send",
                routing::post(NotificationController::send_notification),
            )
            .layer(Extension(db))
            .layer(Extension(rate_limiter));

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/notifications/send")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "notifiable_id=ratelimited-user&title=Test&body=blocked",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 429);
    }

    // -------------------------------------------------------------------------
    // Mark as read
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_mark_as_read_success() {
        let (app, db) = test_router().await;
        let id = insert_test_notification(&db, "user-1", "Read me", "", false).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/api/notifications/{}/read", id))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        // Verify it's marked as read
        let sql = "SELECT read_at FROM notifications WHERE id = ?1";
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                [id.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let read_at: Option<i64> = row.try_get_by_index(0).unwrap();
        assert!(read_at.is_some(), "Notification should have read_at set");
    }

    #[tokio::test]
    async fn test_mark_as_read_not_found() {
        let (app, _db) = test_router().await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/notifications/nonexistent-id/read")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn test_mark_as_read_idempotent() {
        let (app, db) = test_router().await;
        // Insert an already-read notification
        let id = insert_test_notification(&db, "user-1", "Already read", "", true).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/api/notifications/{}/read", id))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            resp.status(),
            200,
            "Marking already-read notification is idempotent"
        );
    }

    // -------------------------------------------------------------------------
    // Mark all as read
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_mark_all_read() {
        let (app, db) = test_router().await;
        // Insert 3 unread notifications
        for i in 0..3 {
            insert_test_notification(&db, "user-1", &format!("Notif {}", i), "", false).await;
        }

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/notifications/read-all")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        // Verify all are read
        let sql = "SELECT COUNT(*) as cnt FROM notifications WHERE read_at IS NULL";
        let row = db
            .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .unwrap()
            .unwrap();
        let unread: i64 = row.try_get_by_index(0).unwrap();
        assert_eq!(unread, 0, "All notifications should be read");

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["count"], 3);
    }

    // -------------------------------------------------------------------------
    // Pagination
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_notifications_pagination() {
        let (app, db) = test_router().await;
        // Insert 5 notifications
        for i in 0..5 {
            insert_test_notification(&db, "user-1", &format!("Notif {}", i), "", false).await;
        }

        // Page 1 with 2 per page
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/notifications?page=1&per_page=2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["total"], 5);
        assert_eq!(json["data"].as_array().unwrap().len(), 2);
        assert_eq!(json["page"], 1);
        assert_eq!(json["per_page"], 2);
    }

    // -------------------------------------------------------------------------
    // Dashboard renders stats
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_dashboard_shows_stats() {
        let (app, db) = test_router().await;
        // Insert some notifications
        insert_test_notification(&db, "user-1", "Notif 1", "", false).await;
        insert_test_notification(&db, "user-1", "Notif 2", "", false).await;
        insert_test_notification(&db, "user-1", "Notif 3", "", true).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/notifications")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 65_536)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&body_bytes);

        // Should show unread count, notification IDs, mark-read forms
        assert!(html.contains("Notification Dashboard"));
        assert!(html.contains(r#"/api/notifications/send"#));
        assert!(html.contains(r#"/api/notifications/read-all"#));
    }

    // -------------------------------------------------------------------------
    // HTML escaping
    // -------------------------------------------------------------------------

    #[test]
    fn test_html_escape() {
        let input = "<script>alert('xss')</script>";
        let escaped = html_escape(input);
        assert_eq!(escaped, "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;");
    }

    #[test]
    fn test_format_timestamp() {
        // Just verify it doesn't panic and returns something reasonable
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let formatted = format_timestamp(now);
        assert_eq!(formatted, "just now");

        let old = format_timestamp(now - 7200);
        assert!(old.contains("h ago") || old.contains("m ago"));
    }

    // -------------------------------------------------------------------------
    // NotificationResponse serialization
    // -------------------------------------------------------------------------

    #[test]
    fn test_notification_response_serialization() {
        let resp = NotificationResponse {
            id: "abc-123".to_string(),
            notifiable_id: "user-1".to_string(),
            notifiable_type: "user".to_string(),
            notification_type: "TestNotification".to_string(),
            data: serde_json::json!({"title": "Test"}),
            read_at: None,
            created_at: 1000,
            updated_at: 1000,
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["id"], "abc-123");
        assert_eq!(json["data"]["title"], "Test");
        assert!(json["read_at"].is_null());
    }

    // -------------------------------------------------------------------------
    // Pagination with per_page cap
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_notifications_per_page_cap() {
        let (app, db) = test_router().await;
        // Insert 150 notifications
        for i in 0..150 {
            insert_test_notification(&db, "user-1", &format!("Bulk {}", i), "", false).await;
        }

        // Request per_page=999 — should be capped at 100
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/notifications?page=1&per_page=999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body_bytes = larastvel_core::axum::body::to_bytes(resp.into_body(), 65_536)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["total"], 150);
        assert_eq!(json["per_page"], 100, "Should be capped at max 100");
    }
}
