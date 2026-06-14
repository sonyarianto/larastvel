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
}

impl CsrfLayer {
    pub fn new() -> Self {
        Self { except: Vec::new() }
    }

    /// Exclude URIs from CSRF validation.
    ///
    /// Supports trailing `*` globs (e.g. `"/webhook/*"`).
    pub fn except(mut self, uris: Vec<String>) -> Self {
        self.except = uris;
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
        }
    }
}

#[derive(Clone)]
pub struct CsrfService<S> {
    inner: S,
    except: Vec<String>,
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
}
