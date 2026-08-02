use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{Method, Request as AxumRequest, StatusCode, Uri};
use axum::response::Response;
use tower::{Layer, Service};

use super::SessionHandle;

#[derive(Debug, Clone)]
pub struct CsrfLayer {
    except: Vec<String>,
    allow_same_site: bool,
    origin_only: bool,
}

impl CsrfLayer {
    pub fn new() -> Self {
        Self {
            except: Vec::new(),
            allow_same_site: false,
            origin_only: false,
        }
    }

    /// Exclude URIs from CSRF validation.
    ///
    /// Supports trailing `*` globs (e.g. `"/webhook/*"`).
    pub fn except(mut self, uris: Vec<String>) -> Self {
        self.except = uris;
        self
    }

    /// Allow requests with `Sec-Fetch-Site: same-site` to bypass token
    /// verification, mirroring `PreventRequestForgery::allowSameSite()`.
    pub fn allow_same_site(mut self, allow: bool) -> Self {
        self.allow_same_site = allow;
        self
    }

    /// Rely solely on origin verification, rejecting requests without a valid
    /// origin even when a token is present, mirroring
    /// `PreventRequestForgery::useOriginOnly()`.
    pub fn use_origin_only(mut self, origin_only: bool) -> Self {
        self.origin_only = origin_only;
        self
    }
}

impl Default for CsrfLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for CsrfLayer
where
    S: Clone + Send + 'static,
{
    type Service = CsrfService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CsrfService {
            inner,
            except: self.except.clone(),
            allow_same_site: self.allow_same_site,
            origin_only: self.origin_only,
        }
    }
}

#[derive(Clone)]
pub struct CsrfService<S> {
    inner: S,
    except: Vec<String>,
    allow_same_site: bool,
    origin_only: bool,
}

impl<S> Service<AxumRequest<Body>> for CsrfService<S>
where
    S: Service<AxumRequest<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Infallible>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: AxumRequest<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        let except = self.except.clone();
        let allow_same_site = self.allow_same_site;
        let origin_only = self.origin_only;

        Box::pin(async move {
            if !is_mutating(request.method()) {
                return Ok(inner
                    .call(request)
                    .await
                    .unwrap_or_else(|e| match e.into() {}));
            }

            if is_excepted(request.uri(), &except) {
                return Ok(inner
                    .call(request)
                    .await
                    .unwrap_or_else(|e| match e.into() {}));
            }

            match verify_origin(request.headers(), allow_same_site, origin_only) {
                OriginVerdict::Pass => {
                    return Ok(inner
                        .call(request)
                        .await
                        .unwrap_or_else(|e| match e.into() {}));
                }
                OriginVerdict::Rejected => return Ok(csrf_origin_failed_response()),
                OriginVerdict::TokenRequired => {}
            }

            let session = match request.extensions().get::<SessionHandle>().cloned() {
                Some(s) => s,
                None => {
                    return Ok(csrf_misconfigured_response());
                }
            };

            let expected = session.csrf_token();

            if let Some(token) = request
                .headers()
                .get("X-CSRF-TOKEN")
                .and_then(|v| v.to_str().ok())
            {
                if constant_time_eq(token, &expected) {
                    return Ok(inner
                        .call(request)
                        .await
                        .unwrap_or_else(|e| match e.into() {}));
                }
            }

            if let Some(token) = request
                .headers()
                .get("X-XSRF-TOKEN")
                .and_then(|v| v.to_str().ok())
            {
                if constant_time_eq(token, &expected) {
                    return Ok(inner
                        .call(request)
                        .await
                        .unwrap_or_else(|e| match e.into() {}));
                }
            }

            let (parts, body) = request.into_parts();
            match axum::body::to_bytes(body, 1_048_576).await {
                Ok(bytes) => {
                    let body_str = String::from_utf8_lossy(&bytes);

                    if let Some(token) = extract_form_token(&body_str) {
                        if constant_time_eq(token, &expected) {
                            let request = AxumRequest::from_parts(parts, Body::from(bytes));
                            return Ok(inner
                                .call(request)
                                .await
                                .unwrap_or_else(|e| match e.into() {}));
                        }
                    }

                    Ok(csrf_failed_response())
                }
                Err(_) => Ok(csrf_failed_response()),
            }
        })
    }
}

/// Outcome of Laravel 13-style origin verification via the `Sec-Fetch-Site`
/// header, mirroring `PreventRequestForgery::hasValidOrigin()`.
enum OriginVerdict {
    /// `Sec-Fetch-Site: same-origin` (or `same-site` when allowed).
    Pass,
    /// No valid origin and `useOriginOnly` is enabled — reject outright.
    Rejected,
    /// No valid origin — fall through to token verification.
    TokenRequired,
}

fn verify_origin(
    headers: &axum::http::HeaderMap,
    allow_same_site: bool,
    origin_only: bool,
) -> OriginVerdict {
    let sec_fetch_site = headers.get("Sec-Fetch-Site").and_then(|v| v.to_str().ok());

    match sec_fetch_site {
        Some("same-origin") => OriginVerdict::Pass,
        Some("same-site") if allow_same_site => OriginVerdict::Pass,
        _ if origin_only => OriginVerdict::Rejected,
        _ => OriginVerdict::TokenRequired,
    }
}

fn csrf_misconfigured_response() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "message": "Session not initialized: ensure SessionLayer is configured before CsrfLayer",
            })
            .to_string(),
        ))
        .unwrap()
}

fn csrf_failed_response() -> Response {
    Response::builder()
        .status(StatusCode::from_u16(419).unwrap())
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "message": "CSRF token mismatch",
                "exception": "Symfony\\Component\\HttpKernel\\Exception\\HttpException",
            })
            .to_string(),
        ))
        .unwrap()
}

fn csrf_origin_failed_response() -> Response {
    Response::builder()
        .status(StatusCode::from_u16(419).unwrap())
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "message": "Origin mismatch.",
                "exception": "Illuminate\\Http\\Exceptions\\OriginMismatchException",
            })
            .to_string(),
        ))
        .unwrap()
}

fn is_mutating(method: &Method) -> bool {
    matches!(
        method,
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    )
}

fn is_excepted(uri: &Uri, except: &[String]) -> bool {
    let path = uri.path();
    except.iter().any(|pattern| {
        if let Some(prefix) = pattern.strip_suffix('*') {
            path.starts_with(prefix.trim_end_matches('*'))
        } else {
            path == pattern
        }
    })
}

fn extract_form_token(body: &str) -> Option<&str> {
    for pair in body.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?;
        if key == "_token" {
            return Some(parts.next().unwrap_or(""));
        }
    }
    None
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
    }

    #[test]
    fn test_is_mutating() {
        assert!(is_mutating(&Method::POST));
        assert!(is_mutating(&Method::PUT));
        assert!(is_mutating(&Method::PATCH));
        assert!(is_mutating(&Method::DELETE));
        assert!(!is_mutating(&Method::GET));
        assert!(!is_mutating(&Method::HEAD));
        assert!(!is_mutating(&Method::OPTIONS));
    }

    #[test]
    fn test_is_excepted() {
        let except = vec!["/webhook".to_string(), "/api/*".to_string()];
        assert!(is_excepted(&Uri::from_static("/webhook"), &except));
        assert!(is_excepted(&Uri::from_static("/api/users"), &except));
        assert!(!is_excepted(&Uri::from_static("/submit"), &except));
    }

    #[test]
    fn test_extract_form_token() {
        assert_eq!(
            extract_form_token("_token=abc123&name=test"),
            Some("abc123")
        );
        assert_eq!(
            extract_form_token("name=test&_token=xyz789"),
            Some("xyz789")
        );
        assert_eq!(extract_form_token("name=test"), None);
    }

    #[test]
    fn test_verify_origin_same_origin_passes() {
        let headers = axum::http::HeaderMap::from_iter([(
            "Sec-Fetch-Site".parse().unwrap(),
            "same-origin".parse().unwrap(),
        )]);
        assert!(matches!(
            verify_origin(&headers, false, false),
            OriginVerdict::Pass
        ));
    }

    #[test]
    fn test_verify_origin_same_site_requires_flag() {
        let headers = axum::http::HeaderMap::from_iter([(
            "Sec-Fetch-Site".parse().unwrap(),
            "same-site".parse().unwrap(),
        )]);
        assert!(matches!(
            verify_origin(&headers, false, false),
            OriginVerdict::TokenRequired
        ));
        assert!(matches!(
            verify_origin(&headers, true, false),
            OriginVerdict::Pass
        ));
    }

    #[test]
    fn test_verify_origin_cross_site() {
        let headers = axum::http::HeaderMap::from_iter([(
            "Sec-Fetch-Site".parse().unwrap(),
            "cross-site".parse().unwrap(),
        )]);
        assert!(matches!(
            verify_origin(&headers, true, false),
            OriginVerdict::TokenRequired
        ));
    }

    #[test]
    fn test_verify_origin_missing_header() {
        let headers = axum::http::HeaderMap::new();
        assert!(matches!(
            verify_origin(&headers, false, false),
            OriginVerdict::TokenRequired
        ));
    }

    #[test]
    fn test_verify_origin_origin_only_rejects() {
        let headers = axum::http::HeaderMap::from_iter([(
            "Sec-Fetch-Site".parse().unwrap(),
            "cross-site".parse().unwrap(),
        )]);
        assert!(matches!(
            verify_origin(&headers, true, true),
            OriginVerdict::Rejected
        ));
    }

    #[test]
    fn test_verify_origin_origin_only_still_passes_same_origin() {
        let headers = axum::http::HeaderMap::from_iter([(
            "Sec-Fetch-Site".parse().unwrap(),
            "same-origin".parse().unwrap(),
        )]);
        assert!(matches!(
            verify_origin(&headers, false, true),
            OriginVerdict::Pass
        ));
    }
}
