//! # AuthServiceProvider Example
//!
//! This example demonstrates how to wire up Larastvel's security services at
//! application boot: **Authorization (Gate)**, **Password Reset**, and
//! **Email Verification**.
//!
//! Copy the relevant sections into your own application's `main.rs` or
//! into a dedicated service provider.
//!
//! ## Overview
//!
//! ```text
//! Application::new()
//!   ├── AuthServiceProvider::register()
//!   │   ├── Gate          ← define abilities, register policies
//!   │   ├── PasswordResetBroker ← token table, mailer, throttle
//!   │   └── EmailVerificationBroker ← JWT token, callbacks, mailer
//!   │
//!   └── Axum Router
//!       ├── auth_middleware            ← extracts JWT → extensions
//!       ├── require_verified_email     ← checks VerifiedUser
//!       ├── require_ability("admin")   ← checks Gate
//!       └── handlers (VerifiedUser, PasswordResetBroker in extensions)
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! // In your application's main.rs or kernel:
//! auth_service_provider::register(&app, &db, &mailer, &gate).await;
//! ```
//!
//! Or pick individual services as needed — see the module-level functions below.

#![allow(unused_imports, dead_code)]

use std::sync::Arc;

use larastvel_core::auth::{
    AuthenticatedUser, EmailVerificationBroker, Gate, GateCheck, MarkVerifiedCallback,
    PasswordResetBroker, PasswordResetConfig, VerificationChecker,
};
use larastvel_core::mail::Mailer;
use larastvel_core::sea_orm::ConnectionTrait;
use larastvel_core::sea_orm::{self, DatabaseConnection};
use larastvel_core::Application;

// =============================================================================
// 1. GATE / AUTHORIZATION
// =============================================================================

/// Register abilities and policies on the Gate.
///
/// Policies can be defined inline as closures or as separate structs
/// implementing the `Policy` trait (generated via `make:policy`).
pub fn register_gate(gate: &Gate) {
    // --- Define abilities inline (closures) ---
    gate.define(
        "view-dashboard",
        |user: &AuthenticatedUser, _args: &[String]| {
            // Simple check: admins can view the dashboard
            if user.user_id.starts_with("admin") {
                GateCheck::Allowed
            } else {
                GateCheck::Denied("Only admins can view the dashboard.".to_string())
            }
        },
    );

    gate.define(
        "manage-users",
        |user: &AuthenticatedUser, _args: &[String]| {
            if user.user_id == "admin-1" {
                GateCheck::Allowed
            } else {
                GateCheck::Denied("You do not have permission to manage users.".to_string())
            }
        },
    );

    // --- Before hook: super-admin bypasses all checks ---
    gate.before(
        |user: &AuthenticatedUser, _ability: &str, _args: &[String]| {
            if user.user_id == "super-admin" {
                Some(GateCheck::Allowed)
            } else {
                None // fall through to normal checks
            }
        },
    );

    // --- After hook: log all denials ---
    gate.after(
        |_user: &AuthenticatedUser, ability: &str, _args: &[String], result: &GateCheck| {
            if result.is_denied() {
                tracing::warn!(
                    "Gate: denied access to '{}' for user '{}': {:?}",
                    ability,
                    _user.user_id,
                    result.message(),
                );
            }
            None // don't override
        },
    );

    // --- Register a policy class (from make:policy) ---
    // Uncomment after generating with: `larastvel make:policy PostPolicy`
    // gate.register_policy("post", std::sync::Arc::new(crate::policies::post_policy::PostPolicy));
}

// =============================================================================
// 2. PASSWORD RESET
// =============================================================================

/// Configuration for the password reset broker.
pub struct PasswordResetSetup {
    pub broker: PasswordResetBroker,
    pub config: PasswordResetConfig,
}

/// Create and configure the PasswordResetBroker.
///
/// ```ignore
/// let setup = create_password_reset_broker(
///     &db,
///     &mailer,
///     "noreply@example.com",
///     "http://localhost:8080",
///     "MyApp",
/// ).await;
/// ```
pub async fn create_password_reset_broker(
    db: &DatabaseConnection,
    mailer: Arc<dyn Mailer>,
    from_address: &str,
    app_url: &str,
    app_name: &str,
) -> PasswordResetSetup {
    // --- Load config from config.toml or use defaults ---
    let config = PasswordResetConfig {
        table: "password_reset_tokens".to_string(), // matches config.toml [password_reset].table
        expire_seconds: 3600,                       // 60 minutes
        throttle_seconds: 60,                       // 60 seconds between requests
    };

    let broker = PasswordResetBroker::new(
        db.clone(),
        config.clone(),
        mailer,
        from_address,
        app_url,
        app_name,
    );

    // Ensure the password_reset_tokens table exists
    broker
        .ensure_table_exists()
        .await
        .expect("Failed to create password_reset_tokens table");

    PasswordResetSetup { broker, config }
}

// =============================================================================
// 3. EMAIL VERIFICATION
// =============================================================================

/// Create the EmailVerificationBroker with database-backed callbacks.
///
/// The `check_verified` and `mark_verified` callbacks query/update the `users`
/// table's `email_verified_at` column using raw SQL or SeaORM.
///
/// ```ignore
/// let verification_broker = create_email_verification_broker(
///     &db,
///     &secret,
///     &mailer,
///     "noreply@example.com",
///     "http://localhost:8080",
///     "MyApp",
/// ).await;
/// ```
pub fn create_email_verification_broker(
    db: &DatabaseConnection,
    secret: &[u8],
    mailer: Arc<dyn Mailer>,
    from_address: &str,
    app_url: &str,
    app_name: &str,
) -> EmailVerificationBroker {
    // --- Callback: check if a user's email is verified ---
    let db_for_check = db.clone();
    let check_verified: VerificationChecker = Arc::new(move |user_id: &str| {
        // Use tokio::task::block_in_place to run async SeaORM query in sync context
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // Parse user_id to i32 (depends on your schema)
                let uid: i32 = match user_id.parse() {
                    Ok(id) => id,
                    Err(_) => return false,
                };

                // Query the users table — adjust the import path to match
                // your project's entity model, or use raw SQL:
                //
                //   let sql = "SELECT email_verified_at FROM users WHERE id = ?1";
                //   let row = db.query_one(sea_orm::Statement::from_sql_and_values(
                //       sea_orm::DatabaseBackend::Sqlite, sql, [uid.into()],
                //   )).await.ok().flatten();
                //   row.and_then(|r| r.try_get_by_index::<String>(0).ok())
                //       .map(|v| !v.is_empty())
                //       .unwrap_or(false)
                //

                // --- Raw SQL approach (works with any schema) ---
                let sql = "SELECT email_verified_at FROM users WHERE id = ?1";
                let row = db_for_check
                    .query_one(larastvel_core::sea_orm::Statement::from_sql_and_values(
                        larastvel_core::sea_orm::DatabaseBackend::Sqlite,
                        sql,
                        [uid.into()],
                    ))
                    .await
                    .ok()
                    .flatten();

                match row {
                    Some(r) => {
                        // If email_verified_at is NOT NULL, user is verified
                        let verified: Option<String> = r.try_get_by_index(0).ok();
                        verified.is_some()
                    }
                    None => false,
                }
            })
        });
        result
    });

    // --- Callback: mark a user's email as verified ---
    let db_for_mark = db.clone();
    let mark_verified: MarkVerifiedCallback = Arc::new(move |user_id: &str| {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let uid: i32 = match user_id.parse() {
                    Ok(id) => id,
                    Err(_) => {
                        return Err(larastvel_core::auth::EmailVerificationError::Token(
                            "Invalid user ID".to_string(),
                        ))
                    }
                };

                // Update the email_verified_at column
                let sql = "UPDATE users SET email_verified_at = datetime('now') WHERE id = ?1";
                db_for_mark
                    .execute(larastvel_core::sea_orm::Statement::from_sql_and_values(
                        larastvel_core::sea_orm::DatabaseBackend::Sqlite,
                        sql,
                        [uid.into()],
                    ))
                    .await
                    .map_err(|e| {
                        larastvel_core::auth::EmailVerificationError::Token(e.to_string())
                    })?;

                Ok(())
            })
        })
    });

    EmailVerificationBroker::new(
        secret,
        mailer,
        from_address,
        app_url,
        app_name,
        check_verified,
        mark_verified,
        3600, // Token expiry: 60 minutes
    )
}

// =============================================================================
// 4. SERVICE PROVIDER — WIRES EVERYTHING TOGETHER
// =============================================================================

/// Register all authentication services with the application.
///
/// Call this during application boot. The services are inserted into the
/// Axum request extensions so that middleware and handlers can access them.
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
///
/// // In your kernel or main.rs:
/// let mut app = Application::new(None);
///
/// // --- Set up database ---
/// let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
/// let db_manager = larastvel_core::database::DatabaseManager::new(&app.config());
/// app = app.with_database(db_manager);
///
/// // --- Set up mailer ---
/// let mail_manager = setup_mailer();
///
/// // --- Set up Gate ---
/// let gate = Gate::new();
/// register_gate(&gate);
///
/// // --- Set up Password Reset ---
/// let pwd_setup = create_password_reset_broker(
///     &db, mail_manager.default_mailer().unwrap(),
///     "noreply@example.com", "http://localhost:8080", "Larastvel",
/// ).await;
///
/// // --- Set up Email Verification ---
/// let secret = app.config().app.key
///     .as_deref()
///     .unwrap_or("change-me")
///     .as_bytes()
///     .to_vec();
/// let verification_broker = create_email_verification_broker(
///     &db, &secret, mail_manager.default_mailer().unwrap(),
///     "noreply@example.com", "http://localhost:8080", "Larastvel",
/// );
///
/// // --- Insert services into application container ---
/// app.bind(gate);
/// app.bind(pwd_setup.broker);
/// app.bind(verification_broker);
/// app.bind(mail_manager);
///
/// // --- Build the Axum router with middleware ---
/// use axum::{Router, middleware};
/// use larastvel_core::auth::{auth_middleware, require_verified_email, require_ability};
///
/// let router = Router::new()
///     // Public routes
///     .route("/health", axum::routing::get(health_check))
///
///     // Authenticated routes (JWT required)
///     .route("/profile", axum::routing::get(get_profile))
///     .route_layer(middleware::from_fn(auth_middleware))
///
///     // Verified-email-only routes
///     .route("/dashboard", axum::routing::get(get_dashboard))
///     .route_layer(middleware::from_fn(require_verified_email))
///     .route_layer(middleware::from_fn(auth_middleware))
///
///     // Admin-only routes
///     .route("/admin", axum::routing::get(admin_panel))
///     .route_layer(middleware::from_fn(require_ability("admin")))
///     .route_layer(middleware::from_fn(auth_middleware));
///
/// // --- Insert services into every request's extensions ---
/// // (This is done via a Tower layer or in the auth_middleware itself)
/// //
/// // For example, in auth_middleware, after extracting the user:
/// //   req.extensions_mut().insert(gate.clone());
/// //   req.extensions_mut().insert(verification_broker.clone());
/// ```
pub async fn register_all(
    app: &Application,
    db: &DatabaseConnection,
    gate: &Gate,
    mailer: Arc<dyn Mailer>,
) {
    let config = app.config();
    let from_address = format!("noreply@{}", config.app.name.to_lowercase());
    let app_url = config.app.url.clone();
    let app_name = config.app.name.clone();

    // --- Gate ---
    register_gate(gate);
    app.bind(gate.clone());

    // --- Password Reset ---
    let pwd_setup =
        create_password_reset_broker(db, mailer.clone(), &from_address, &app_url, &app_name).await;
    app.bind(pwd_setup.broker);

    // --- Email Verification ---
    let secret = config
        .app
        .key
        .as_deref()
        .unwrap_or("change-me-in-production")
        .as_bytes()
        .to_vec();
    let verification_broker =
        create_email_verification_broker(db, &secret, mailer, &from_address, &app_url, &app_name);
    app.bind(verification_broker);

    tracing::info!("AuthServiceProvider: all services registered");
}

// =============================================================================
// 5. EXAMPLE HANDLERS
// =============================================================================

/// Example handler using `VerifiedUser` extractor.
///
/// ```ignore
/// use axum::response::Json;
/// use larastvel_core::auth::VerifiedUser;
/// use serde_json::json;
///
/// async fn get_dashboard(user: VerifiedUser) -> Json<serde_json::Value> {
///     Json(json!({
///         "user_id": user.user_id,
///         "message": "Welcome to your dashboard!"
///     }))
/// }
/// ```

/// Example handler using `AuthenticatedUser` and the Gate from extensions.
///
/// ```ignore
/// use axum::{Extension, Json};
/// use larastvel_core::auth::{AuthenticatedUser, Gate};
///
/// async fn delete_post(
///     user: AuthenticatedUser,
///     Extension(gate): Extension<Gate>,
/// ) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
///     match authorize(&gate, &user, "delete-post", &["42".to_string()]) {
///         Ok(()) => Ok(Json(serde_json::json!({ "status": "deleted" }))),
///         Err(check) => Err((
///             axum::http::StatusCode::FORBIDDEN,
///             Json(serde_json::json!({ "error": check.message() })),
///         )),
///     }
/// }
/// ```

/// Entry point (required for `cargo run --example`).
/// See the module-level documentation for how to wire up the services.
fn main() {
    println!("AuthServiceProvider example — see the source code for wiring patterns.");
    println!();
    println!("Key services demonstrated:");
    println!("  1. Gate - abilities, policies, before/after hooks");
    println!("  2. PasswordResetBroker - token table, mailer, throttle");
    println!("  3. EmailVerificationBroker - JWT tokens, callbacks");
    println!();
    println!("To run the unit tests: cargo test --example auth_service_provider");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that the Gate wiring compiles and basic checks work.
    #[test]
    fn test_gate_wiring() {
        let gate = Gate::new();
        register_gate(&gate);

        let admin = AuthenticatedUser {
            user_id: "admin-1".to_string(),
            claims: larastvel_core::auth::Claims {
                sub: "admin-1".to_string(),
                exp: 9999999999,
                iat: 0,
            },
        };

        let regular = AuthenticatedUser {
            user_id: "user-1".to_string(),
            claims: larastvel_core::auth::Claims {
                sub: "user-1".to_string(),
                exp: 9999999999,
                iat: 0,
            },
        };

        assert!(gate.allows(&admin, "view-dashboard", &[]));
        assert!(!gate.allows(&regular, "view-dashboard", &[]));
        assert!(!gate.allows(&admin, "nonexistent", &[]));
    }

    /// Test that the before hook (super-admin bypass) works.
    #[test]
    fn test_gate_before_hook() {
        let gate = Gate::new();
        register_gate(&gate);

        let super_admin = AuthenticatedUser {
            user_id: "super-admin".to_string(),
            claims: larastvel_core::auth::Claims {
                sub: "super-admin".to_string(),
                exp: 9999999999,
                iat: 0,
            },
        };

        // Super-admin should be allowed even for undefined abilities
        assert!(gate.allows(&super_admin, "anything", &[]));
    }

    /// Test that PasswordResetConfig defaults are sensible.
    #[test]
    fn test_password_reset_config() {
        let config = PasswordResetConfig::default();
        assert_eq!(config.expire_seconds, 3600);
        assert_eq!(config.throttle_seconds, 60);
        assert_eq!(config.table, "password_reset_tokens");
    }
}
