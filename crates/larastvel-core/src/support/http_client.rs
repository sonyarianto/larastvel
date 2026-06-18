//! # HTTP Client
//!
//! A fluent wrapper around `reqwest` for making HTTP requests, inspired by
//! Laravel's `Illuminate\Support\Facades\Http`. Supports bearer tokens, basic
//! auth, custom headers, JSON/form/raw bodies, query parameters, timeouts,
//! retries, and base URLs.
//!
//! ## Example
//!
//! ```rust,no_run
//! use larastvel_core::Http;
//! use std::time::Duration;
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     let response = Http::with_token("your-token")
//!         .timeout(Duration::from_secs(10))
//!         .get("https://api.github.com/user")
//!         .await?;
//!
//!     let status = response.status_code();
//!     let data: serde_json::Value = response.json().await?;
//!     println!("Status: {}", status);
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method, Response as ReqwestResponse};
use serde::Serialize;

use super::collection::Collection;

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),
}

impl From<reqwest::Error> for HttpError {
    fn from(e: reqwest::Error) -> Self {
        HttpError::RequestFailed(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum Body {
    Json(serde_json::Value),
    Form(Vec<(String, String)>),
    Text(String),
    None,
}

#[derive(Debug, Clone)]
pub struct PendingRequest {
    client: Option<Client>,
    base_url: Option<String>,
    headers: HeaderMap,
    query: Vec<(String, String)>,
    timeout: Option<Duration>,
    retry_max: Option<usize>,
    retry_delay: Option<Duration>,
    body: Body,
}

impl Default for PendingRequest {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingRequest {
    pub fn new() -> Self {
        Self {
            client: None,
            base_url: None,
            headers: HeaderMap::new(),
            query: Vec::new(),
            timeout: None,
            retry_max: None,
            retry_delay: None,
            body: Body::None,
        }
    }

    pub fn with_token(mut self, token: &str) -> Self {
        let value = HeaderValue::from_str(&format!("Bearer {}", token)).unwrap();
        self.headers.insert(reqwest::header::AUTHORIZATION, value);
        self
    }

    pub fn with_basic_auth(mut self, username: &str, password: &str) -> Self {
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{}:{}", username, password),
        );
        let value = HeaderValue::from_str(&format!("Basic {}", encoded)).unwrap();
        self.headers.insert(reqwest::header::AUTHORIZATION, value);
        self
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            self.headers.insert(name, value);
        }
        self
    }

    pub fn with_headers(mut self, headers: HashMap<&str, &str>) -> Self {
        for (name, value) in headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                self.headers.insert(name, value);
            }
        }
        self
    }

    pub fn accept(mut self, mime: &str) -> Self {
        let value = HeaderValue::from_str(mime).unwrap();
        self.headers.insert(reqwest::header::ACCEPT, value);
        self
    }

    pub fn accept_json(self) -> Self {
        self.accept("application/json")
    }

    pub fn content_type(mut self, mime: &str) -> Self {
        let value = HeaderValue::from_str(mime).unwrap();
        self.headers.insert(reqwest::header::CONTENT_TYPE, value);
        self
    }

    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    pub fn retry(mut self, max: usize, delay: Duration) -> Self {
        self.retry_max = Some(max);
        self.retry_delay = Some(delay);
        self
    }

    pub fn with_query(mut self, params: Vec<(&str, &str)>) -> Self {
        self.query = params
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self
    }

    pub fn base_url(mut self, url: &str) -> Self {
        self.base_url = Some(url.to_string());
        self
    }

    pub fn body(mut self, body: &str) -> Self {
        self.body = Body::Text(body.to_string());
        self
    }

    pub fn json<T: Serialize>(mut self, value: &T) -> Self {
        let json_value = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
        self.body = Body::Json(json_value);
        self.content_type("application/json")
    }

    pub fn form<T: Serialize>(mut self, value: &T) -> Self {
        let pairs = serde_urlencoded::to_string(value).unwrap_or_default();
        self.body = Body::Text(pairs);
        self.content_type("application/x-www-form-urlencoded")
    }

    pub fn as_form(mut self, pairs: Vec<(&str, &str)>) -> Self {
        let encoded = serde_urlencoded::to_string(&pairs).unwrap_or_default();
        self.body = Body::Text(encoded);
        self.content_type("application/x-www-form-urlencoded")
    }

    async fn build_client(&self) -> Client {
        if let Some(client) = &self.client {
            return client.clone();
        }
        let mut builder = Client::builder();
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        builder.build().unwrap_or_default()
    }

    fn resolve_url(&self, url: &str) -> String {
        match &self.base_url {
            Some(base) => {
                let base = base.trim_end_matches('/');
                let path = url.trim_start_matches('/');
                format!("{}/{}", base, path)
            }
            None => url.to_string(),
        }
    }

    async fn do_request(&self, method: Method, url: &str) -> Result<Response, HttpError> {
        let client = self.build_client().await;
        let url = self.resolve_url(url);

        let mut req = client.request(method.clone(), &url);

        req = req.headers(self.headers.clone());

        if !self.query.is_empty() {
            req = req.query(&self.query);
        }

        req = match &self.body {
            Body::Json(value) => req.json(value),
            Body::Text(text) => req.body(text.clone()),
            Body::Form(pairs) => req.form(pairs),
            Body::None => req,
        };

        let max_retries = self.retry_max.unwrap_or(0);
        let retry_delay = self.retry_delay.unwrap_or(Duration::from_millis(100));

        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                tokio::time::sleep(retry_delay).await;
            }

            match req.try_clone() {
                Some(cloned_req) => match cloned_req.send().await {
                    Ok(resp) => return Ok(Response::new(resp)),
                    Err(e) => {
                        last_error = Some(e);
                    }
                },
                None => {
                    let resp = req
                        .try_clone()
                        .ok_or_else(|| {
                            HttpError::RequestFailed("request body cannot be retried".into())
                        })?
                        .send()
                        .await?;
                    return Ok(Response::new(resp));
                }
            }
        }

        Err(HttpError::RequestFailed(format!(
            "HTTP request failed after {} retries: {:?}",
            max_retries,
            last_error.map(|e| e.to_string()).unwrap_or_default()
        )))
    }

    pub async fn get(&self, url: &str) -> Result<Response, HttpError> {
        self.do_request(Method::GET, url).await
    }

    pub async fn post(&self, url: &str) -> Result<Response, HttpError> {
        self.do_request(Method::POST, url).await
    }

    pub async fn put(&self, url: &str) -> Result<Response, HttpError> {
        self.do_request(Method::PUT, url).await
    }

    pub async fn patch(&self, url: &str) -> Result<Response, HttpError> {
        self.do_request(Method::PATCH, url).await
    }

    pub async fn delete(&self, url: &str) -> Result<Response, HttpError> {
        self.do_request(Method::DELETE, url).await
    }

    pub async fn head(&self, url: &str) -> Result<Response, HttpError> {
        self.do_request(Method::HEAD, url).await
    }
}

#[derive(Debug)]
pub struct Response {
    inner: ReqwestResponse,
}

impl Response {
    pub fn new(response: ReqwestResponse) -> Self {
        Self { inner: response }
    }

    pub fn status(&self) -> reqwest::StatusCode {
        self.inner.status()
    }

    pub fn ok(&self) -> bool {
        self.inner.status().is_success()
    }

    pub fn client_error(&self) -> bool {
        self.inner.status().is_client_error()
    }

    pub fn server_error(&self) -> bool {
        self.inner.status().is_server_error()
    }

    pub fn status_code(&self) -> u16 {
        self.inner.status().as_u16()
    }

    pub async fn text(self) -> Result<String, HttpError> {
        self.inner.text().await.map_err(HttpError::from)
    }

    pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, HttpError> {
        self.inner.json().await.map_err(HttpError::from)
    }

    pub async fn body(self) -> Result<Vec<u8>, HttpError> {
        self.inner
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(HttpError::from)
    }

    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    pub fn header(&self, name: &str) -> Option<&HeaderValue> {
        self.inner.headers().get(name)
    }

    pub async fn collect_headers(self) -> Collection<(String, String)> {
        let headers: Vec<(String, String)> = self
            .inner
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        Collection::new(headers)
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Response {} {}",
            self.status_code(),
            self.status().canonical_reason().unwrap_or("")
        )
    }
}

pub struct Http;

impl Http {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> PendingRequest {
        PendingRequest::new()
    }

    pub fn with_token(token: &str) -> PendingRequest {
        PendingRequest::new().with_token(token)
    }

    pub fn with_basic_auth(username: &str, password: &str) -> PendingRequest {
        PendingRequest::new().with_basic_auth(username, password)
    }

    pub fn accept_json() -> PendingRequest {
        PendingRequest::new().accept_json()
    }

    pub fn timeout(duration: Duration) -> PendingRequest {
        PendingRequest::new().timeout(duration)
    }

    pub fn base_url(url: &str) -> PendingRequest {
        PendingRequest::new().base_url(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_request_new() {
        let req = PendingRequest::new();
        assert!(req.headers.is_empty());
        assert!(req.query.is_empty());
        assert!(req.timeout.is_none());
    }

    #[test]
    fn test_with_token_header() {
        let req = PendingRequest::new().with_token("my-token");
        let auth = req
            .headers
            .get(reqwest::header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(auth, "Bearer my-token");
    }

    #[test]
    fn test_with_header() {
        let req = PendingRequest::new().with_header("X-Custom", "value");
        assert_eq!(
            req.headers.get("x-custom").unwrap().to_str().unwrap(),
            "value"
        );
    }

    #[test]
    fn test_accept_json() {
        let req = PendingRequest::new().accept_json();
        assert_eq!(
            req.headers
                .get(reqwest::header::ACCEPT)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_content_type() {
        let req = PendingRequest::new().content_type("application/xml");
        assert_eq!(
            req.headers
                .get(reqwest::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/xml"
        );
    }

    #[test]
    fn test_with_query() {
        let req = PendingRequest::new().with_query(vec![("key", "value")]);
        assert_eq!(req.query.len(), 1);
        assert_eq!(req.query[0], ("key".to_string(), "value".to_string()));
    }

    #[test]
    fn test_base_url() {
        let req = PendingRequest::new().base_url("https://api.example.com");
        assert_eq!(req.base_url, Some("https://api.example.com".to_string()));
    }

    #[test]
    fn test_resolve_url() {
        let req = PendingRequest::new().base_url("https://api.example.com/");
        assert_eq!(
            req.resolve_url("/v1/users"),
            "https://api.example.com/v1/users"
        );
    }

    #[test]
    fn test_retry_config() {
        let req = PendingRequest::new().retry(3, Duration::from_secs(1));
        assert_eq!(req.retry_max, Some(3));
        assert_eq!(req.retry_delay, Some(Duration::from_secs(1)));
    }

    #[test]
    fn test_timeout_config() {
        let req = PendingRequest::new().timeout(Duration::from_secs(30));
        assert_eq!(req.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_http_facade_new() {
        let req = Http::new();
        assert!(req.headers.is_empty());
    }

    #[test]
    fn test_http_facade_with_token() {
        let req = Http::with_token("token-123");
        let auth = req
            .headers
            .get(reqwest::header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(auth, "Bearer token-123");
    }

    #[test]
    fn test_http_facade_accept_json() {
        let req = Http::accept_json();
        assert!(req.headers.get(reqwest::header::ACCEPT).is_some());
    }

    #[test]
    fn test_json_body_sets_content_type() {
        let req = PendingRequest::new().json(&serde_json::json!({"key": "value"}));
        assert!(matches!(req.body, Body::Json(_)));
        assert_eq!(
            req.headers
                .get(reqwest::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_form_body_sets_content_type() {
        let req = PendingRequest::new().form(&vec![("key", "value")]);
        assert_eq!(
            req.headers
                .get(reqwest::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/x-www-form-urlencoded"
        );
    }
}
