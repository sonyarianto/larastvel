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
use larastvel_core::auth::PasswordResetBroker;
use larastvel_core::auth::PasswordResetConfig;

let config = PasswordResetConfig::default();
let broker = PasswordResetBroker::new(db, config);

// Create a reset token
broker.create_token("user@example.com").await?;

// Validate and reset
broker.reset("user@example.com", "token", "new-password").await?;
```

## Email Verification

```rust
use larastvel_core::auth::EmailVerificationBroker;

let verifier = EmailVerificationBroker::new(db);
verifier.send_verification_email("user@example.com").await?;
verifier.verify("token").await?;
```
