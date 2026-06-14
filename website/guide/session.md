# Session & CSRF

Sessions are encrypted cookie-based and auto-wired when `app.key` is configured.

## How It Works

When `config.app.key` is set, `Application::run()` automatically creates two middleware layers:

1. **SessionLayer** (outermost) — decrypts the session cookie, loads the `SessionHandle`
2. **CsrfLayer** (inside SessionLayer) — validates CSRF tokens

Routes matching `/api/*` and `/health` are automatically CSRF-excepted.

## Session Usage

```rust
use larastvel_core::session::SessionHandle;

async fn handler(session: SessionHandle) -> impl IntoResponse {
    // Read
    let count: Option<i32> = session.get("counter").await.unwrap();

    // Write
    session.set("counter", count.unwrap_or(0) + 1).await.unwrap();

    // Flash data
    session.flash("status", "Saved!").await.unwrap();

    // Remove
    session.remove("counter").await.unwrap();
}
```

## CSRF Protection

CSRF tokens are validated via:

- `X-CSRF-TOKEN` header (AJAX/SPA)
- `X-XSRF-TOKEN` header (Axios/Vite)
- `_token` form field (HTML forms)

Validation uses constant-time comparison via `subtle::ConstantEq`.

### Get CSRF Token in Templates

```html
<form method="POST" action="/submit">
    @csrf
    <input name="title">
    <button>Submit</button>
</form>
```

The `@csrf` Blade directive renders `<input type="hidden" name="_token" value="...">`.

## Session Configuration

Session behavior is controlled via `SessionConfig`:

```rust
SessionConfig {
    cookie_name: "larastvel_session".into(),
    secure: false,      // true in production
    http_only: true,
    same_site: "lax".into(),
    lifetime_minutes: 120,
}
```
