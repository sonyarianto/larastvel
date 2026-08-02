# Logging

Larastvel uses the `tracing` crate for structured, async-aware logging.

## Configuration

Configure logging in `config/logging.toml`:

```toml
level = "debug"
format = "text"      # or "json"
driver = "console"   # "console" or "monthly"
path = "logs/laravel.log"
max_files = 3
```

## Monthly file driver

The `monthly` driver (Laravel 13.23 parity) writes one log file per calendar
month instead of printing to the console:

```toml
driver = "monthly"
path = "logs/laravel.log"   # writes to logs/laravel-2026-08.log during August
```

`MonthlyWriter` appends to `laravel-YYYY-MM.log`, rotating once per month. Old
monthly files beyond `max_files` (default 3) are pruned at startup.

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
