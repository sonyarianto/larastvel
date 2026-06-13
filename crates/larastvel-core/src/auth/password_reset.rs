use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use rand::Rng;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::json;
use thiserror::Error;

use crate::mail::{Mailable, Mailer};

/// A password reset token entry.
#[derive(Debug, Clone)]
pub struct PasswordResetToken {
    pub email: String,
    pub token: String,
    pub created_at: i64,
}

/// Errors that can occur during password reset operations.
#[derive(Debug, Error)]
pub enum PasswordResetError {
    #[error("User not found: {0}")]
    UserNotFound(String),
    #[error("Invalid or expired reset token")]
    InvalidToken,
    #[error("Token has expired")]
    TokenExpired,
    #[error("Too many reset attempts. Please wait {0} seconds.")]
    Throttle(u64),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mail error: {0}")]
    Mail(String),
    #[error("Invalid email address")]
    InvalidEmail,
}

impl IntoResponse for PasswordResetError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            PasswordResetError::UserNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            PasswordResetError::InvalidToken => (
                StatusCode::BAD_REQUEST,
                "The password reset token is invalid.".to_string(),
            ),
            PasswordResetError::TokenExpired => (
                StatusCode::BAD_REQUEST,
                "The password reset token has expired.".to_string(),
            ),
            PasswordResetError::Throttle(seconds) => (
                StatusCode::TOO_MANY_REQUESTS,
                format!("Too many reset attempts. Please wait {} seconds.", seconds),
            ),
            PasswordResetError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal error occurred.".to_string(),
            ),
            PasswordResetError::Mail(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send password reset email.".to_string(),
            ),
            PasswordResetError::InvalidEmail => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "The email address is invalid.".to_string(),
            ),
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}

/// Configuration for password reset behavior.
///
/// Mirrors Laravel's `config/auth.php` `passwords.users` section.
#[derive(Debug, Clone)]
pub struct PasswordResetConfig {
    /// The database table used to store reset tokens.
    pub table: String,
    /// How many seconds a reset token remains valid (default: 3600 = 60 min).
    pub expire_seconds: u64,
    /// Minimum seconds between reset requests for the same email (default: 60).
    pub throttle_seconds: u64,
}

impl Default for PasswordResetConfig {
    fn default() -> Self {
        Self {
            table: "password_reset_tokens".to_string(),
            expire_seconds: 3600,
            throttle_seconds: 60,
        }
    }
}

/// Manages password reset token lifecycle and notifications.
///
/// # Example
///
/// ```ignore
/// use larastvel_core::auth::{PasswordResetBroker, PasswordResetConfig};
/// use larastvel_core::mail::LogMailer;
/// use std::sync::Arc;
///
/// let broker = PasswordResetBroker::new(
///     db_connection,
///     PasswordResetConfig::default(),
///     Arc::new(LogMailer::new("log")),
///     "noreply@example.com",
///     "http://localhost:8080",
///     "MyApp",
/// );
///
/// // Send a reset link
/// broker.send_reset_link("user@example.com").await?;
///
/// // Reset the password
/// broker.reset("user@example.com", "the-token", "new-hash", |email, password| {
///     // Update user's password in the database
///     Ok(())
/// }).await?;
/// ```
#[derive(Clone)]
pub struct PasswordResetBroker {
    db: sea_orm::DatabaseConnection,
    config: PasswordResetConfig,
    mailer: Arc<dyn Mailer>,
    from_address: String,
    app_url: String,
    app_name: String,
}

impl std::fmt::Debug for PasswordResetBroker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordResetBroker")
            .field("config", &self.config)
            .field("from_address", &self.from_address)
            .field("app_url", &self.app_url)
            .field("app_name", &self.app_name)
            .field("mailer", &"<dyn Mailer>")
            .finish()
    }
}

impl PasswordResetBroker {
    /// Create a new password reset broker.
    ///
    /// - `db`: A SeaORM database connection for token storage.
    /// - `config`: Configuration for expiry, throttle, and table name.
    /// - `mailer`: The mailer used to send reset link emails.
    /// - `from_address`: The "From" address for reset emails.
    /// - `app_url`: Base URL of the application (used to build reset links).
    /// - `app_name`: Application name (used in email subject and body).
    pub fn new(
        db: sea_orm::DatabaseConnection,
        config: PasswordResetConfig,
        mailer: Arc<dyn Mailer>,
        from_address: &str,
        app_url: &str,
        app_name: &str,
    ) -> Self {
        Self {
            db,
            config,
            mailer,
            from_address: from_address.to_string(),
            app_url: app_url.to_string(),
            app_name: app_name.to_string(),
        }
    }

    /// Ensure the password reset tokens table exists in the database.
    pub async fn ensure_table_exists(&self) -> Result<(), PasswordResetError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                email TEXT PRIMARY KEY,
                token TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
            self.config.table
        );
        self.db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| PasswordResetError::Database(format!("Failed to create table: {}", e)))?;
        Ok(())
    }

    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    fn generate_token() -> String {
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        hex::encode(&bytes)
    }

    fn is_token_expired(&self, created_at: i64) -> bool {
        let now = Self::now();
        now - created_at > self.config.expire_seconds as i64
    }

    /// Send a password reset email to the given address.
    ///
    /// Returns `Ok(())` even if the email doesn't exist in the user table
    /// (to prevent email enumeration attacks). The token is always stored.
    ///
    /// Returns `Err(PasswordResetError::Throttle)` if called too soon after
    /// the last request for the same email.
    pub async fn send_reset_link(&self, email: &str) -> Result<(), PasswordResetError> {
        // --- Throttle check ---
        let existing = self.find_token_row(email).await?;

        if let Some(token_row) = existing {
            let created_at: i64 = token_row.try_get_by_index::<i64>(2).unwrap_or(0);
            let elapsed = Self::now() - created_at;
            if elapsed < self.config.throttle_seconds as i64 {
                let wait = self.config.throttle_seconds.saturating_sub(elapsed as u64);
                return Err(PasswordResetError::Throttle(wait));
            }
        }

        // --- Generate & store token ---
        let token = Self::generate_token();
        let now = Self::now();

        let upsert_sql = format!(
            "INSERT OR REPLACE INTO {} (email, token, created_at) VALUES (?1, ?2, ?3)",
            self.config.table
        );
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &upsert_sql,
                [email.into(), token.clone().into(), now.into()],
            ))
            .await
            .map_err(|e| PasswordResetError::Database(e.to_string()))?;

        // --- Build reset URL ---
        let encoded_email = urlencoding(email);
        let reset_url = format!(
            "{}/password/reset/{}?email={}",
            self.app_url.trim_end_matches('/'),
            token,
            encoded_email,
        );

        // --- Send email ---
        let subject = format!("{} - Reset Password Notification", self.app_name);
        let minutes = self.config.expire_seconds / 60;
        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head><meta charset="UTF-8"></head>
<body style="font-family: Arial, Helvetica, sans-serif; line-height: 1.6; color: #1a1a2e; margin: 0; padding: 0;">
    <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
        <div style="background-color: #4f46e5; padding: 30px; border-radius: 8px 8px 0 0; text-align: center;">
            <h1 style="color: #ffffff; margin: 0; font-size: 24px;">Reset Password</h1>
        </div>
        <div style="background-color: #ffffff; padding: 30px; border: 1px solid #e5e7eb; border-top: none; border-radius: 0 0 8px 8px;">
            <p style="font-size: 16px;">Hello!</p>
            <p style="font-size: 16px;">You are receiving this email because we received a password reset request for your account.</p>
            <p style="text-align: center; margin: 30px 0;">
                <a href="{}"
                   style="display: inline-block; padding: 14px 32px; background-color: #4f46e5;
                          color: #ffffff; text-decoration: none; border-radius: 6px;
                          font-weight: bold; font-size: 16px;">
                    Reset Password
                </a>
            </p>
            <p style="font-size: 14px; color: #6b7280;">This password reset link will expire in {} minutes.</p>
            <p style="font-size: 14px; color: #6b7280;">If you did not request a password reset, no further action is required.</p>
            <hr style="border: none; border-top: 1px solid #e5e7eb; margin: 24px 0;">
            <p style="font-size: 14px; color: #6b7280;">Regards,<br>{} Team</p>
        </div>
    </div>
</body>
</html>"#,
            reset_url, minutes, self.app_name,
        );

        let text_body = format!(
            "Hello!\n\n\
             You are receiving this email because we received a password reset request for your account.\n\n\
             Click the link below to reset your password:\n\
             {}\n\n\
             This password reset link will expire in {} minutes.\n\n\
             If you did not request a password reset, no further action is required.\n\n\
             Regards,\n\
             {} Team",
            reset_url, minutes, self.app_name,
        );

        let mailable =
            Mailable::html(vec![email.to_string()], &subject, &html_body).from(&self.from_address);

        // Also send a plain-text alternative through the log
        tracing::debug!(
            target: "larastvel::auth::password_reset",
            "Password reset email sent to {}: text body preview: {}",
            email,
            &text_body[..text_body.len().min(100)],
        );

        self.mailer
            .send(mailable)
            .await
            .map_err(|e| PasswordResetError::Mail(e.to_string()))?;

        Ok(())
    }

    /// Look up a token row by email.
    async fn find_token_row(
        &self,
        email: &str,
    ) -> Result<Option<sea_orm::QueryResult>, PasswordResetError> {
        let sql = format!(
            "SELECT email, token, created_at FROM {} WHERE email = ?1",
            self.config.table
        );
        self.db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [email.into()],
            ))
            .await
            .map_err(|e| PasswordResetError::Database(e.to_string()))
    }

    /// Find a valid, non-expired reset token for the given email and token value.
    async fn find_valid_token(
        &self,
        email: &str,
        token: &str,
    ) -> Result<PasswordResetToken, PasswordResetError> {
        let sql = format!(
            "SELECT email, token, created_at FROM {} WHERE email = ?1 AND token = ?2",
            self.config.table
        );

        let result = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [email.into(), token.into()],
            ))
            .await
            .map_err(|e| PasswordResetError::Database(e.to_string()))?;

        match result {
            Some(row) => {
                let email_val: String = row
                    .try_get_by_index(0)
                    .map_err(|e| PasswordResetError::Database(e.to_string()))?;
                let token_val: String = row
                    .try_get_by_index(1)
                    .map_err(|e| PasswordResetError::Database(e.to_string()))?;
                let created_at: i64 = row
                    .try_get_by_index(2)
                    .map_err(|e| PasswordResetError::Database(e.to_string()))?;

                let reset_token = PasswordResetToken {
                    email: email_val,
                    token: token_val,
                    created_at,
                };

                if self.is_token_expired(reset_token.created_at) {
                    // Clean up expired token
                    let _ = self.delete_token(&reset_token.email).await;
                    return Err(PasswordResetError::TokenExpired);
                }

                Ok(reset_token)
            }
            None => Err(PasswordResetError::InvalidToken),
        }
    }

    /// Delete a password reset token for the given email.
    async fn delete_token(&self, email: &str) -> Result<(), PasswordResetError> {
        let sql = format!("DELETE FROM {} WHERE email = ?1", self.config.table);
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [email.into()],
            ))
            .await
            .map_err(|e| PasswordResetError::Database(e.to_string()))?;
        Ok(())
    }

    /// Reset a user's password using a valid reset token.
    ///
    /// The `update_password` closure receives the validated email and the new
    /// password hash. It should update the user record in the database.
    ///
    /// After a successful reset, the token is deleted from the database.
    pub async fn reset<F>(
        &self,
        email: &str,
        token: &str,
        password: &str,
        update_password: F,
    ) -> Result<(), PasswordResetError>
    where
        F: FnOnce(&str, &str) -> Result<(), PasswordResetError> + Send,
    {
        // Validate the token
        let reset_token = self.find_valid_token(email, token).await?;

        // Update the password
        update_password(&reset_token.email, password)?;

        // Delete the used token
        self.delete_token(&reset_token.email).await?;

        Ok(())
    }

    /// Return a reference to the config.
    pub fn config(&self) -> &PasswordResetConfig {
        &self.config
    }
}

/// URL-encode a string (simple version, sufficient for email addresses).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::LogMailer;
    use std::sync::Arc;

    async fn setup_broker() -> PasswordResetBroker {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let mailer = Arc::new(LogMailer::new("log"));

        let broker = PasswordResetBroker::new(
            db,
            PasswordResetConfig::default(),
            mailer,
            "noreply@example.com",
            "http://localhost:8080",
            "Larastvel",
        );
        broker.ensure_table_exists().await.unwrap();
        broker
    }

    #[tokio::test]
    async fn test_send_reset_link_creates_token() {
        let broker = setup_broker().await;
        broker.send_reset_link("user@example.com").await.unwrap();

        // Verify token exists in DB
        let sql = "SELECT email, token, created_at FROM password_reset_tokens WHERE email = ?1";
        let result = broker
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["user@example.com".into()],
            ))
            .await
            .unwrap();
        assert!(result.is_some());
        let row = result.unwrap();
        let token: String = row.try_get_by_index(1).unwrap();
        assert_eq!(token.len(), 64); // 32 bytes hex-encoded
    }

    #[tokio::test]
    async fn test_throttle_prevents_rapid_requests() {
        let broker = setup_broker().await;
        broker
            .send_reset_link("throttle@example.com")
            .await
            .unwrap();

        // Second attempt should be throttled (throttle is 60 sec, we just did it)
        let result = broker.send_reset_link("throttle@example.com").await;
        assert!(result.is_err());
        match result {
            Err(PasswordResetError::Throttle(_)) => {} // expected
            _ => panic!("Expected Throttle error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_find_valid_token() {
        let broker = setup_broker().await;
        broker.send_reset_link("test@example.com").await.unwrap();

        // Get the token from DB
        let sql = "SELECT token FROM password_reset_tokens WHERE email = ?1";
        let row = broker
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["test@example.com".into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let token: String = row.try_get_by_index(0).unwrap();

        // Find it via the broker
        let found = broker.find_valid_token("test@example.com", &token).await;
        assert!(found.is_ok());
        assert_eq!(found.unwrap().email, "test@example.com");
    }

    #[tokio::test]
    async fn test_find_invalid_token_returns_error() {
        let broker = setup_broker().await;
        let result = broker
            .find_valid_token("unknown@example.com", "invalid-token")
            .await;
        assert!(result.is_err());
        match result {
            Err(PasswordResetError::InvalidToken) => {} // expected
            _ => panic!("Expected InvalidToken error"),
        }
    }

    #[tokio::test]
    async fn test_reset_with_valid_token() {
        let broker = setup_broker().await;
        broker.send_reset_link("reset@example.com").await.unwrap();

        // Get the token
        let sql = "SELECT token FROM password_reset_tokens WHERE email = ?1";
        let row = broker
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["reset@example.com".into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let token: String = row.try_get_by_index(0).unwrap();

        let password_updated = std::sync::atomic::AtomicBool::new(false);
        let updated = &password_updated;

        let result = broker
            .reset(
                "reset@example.com",
                &token,
                "new-password-hash",
                |email, password| {
                    assert_eq!(email, "reset@example.com");
                    assert_eq!(password, "new-password-hash");
                    updated.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
            )
            .await;

        assert!(result.is_ok());
        assert!(updated.load(std::sync::atomic::Ordering::SeqCst));

        // Token should be deleted
        let result = broker.find_valid_token("reset@example.com", &token).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reset_with_invalid_token_fails() {
        let broker = setup_broker().await;
        let result = broker
            .reset("nobody@example.com", "fake-token", "hash", |_, _| Ok(()))
            .await;
        assert!(result.is_err());
        match result {
            Err(PasswordResetError::InvalidToken) => {} // expected
            _ => panic!("Expected InvalidToken error"),
        }
    }

    #[tokio::test]
    async fn test_generate_token_produces_hex() {
        let token = PasswordResetBroker::generate_token();
        assert_eq!(token.len(), 64); // 32 bytes = 64 hex chars
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_generate_token_unique() {
        let t1 = PasswordResetBroker::generate_token();
        let t2 = PasswordResetBroker::generate_token();
        assert_ne!(t1, t2);
    }

    #[tokio::test]
    async fn test_default_config() {
        let config = PasswordResetConfig::default();
        assert_eq!(config.expire_seconds, 3600);
        assert_eq!(config.throttle_seconds, 60);
        assert_eq!(config.table, "password_reset_tokens");
    }

    #[tokio::test]
    async fn test_custom_config() {
        let config = PasswordResetConfig {
            expire_seconds: 1800,
            throttle_seconds: 30,
            table: "custom_reset_tokens".to_string(),
        };
        assert_eq!(config.expire_seconds, 1800);
        assert_eq!(config.throttle_seconds, 30);
        assert_eq!(config.table, "custom_reset_tokens");
    }

    #[tokio::test]
    async fn test_delete_token() {
        let broker = setup_broker().await;
        broker
            .send_reset_link("delete_me@example.com")
            .await
            .unwrap();

        // Verify it exists
        let sql = "SELECT email FROM password_reset_tokens WHERE email = ?1";
        let row = broker
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["delete_me@example.com".into()],
            ))
            .await
            .unwrap();
        assert!(row.is_some(), "Token should exist before deletion");

        // Delete it
        broker.delete_token("delete_me@example.com").await.unwrap();

        // Verify it's gone
        let row = broker
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                ["delete_me@example.com".into()],
            ))
            .await
            .unwrap();
        assert!(row.is_none(), "Token should be deleted");
    }

    #[tokio::test]
    async fn test_urlencoding() {
        assert_eq!(urlencoding("user@example.com"), "user%40example.com");
        assert_eq!(urlencoding("simple"), "simple");
        assert_eq!(urlencoding("a b"), "a%20b");
    }

    #[tokio::test]
    async fn test_config_accessor() {
        let broker = setup_broker().await;
        assert_eq!(broker.config().expire_seconds, 3600);
    }
}
