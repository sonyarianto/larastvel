use std::sync::Arc;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
    routing::MethodRouter,
};
use serde_json::json;

use crate::auth::{auth_middleware, require_verified_email, AuthenticatedUser};
use crate::rate_limiter::{RateLimitConfig, RateLimiter};

use super::{cors_middleware, request_logger};

/// Preset that wraps `auth_middleware` (JWT bearer-token authentication).
///
/// Returns 401 if the request lacks a valid `Authorization: Bearer <token>` header.
///
/// # Example
///
/// ```ignore
/// registrar.middleware("auth", larastvel_core::middleware::presets::auth());
/// ```
pub fn auth() -> impl Fn(MethodRouter) -> MethodRouter {
    |r| r.layer(axum::middleware::from_fn(auth_middleware))
}

/// Preset that wraps `cors_middleware` (permissive CORS headers).
///
/// Sets `Access-Control-Allow-Origin: *` plus common methods and headers.
///
/// # Example
///
/// ```ignore
/// registrar.middleware("cors", larastvel_core::middleware::presets::cors());
/// ```
pub fn cors() -> impl Fn(MethodRouter) -> MethodRouter {
    |r| r.layer(axum::middleware::from_fn(cors_middleware))
}

/// Preset that wraps `request_logger` (Tracing request/response logging).
///
/// Logs every request and its response status via `tracing::info!`.
///
/// # Example
///
/// ```ignore
/// registrar.middleware("log", larastvel_core::middleware::presets::logger());
/// ```
pub fn logger() -> impl Fn(MethodRouter) -> MethodRouter {
    |r| r.layer(axum::middleware::from_fn(request_logger))
}

/// Preset that wraps `require_verified_email` middleware.
///
/// Returns 401/403 if the user has not verified their email.
/// Requires `AuthenticatedUser` and `EmailVerificationBroker` in request extensions.
///
/// # Example
///
/// ```ignore
/// registrar.middleware("verified", larastvel_core::middleware::presets::verified());
/// ```
pub fn verified() -> impl Fn(MethodRouter) -> MethodRouter {
    |r| r.layer(axum::middleware::from_fn(require_verified_email))
}

/// Preset for guest-only routes — blocks authenticated users.
///
/// Returns 403 if the request carries a valid JWT bearer token; passes through otherwise.
///
/// # Example
///
/// ```ignore
/// registrar.middleware("guest", larastvel_core::middleware::presets::guest());
/// ```
pub fn guest() -> impl Fn(MethodRouter) -> MethodRouter {
    |r| r.layer(axum::middleware::from_fn(guest_middleware))
}

async fn guest_middleware(
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    if req.extensions().get::<AuthenticatedUser>().is_some() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Already authenticated."})),
        ));
    }
    Ok(next.run(req).await)
}

/// Preset for rate-limiting / throttling routes using an in-memory sliding window.
///
/// Creates a dedicated `RateLimiter` with the given config and spawns a
/// per-route middleware that checks this specific limiter against the
/// client IP.  The limiter is shared across all routes that reference the
/// same preset instance, so a single named preset can be registered once and
/// applied to multiple routes.
///
/// # Example
///
/// ```ignore
/// use larastvel_core::rate_limiter::RateLimitConfig;
///
/// registrar.middleware(
///     "throttle:60,1",
///     larastvel_core::middleware::presets::throttle(
///         RateLimitConfig::per_minute(60),
///     ),
/// );
/// ```
pub fn throttle(config: RateLimitConfig) -> impl Fn(MethodRouter) -> MethodRouter {
    let limiter = Arc::new(RateLimiter::new(config));
    move |r| {
        let limiter = Arc::clone(&limiter);
        r.layer(axum::middleware::from_fn(move |req: Request, next: Next| {
            let limiter = Arc::clone(&limiter);
            async move {
                let ip_header = resolve_client_ip(&req);
                if limiter.too_many_attempts(&ip_header) {
                    let retry_after = limiter.retry_after(&ip_header);
                    let body = json!({
                        "error": format!("Rate limit exceeded. Try again in {} seconds.", retry_after),
                        "retry_after": retry_after,
                    });
                    let mut resp = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
                    resp.headers_mut().insert(
                        "Retry-After",
                        retry_after.to_string().parse().unwrap(),
                    );
                    return Err(resp);
                }
                limiter.hit(&ip_header);
                Ok(next.run(req).await)
            }
        }))
    }
}

fn resolve_client_ip(req: &Request) -> String {
    let headers = req.headers();
    if let Some(val) = headers.get("X-Forwarded-For").and_then(|v| v.to_str().ok()) {
        return val.to_string();
    }
    if let Some(val) = headers.get("X-Real-IP").and_then(|v| v.to_str().ok()) {
        return val.to_string();
    }
    if let Some(addr) = req.extensions().get::<std::net::SocketAddr>() {
        return addr.ip().to_string();
    }
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::Registrar;
    use axum::Router as AxumRouter;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    #[test]
    fn test_guest_preset_allows_anonymous() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.middleware("guest", guest());
        registrar.with_middleware(vec!["guest"]);
        registrar.get("/login", || async { "login page" });

        let app = registrar.build();
        let resp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.oneshot(
                    axum::http::Request::builder()
                        .method("GET")
                        .uri("/login")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[test]
    fn test_guest_preset_blocks_authenticated() {
        use crate::auth::Claims;

        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.middleware("guest", guest());
        registrar.with_middleware(vec!["guest"]);
        registrar.get("/login", || async { "login page" });

        let app = registrar.build();
        let mut req = axum::http::Request::builder()
            .method("GET")
            .uri("/login")
            .body(axum::body::Body::empty())
            .unwrap();
        req.extensions_mut().insert(AuthenticatedUser {
            user_id: "user-1".to_string(),
            claims: Claims {
                sub: "user-1".to_string(),
                exp: 9999999999,
                iat: 0,
            },
        });

        let resp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(app.oneshot(req))
            .unwrap();
        assert_eq!(resp.status(), 403);
    }

    #[test]
    fn test_throttle_preset_allows_within_limit() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.middleware("throttle", throttle(RateLimitConfig::per_second(5)));
        registrar.with_middleware(vec!["throttle"]);
        registrar.get("/api", || async { "ok" });

        let app = registrar.build();
        let resp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.oneshot(
                    axum::http::Request::builder()
                        .method("GET")
                        .uri("/api")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[test]
    fn test_throttle_preset_exceeds_limit() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.middleware("throttle", throttle(RateLimitConfig::per_second(2)));
        registrar.with_middleware(vec!["throttle"]);
        registrar.get("/api", || async { "ok" });

        let app = registrar.build();
        let rt = tokio::runtime::Runtime::new().unwrap();

        // 2 requests should pass
        for _ in 0..2 {
            let resp = rt
                .block_on(
                    app.clone().oneshot(
                        axum::http::Request::builder()
                            .method("GET")
                            .uri("/api")
                            .body(axum::body::Body::empty())
                            .unwrap(),
                    ),
                )
                .unwrap();
            assert_eq!(resp.status(), 200);
        }

        // 3rd request should be rate limited
        let resp = rt.block_on(
            app.oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/api")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            ),
        );
        let resp = resp.unwrap();
        assert_eq!(resp.status(), 429);
        assert!(resp.headers().get("Retry-After").is_some());
    }

    #[test]
    fn test_cors_preset_smoke() {
        let _mw = cors();
    }

    #[test]
    fn test_auth_preset_smoke() {
        let _mw = auth();
    }

    #[test]
    fn test_logger_preset_smoke() {
        let _mw = logger();
    }

    #[test]
    fn test_verified_preset_smoke() {
        let _mw = verified();
    }

    #[test]
    fn test_guest_preset_smoke() {
        let _mw = guest();
    }

    #[test]
    fn test_throttle_preset_smoke() {
        let _mw = throttle(RateLimitConfig::per_minute(60));
    }
}
