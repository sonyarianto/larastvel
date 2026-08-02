# Password Reset

Larastvel provides a token-based password reset system.

## Configuration

Configure in `config/password_reset.toml`:

```toml
table = "password_reset_tokens"
expire_seconds = 3600
throttle_seconds = 60
```

## Usage

```rust
use larastvel_core::auth::{PasswordResetBroker, PasswordResetConfig};
use larastvel_core::mail::LogMailer;
use std::sync::Arc;

let config = PasswordResetConfig::default();
let broker = PasswordResetBroker::new(
    db,                                      // sea_orm::DatabaseConnection
    config,
    Arc::new(LogMailer::new("log")),         // Arc<dyn Mailer>
    "noreply@example.com",                   // from_address
    "http://localhost:8080",                 // app_url
    "MyApp",                                 // app_name
);

// Send a reset link (creates + emails the token)
broker.send_reset_link("user@example.com").await?;

// Validate the token and reset the password
broker.reset("user@example.com", "token", "new-password-hash", |email, password| {
    // Update the user's password in the database
    Ok(())
}).await?;
```

## Email Verification

```rust
use larastvel_core::auth::EmailVerificationBroker;
use larastvel_core::mail::LogMailer;
use std::sync::Arc;

let verifier = EmailVerificationBroker::new(
    b"your-app-secret-key",                  // &[u8] JWT signing secret
    Arc::new(LogMailer::new("log")),         // Arc<dyn Mailer>
    "noreply@example.com",                   // from_address
    "http://localhost:8080",                 // app_url
    "MyApp",                                 // app_name
    check_verified,                          // VerificationChecker closure
    mark_verified,                           // MarkVerifiedCallback closure
    3600,                                    // token_expiry_seconds
);

// Send a verification email
verifier.send_verification_email("user-42", "user@example.com").await?;

// Verify a token (returns (user_id, email))
let (user_id, email) = verifier.verify_token("token")?;
```
