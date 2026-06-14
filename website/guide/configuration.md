# Configuration

Larastvel uses TOML files for configuration. The `config/` directory holds per-section files:

```
config/
├── app.toml
├── broadcasting.toml
├── cache.toml
├── database.toml
├── logging.toml
├── password_reset.toml
└── view.toml
```

A single `config.toml` at the project root still works (legacy format), but `config/` takes precedence.

## App

```toml
# config/app.toml
name = "Larastvel"
url = "http://localhost:8080"
env = "local"
debug = true
key = ""    # generate with `larastvel key:generate`
```

## Database

```toml
# config/database.toml
driver = "sqlite"       # sqlite, postgres, mysql
host = "127.0.0.1"
port = 3306
database = "larastvel"
username = "root"
password = ""
```

## Logging

```toml
# config/logging.toml
level = "debug"
format = "text"         # text, json
```

## View

```toml
# config/view.toml
engine = "tera"
paths = ["resources/views"]
```

## Broadcasting

```toml
# config/broadcasting.toml
default = "log"
# Pusher/Ably credentials:
app_id = ""
key = ""
secret = ""
cluster = "mt1"
encrypted = true
```

## Cache

```toml
# config/cache.toml
default = "array"
prefix = ""
table = "cache"
file_path = "storage/framework/cache/data"
```

## Password Reset

```toml
# config/password_reset.toml
table = "password_reset_tokens"
expire_seconds = 3600
throttle_seconds = 60
```

## Environment Variables

Environment variables override config values at runtime. Use `.env` in your project root:

```bash
APP_KEY=base64:abc123...
DB_HOST=localhost
DB_PASSWORD=secret
```

## Partial Configs

Missing sections and fields use built-in defaults. A config file with just:

```toml
# config/app.toml
key = "base64:abc123..."
```

is perfectly valid — only `key` differs from defaults.
