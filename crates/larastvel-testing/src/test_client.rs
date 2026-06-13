use std::collections::HashMap;

use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request};
use axum::Router;
use base64::Engine;
use bytes::Bytes;
use serde::Serialize;
use tower::ServiceExt;

use crate::test_response::TestResponse;

pub struct TestClient {
    router: Router,
    default_headers: HeaderMap,
}

impl TestClient {
    pub fn new(router: Router) -> Self {
        Self {
            router,
            default_headers: HeaderMap::new(),
        }
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.default_headers.insert(
            HeaderName::from_bytes(key.as_bytes()).unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
        self
    }

    pub fn with_basic_auth(mut self, username: &str, password: &str) -> Self {
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", username, password));
        self.default_headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {}", encoded)).unwrap(),
        );
        self
    }

    pub fn with_bearer_token(mut self, token: &str) -> Self {
        self.default_headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );
        self
    }

    pub fn with_cookie(mut self, name: &str, value: &str) -> Self {
        let existing = self
            .default_headers
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok().map(|s| s.to_string()))
            .unwrap_or_default();
        let cookie_str = if existing.is_empty() {
            format!("{}={}", name, value)
        } else {
            format!("{}; {}={}", existing, name, value)
        };
        self.default_headers.insert(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&cookie_str).unwrap(),
        );
        self
    }

    pub async fn get(&self, uri: &str) -> TestResponse {
        self.request(Method::GET, uri, Bytes::new()).await
    }

    pub async fn post(&self, uri: &str, body: impl Into<Bytes>) -> TestResponse {
        self.request(Method::POST, uri, body.into()).await
    }

    pub async fn put(&self, uri: &str, body: impl Into<Bytes>) -> TestResponse {
        self.request(Method::PUT, uri, body.into()).await
    }

    pub async fn patch(&self, uri: &str, body: impl Into<Bytes>) -> TestResponse {
        self.request(Method::PATCH, uri, body.into()).await
    }

    pub async fn delete(&self, uri: &str) -> TestResponse {
        self.request(Method::DELETE, uri, Bytes::new()).await
    }

    pub async fn post_json<T: Serialize>(&self, uri: &str, data: &T) -> TestResponse {
        let body = serde_json::to_vec(data).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        self.request_with_headers(Method::POST, uri, Bytes::from(body), headers)
            .await
    }

    pub async fn put_json<T: Serialize>(&self, uri: &str, data: &T) -> TestResponse {
        let body = serde_json::to_vec(data).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        self.request_with_headers(Method::PUT, uri, Bytes::from(body), headers)
            .await
    }

    pub async fn post_form(&self, uri: &str, data: &HashMap<&str, &str>) -> TestResponse {
        let body = serde_urlencoded::to_string(data).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        self.request_with_headers(Method::POST, uri, Bytes::from(body), headers)
            .await
    }

    async fn request(&self, method: Method, uri: &str, body: Bytes) -> TestResponse {
        let empty = HeaderMap::new();
        self.request_with_headers(method, uri, body, empty).await
    }

    async fn request_with_headers(
        &self,
        method: Method,
        uri: &str,
        body: Bytes,
        extra_headers: HeaderMap,
    ) -> TestResponse {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .body(axum::body::Body::from(body))
            .unwrap();

        for (key, value) in &self.default_headers {
            req.headers_mut().insert(key.clone(), value.clone());
        }
        for (key, value) in extra_headers {
            if let Some(key) = key {
                req.headers_mut().insert(key, value);
            }
        }

        let response = self.router.clone().oneshot(req).await.unwrap();

        let (parts, body) = response.into_parts();
        let status = parts.status;
        let headers = parts.headers;

        let bytes = axum::body::to_bytes(body, usize::MAX)
            .await
            .unwrap_or_default();

        TestResponse::new(status, headers, bytes)
    }
}
