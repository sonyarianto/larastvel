use std::sync::Arc;

use axum::{
    http::{header::SET_COOKIE, HeaderValue},
};
use tower::Layer;

use super::{Session, SessionHandle};
use crate::encryption::Encrypter;

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub cookie_name: String,
    pub lifetime_minutes: u64,
    pub path: String,
    pub domain: Option<String>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
}

#[derive(Debug, Clone)]
pub enum SameSite {
    Lax,
    Strict,
    None,
}

impl SameSite {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Lax => "Lax",
            Self::Strict => "Strict",
            Self::None => "None",
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            cookie_name: "larastvel_session".to_string(),
            lifetime_minutes: 120,
            path: "/".to_string(),
            domain: None,
            secure: false,
            http_only: true,
            same_site: SameSite::Lax,
        }
    }
}

#[derive(Clone)]
pub struct SessionLayer {
    pub config: SessionConfig,
    pub encrypter: Option<Arc<Encrypter>>,
}

impl SessionLayer {
    pub fn new(config: SessionConfig, encrypter: Option<Arc<Encrypter>>) -> Self {
        Self { config, encrypter }
    }

    pub fn default_with_key(key: &[u8]) -> Self {
        let encrypter = Arc::new(Encrypter::new(key).expect("Invalid encryption key for sessions"));
        Self {
            config: SessionConfig::default(),
            encrypter: Some(encrypter),
        }
    }
}

impl<S> Layer<S> for SessionLayer
where
    S: Clone + Send + 'static,
{
    type Service = SessionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionService {
            inner,
            config: self.config.clone(),
            encrypter: self.encrypter.clone(),
        }
    }
}

#[derive(Clone)]
pub struct SessionService<S> {
    inner: S,
    config: SessionConfig,
    encrypter: Option<Arc<Encrypter>>,
}

impl<S, ReqBody, ResBody> tower::Service<axum::extract::Request<ReqBody>> for SessionService<S>
where
    S: tower::Service<axum::extract::Request<ReqBody>, Response = axum::response::Response<ResBody>>,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    ReqBody: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = axum::response::Response<ResBody>;
    type Error = S::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&self, request: axum::extract::Request<ReqBody>) -> Self::Future {
        let inner = self.inner.clone();
        let config = self.config.clone();
        let encrypter = self.encrypter.clone();

        Box::pin(async move {
            let session = load_session_from_request(&request, &config, encrypter.as_deref());
            let handle = SessionHandle::new(session);

            let mut request = request;
            request.extensions_mut().insert(handle.clone());

            let mut response: axum::response::Response<ResBody> = inner.call(request).await.map_err(Into::into)?;

            save_session_to_response(&handle, &mut response, &config, encrypter.as_deref());

            Ok(response)
        })
    }
}

fn load_session_from_request(
    request: &axum::extract::Request<impl std::any::Any>,
    config: &SessionConfig,
    encrypter: Option<&Encrypter>,
) -> Session {
    let cookie_header = request
        .headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok());

    if let Some(cookie_str) = cookie_header {
        for cookie in cookie_str.split(';') {
            let cookie = cookie.trim();
            if let Some(value) = cookie.strip_prefix(&format!("{}=", config.cookie_name)) {
                let decoded = url_encoded_decode(value);

                if let Some(enc) = encrypter {
                    if let Ok(plaintext) = enc.decrypt(&decoded) {
                        if let Some(session) = Session::from_payload(&plaintext) {
                            return session;
                        }
                    }
                } else if let Some(session) = Session::from_payload(&decoded) {
                    return session;
                }
            }
        }
    }

    let mut session = Session::new();
    session.modified = true;
    session
}

fn save_session_to_response<ResBody>(
    handle: &SessionHandle,
    response: &mut axum::response::Response<ResBody>,
    config: &SessionConfig,
    encrypter: Option<&Encrypter>,
) {
    let mut session = handle.0.lock().unwrap();

    if !session.is_modified() && session.flash_old.is_empty() {
        return;
    }

    session.age_flash();

    let payload = session.to_payload();

    let cookie_value = if let Some(enc) = encrypter {
        match enc.encrypt(&payload) {
            Ok(encrypted) => encrypted,
            Err(_) => return,
        }
    } else {
        payload
    };

    let encoded = url_encoded_encode(&cookie_value);

    let cookie_str = format!(
        "{cookie_name}={value}; Path={path}; HttpOnly; SameSite={same_site}{domain}{max_age}",
        cookie_name = &config.cookie_name,
        value = encoded,
        path = &config.path,
        same_site = config.same_site.as_str(),
        domain = config
            .domain
            .as_ref()
            .map(|d| format!("; Domain={}", d))
            .unwrap_or_default(),
        max_age = format!("; Max-Age={}", config.lifetime_minutes * 60),
    );

    response
        .headers_mut()
        .insert(SET_COOKIE, HeaderValue::from_str(&cookie_str).unwrap());
}

fn url_encoded_encode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

fn url_encoded_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encoded_roundtrip() {
        let input = "hello world!@#$%^&*()";
        let encoded = url_encoded_encode(input);
        let decoded = url_encoded_decode(&encoded);
        assert_eq!(input, decoded);
    }

    #[test]
    fn test_url_encode_alphanumeric_unchanged() {
        let input = "abc123XYZ-_.~";
        let encoded = url_encoded_encode(input);
        assert_eq!(input, encoded);
    }

    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert_eq!(config.cookie_name, "larastvel_session");
        assert_eq!(config.lifetime_minutes, 120);
        assert_eq!(config.path, "/");
        assert!(config.http_only);
    }

    #[test]
    fn test_same_site_as_str() {
        assert_eq!(SameSite::Lax.as_str(), "Lax");
        assert_eq!(SameSite::Strict.as_str(), "Strict");
        assert_eq!(SameSite::None.as_str(), "None");
    }

    #[test]
    fn test_layer_creation() {
        let config = SessionConfig::default();
        let layer = SessionLayer::new(config.clone(), None);
        assert_eq!(layer.config.cookie_name, "larastvel_session");
        assert!(layer.encrypter.is_none());
    }
}
