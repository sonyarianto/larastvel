# Logging

Larastvel uses the `tracing` crate for structured, async-aware logging.

## Configuration

Configure logging in `config/logging.toml`:

```toml
level = "debug"
format = "text"   # or "json"
```

## Usage

```rust
use tracing::{info, debug, warn, error};

info!("User {} logged in", user_id);
debug!("Processing request: {:?}", request);
warn!("Rate limit approaching for IP {}", ip);
error!("Database connection failed: {}", err);
```

## Log Mailer

The `LogMailer` writes email content to the log instead of sending:

```rust
let mailer = LogMailer::new("log");
mailer.send(mailable).await?;
```

## Initialization

Logging is initialized in the application bootstrap:

```rust
use larastvel_core::logging::init as init_logging;

init_logging(&config);
```
