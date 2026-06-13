use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub max_attempts: u64,
    pub decay_seconds: u64,
    pub name: String,
}

impl RateLimitConfig {
    pub fn per_second(max_attempts: u64) -> Self {
        Self {
            max_attempts,
            decay_seconds: 1,
            name: format!("{}_per_sec", max_attempts),
        }
    }

    pub fn per_minute(max_attempts: u64) -> Self {
        Self {
            max_attempts,
            decay_seconds: 60,
            name: format!("{}_per_min", max_attempts),
        }
    }

    pub fn per_hour(max_attempts: u64) -> Self {
        Self {
            max_attempts,
            decay_seconds: 3600,
            name: format!("{}_per_hr", max_attempts),
        }
    }

    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    fn key(&self, identifier: &str) -> String {
        format!("{}:{}", self.name, identifier)
    }
}

#[derive(Debug, Clone)]
struct WindowState {
    attempts: u64,
    window_start: Instant,
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    state: Arc<Mutex<HashMap<String, WindowState>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn attempts(&self, identifier: &str) -> u64 {
        let key = self.config.key(identifier);
        let state = self.state.lock().unwrap();
        if let Some(ws) = state.get(&key) {
            if ws.window_start.elapsed() < Duration::from_secs(self.config.decay_seconds) {
                return ws.attempts;
            }
        }
        0
    }

    pub fn remaining(&self, identifier: &str) -> u64 {
        self.config.max_attempts.saturating_sub(self.attempts(identifier))
    }

    pub fn too_many_attempts(&self, identifier: &str) -> bool {
        self.attempts(identifier) >= self.config.max_attempts
    }

    pub fn hit(&self, identifier: &str) -> u64 {
        let key = self.config.key(identifier);
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();

        let ws = state.entry(key).or_insert(WindowState {
            attempts: 0,
            window_start: now,
        });

        if ws.window_start.elapsed() >= Duration::from_secs(self.config.decay_seconds) {
            ws.attempts = 0;
            ws.window_start = now;
        }

        ws.attempts += 1;
        ws.attempts
    }

    pub fn reset(&self, identifier: &str) {
        let key = self.config.key(identifier);
        self.state.lock().unwrap().remove(&key);
    }

    pub fn retry_after(&self, identifier: &str) -> u64 {
        let key = self.config.key(identifier);
        let state = self.state.lock().unwrap();
        if let Some(ws) = state.get(&key) {
            let elapsed = ws.window_start.elapsed().as_secs();
            if elapsed < self.config.decay_seconds {
                return self.config.decay_seconds - elapsed;
            }
        }
        0
    }

    #[cfg(test)]
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiterRegistry {
    limiters: Arc<Mutex<HashMap<String, RateLimiter>>>,
}

impl RateLimiterRegistry {
    pub fn new() -> Self {
        Self {
            limiters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, limiter: RateLimiter) {
        self.limiters
            .lock()
            .unwrap()
            .insert(limiter.config.name.clone(), limiter);
    }

    pub fn get(&self, name: &str) -> Option<RateLimiter> {
        self.limiters.lock().unwrap().get(name).cloned()
    }

    pub fn limiter(&self, name: &str) -> Option<RateLimiter> {
        self.get(name)
    }

    pub fn for_name(&self, name: &str, identifier: &str) -> Option<u64> {
        self.get(name).map(|l| l.attempts(identifier))
    }
}

impl Default for RateLimiterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn rate_limiter(config: RateLimitConfig) -> RateLimiter {
    RateLimiter::new(config)
}

#[derive(Debug)]
pub struct RateLimitExceeded {
    pub retry_after: u64,
    pub limiter_name: String,
}

impl IntoResponse for RateLimitExceeded {
    fn into_response(self) -> Response {
        let body = json!({
            "error": format!("Rate limit exceeded. Try again in {} seconds.", self.retry_after),
            "retry_after": self.retry_after,
        });
        let retry_after_str = self.retry_after.to_string();
        (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", retry_after_str.as_str())],
            Json(body),
        )
            .into_response()
    }
}

pub async fn rate_limit_middleware(
    req: Request,
    next: Next,
) -> Result<Response, RateLimitExceeded> {
    let registry = req.extensions().get::<RateLimiterRegistry>().cloned();
    let ip_header = resolve_client_ip(&req);

    let Some(registry) = registry else {
        return Ok(next.run(req).await);
    };

    let limiters = registry.limiters.lock().unwrap();
    let limiter_names: Vec<String> = limiters.keys().cloned().collect();
    drop(limiters);

    for name in &limiter_names {
        if let Some(limiter) = registry.limiter(name) {
            if limiter.too_many_attempts(&ip_header) {
                return Err(RateLimitExceeded {
                    retry_after: limiter.retry_after(&ip_header),
                    limiter_name: name.clone(),
                });
            }
            limiter.hit(&ip_header);
        }
    }

    Ok(next.run(req).await)
}

fn resolve_client_ip(req: &Request) -> String {
    let headers = req.headers();
    if let Some(val) = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
    {
        return val.to_string();
    }
    if let Some(val) = headers
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
    {
        return val.to_string();
    }
    if let Some(addr) = req
        .extensions()
        .get::<std::net::SocketAddr>()
    {
        return addr.ip().to_string();
    }
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_per_minute_config() {
        let config = RateLimitConfig::per_minute(60);
        assert_eq!(config.max_attempts, 60);
        assert_eq!(config.decay_seconds, 60);
        assert_eq!(config.name, "60_per_min");
    }

    #[test]
    fn test_per_second_config() {
        let config = RateLimitConfig::per_second(5);
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.decay_seconds, 1);
    }

    #[test]
    fn test_per_hour_config() {
        let config = RateLimitConfig::per_hour(100);
        assert_eq!(config.max_attempts, 100);
        assert_eq!(config.decay_seconds, 3600);
    }

    #[test]
    fn test_named_config() {
        let config = RateLimitConfig::per_minute(10).named("login");
        assert_eq!(config.name, "login");
    }

    #[test]
    fn test_hit_and_attempts() {
        let limiter = RateLimiter::new(RateLimitConfig::per_second(3));
        assert_eq!(limiter.hit("user:1"), 1);
        assert_eq!(limiter.hit("user:1"), 2);
        assert_eq!(limiter.attempts("user:1"), 2);
        assert_eq!(limiter.remaining("user:1"), 1);
        assert!(!limiter.too_many_attempts("user:1"));
    }

    #[test]
    fn test_rate_limit_exceeded() {
        let limiter = RateLimiter::new(RateLimitConfig::per_second(2));
        assert_eq!(limiter.hit("user:1"), 1);
        assert_eq!(limiter.hit("user:1"), 2);
        assert!(limiter.too_many_attempts("user:1"));
        assert_eq!(limiter.remaining("user:1"), 0);
    }

    #[test]
    fn test_independent_identifiers() {
        let limiter = RateLimiter::new(RateLimitConfig::per_second(2));
        limiter.hit("user:1");
        limiter.hit("user:1");
        assert!(limiter.too_many_attempts("user:1"));
        assert_eq!(limiter.attempts("user:2"), 0);
        assert!(!limiter.too_many_attempts("user:2"));
    }

    #[test]
    fn test_reset() {
        let limiter = RateLimiter::new(RateLimitConfig::per_second(2));
        limiter.hit("user:1");
        limiter.hit("user:1");
        assert!(limiter.too_many_attempts("user:1"));
        limiter.reset("user:1");
        assert_eq!(limiter.attempts("user:1"), 0);
    }

    #[test]
    fn test_retry_after() {
        let limiter = RateLimiter::new(RateLimitConfig::per_second(60));
        limiter.hit("user:1");
        let retry = limiter.retry_after("user:1");
        assert!(retry > 0 && retry <= 60);
    }

    #[test]
    fn test_zero_retry_after_for_unknown_key() {
        let limiter = RateLimiter::new(RateLimitConfig::per_second(60));
        assert_eq!(limiter.retry_after("nonexistent"), 0);
    }

    #[test]
    fn test_registry() {
        let registry = RateLimiterRegistry::new();
        let limiter = RateLimiter::new(RateLimitConfig::per_minute(60).named("api"));
        registry.register(limiter);

        let retrieved = registry.get("api");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().config.max_attempts, 60);

        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_for_name() {
        let registry = RateLimiterRegistry::new();
        let limiter = RateLimiter::new(RateLimitConfig::per_minute(10).named("login"));
        registry.register(limiter);

        let result = registry.for_name("login", "user:a");
        assert_eq!(result, Some(0));
    }

    #[test]
    fn test_rate_limit_exceeded_into_response() {
        let err = RateLimitExceeded {
            retry_after: 30,
            limiter_name: "api".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers().get("Retry-After").unwrap().to_str().unwrap(),
            "30"
        );
    }

    #[test]
    fn test_rate_limiter_free_fn() {
        let limiter = rate_limiter(RateLimitConfig::per_second(5));
        assert_eq!(limiter.config.max_attempts, 5);
    }

    #[test]
    fn test_different_limiters_independent() {
        let api = RateLimiter::new(RateLimitConfig::per_minute(60).named("api"));
        let login = RateLimiter::new(RateLimitConfig::per_minute(3).named("login"));

        api.hit("user:1");
        api.hit("user:1");
        assert_eq!(api.attempts("user:1"), 2);
        assert_eq!(login.attempts("user:1"), 0);
    }

    #[test]
    fn test_middleware_checks_via_registry() {
        let registry = RateLimiterRegistry::new();
        let limiter = RateLimiter::new(RateLimitConfig::per_second(1).named("test"));
        registry.register(limiter);

        let retrieved = registry.limiter("test").unwrap();
        assert_eq!(retrieved.hit("127.0.0.1"), 1);
        assert!(retrieved.too_many_attempts("127.0.0.1"));
    }
}
