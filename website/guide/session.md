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

### Origin Verification

Matching Laravel 13's `PreventRequestForgery`, state-changing requests are also checked against the `Sec-Fetch-Site` header. Requests from cross-site origins are rejected with a `419` "Origin mismatch." response, protecting against cross-site request forgery even when a token leaks.

`CsrfLayer` allows tuning the verification:

```rust
use larastvel_core::session::csrf::CsrfLayer;

// Default: cross-site requests must carry Sec-Fetch-Site: same-origin / same-site
let layer = CsrfLayer::new();

// Relax: trust all same-site requests (includes subdomains)
let relaxed = layer.allow_same_site(true);

// Strict: only requests with Sec-Fetch-Site: same-origin pass
let strict = relaxed.use_origin_only(true);
```

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
