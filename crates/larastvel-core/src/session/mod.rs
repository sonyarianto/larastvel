use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::Json,
};
use rand::RngCore;
use serde_json::Value;

pub mod csrf;
pub mod middleware;

#[derive(Debug, Clone)]
pub struct Session {
    pub(crate) id: String,
    pub(crate) data: HashMap<String, String>,
    pub(crate) flash_new: HashMap<String, String>,
    pub(crate) flash_old: HashMap<String, String>,
    pub(crate) csrf_token: String,
    pub(crate) modified: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        let mut rng = rand::rngs::OsRng;
        let mut id_bytes = [0u8; 20];
        rng.fill_bytes(&mut id_bytes);
        let id = hex::encode(id_bytes);

        let mut token_bytes = [0u8; 32];
        rng.fill_bytes(&mut token_bytes);
        let csrf_token = hex::encode(token_bytes);

        Self {
            id,
            data: HashMap::new(),
            flash_new: HashMap::new(),
            flash_old: HashMap::new(),
            csrf_token,
            modified: false,
        }
    }

    pub fn from_data(
        data: HashMap<String, String>,
        flash_new: HashMap<String, String>,
        csrf_token: String,
    ) -> Self {
        let mut rng = rand::rngs::OsRng;
        let mut id_bytes = [0u8; 20];
        rng.fill_bytes(&mut id_bytes);
        let id = hex::encode(id_bytes);

        Self {
            id,
            data,
            flash_new,
            flash_old: HashMap::new(),
            csrf_token,
            modified: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn csrf_token(&self) -> &str {
        &self.csrf_token
    }

    pub fn regenerate(&mut self) {
        let mut rng = rand::rngs::OsRng;
        let mut id_bytes = [0u8; 20];
        rng.fill_bytes(&mut id_bytes);
        self.id = hex::encode(id_bytes);
        self.modified = true;
    }

    pub fn regenerate_csrf(&mut self) {
        let mut rng = rand::rngs::OsRng;
        let mut token_bytes = [0u8; 32];
        rng.fill_bytes(&mut token_bytes);
        self.csrf_token = hex::encode(token_bytes);
        self.modified = true;
    }

    pub fn put(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
        self.modified = true;
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    pub fn forget(&mut self, key: &str) {
        self.data.remove(key);
        self.modified = true;
    }

    pub fn flush(&mut self) {
        self.data.clear();
        self.modified = true;
    }

    pub fn all(&self) -> &HashMap<String, String> {
        &self.data
    }

    pub fn flash(&mut self, key: &str, value: &str) {
        self.flash_new.insert(key.to_string(), value.to_string());
        self.modified = true;
    }

    pub fn get_flash(&self, key: &str) -> Option<&str> {
        self.flash_old.get(key).map(|s| s.as_str())
    }

    pub fn reflash(&mut self) {
        for (k, v) in self.flash_old.drain() {
            self.flash_new.insert(k, v);
        }
        self.modified = true;
    }

    pub fn keep(&mut self, keys: &[&str]) {
        for key in keys {
            if let Some(v) = self.flash_old.remove(*key) {
                self.flash_new.insert(key.to_string(), v);
            }
        }
        self.modified = true;
    }

    pub fn all_flash(&self) -> (&HashMap<String, String>, &HashMap<String, String>) {
        (&self.flash_new, &self.flash_old)
    }

    #[allow(dead_code)]
    pub(crate) fn age_flash(&mut self) {
        self.flash_old = std::mem::take(&mut self.flash_new);
    }

    pub fn to_payload(&self) -> String {
        let payload = serde_json::json!({
            "data": self.data,
            "flash_new": self.flash_new,
            "csrf_token": self.csrf_token,
        });
        payload.to_string()
    }

    pub fn from_payload(payload: &str) -> Option<Self> {
        let parsed: Value = serde_json::from_str(payload).ok()?;
        let data: HashMap<String, String> =
            serde_json::from_value(parsed.get("data")?.clone()).ok()?;
        let flash_new: HashMap<String, String> =
            serde_json::from_value(parsed.get("flash_new")?.clone()).ok()?;
        let csrf_token: String = serde_json::from_value(parsed.get("csrf_token")?.clone()).ok()?;
        Some(Self::from_data(data, flash_new, csrf_token))
    }

    pub fn has(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn pull(&mut self, key: &str) -> Option<String> {
        let value = self.data.remove(key);
        if value.is_some() {
            self.modified = true;
        }
        value
    }

    pub fn increment(&mut self, key: &str, by: i64) -> i64 {
        let current = self
            .data
            .get(key)
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        let new = current + by;
        self.put(key, &new.to_string());
        new
    }

    pub fn decrement(&mut self, key: &str, by: i64) -> i64 {
        self.increment(key, -by)
    }
}

#[derive(Debug, Clone)]
pub struct SessionHandle(pub Arc<Mutex<Session>>);

impl SessionHandle {
    pub fn new(session: Session) -> Self {
        Self(Arc::new(Mutex::new(session)))
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let session = self.0.lock().unwrap();
        session.get(key).map(|s| s.to_string())
    }

    pub fn put(&self, key: &str, value: &str) {
        let mut session = self.0.lock().unwrap();
        session.put(key, value);
    }

    pub fn forget(&self, key: &str) {
        let mut session = self.0.lock().unwrap();
        session.forget(key);
    }

    pub fn flush(&self) {
        let mut session = self.0.lock().unwrap();
        session.flush();
    }

    pub fn flash(&self, key: &str, value: &str) {
        let mut session = self.0.lock().unwrap();
        session.flash(key, value);
    }

    pub fn get_flash(&self, key: &str) -> Option<String> {
        let session = self.0.lock().unwrap();
        session.get_flash(key).map(|s| s.to_string())
    }

    pub fn reflash(&self) {
        let mut session = self.0.lock().unwrap();
        session.reflash();
    }

    pub fn keep(&self, keys: &[&str]) {
        let mut session = self.0.lock().unwrap();
        session.keep(keys);
    }

    pub fn csrf_token(&self) -> String {
        let session = self.0.lock().unwrap();
        session.csrf_token().to_string()
    }

    pub fn regenerate_csrf(&self) {
        let mut session = self.0.lock().unwrap();
        session.regenerate_csrf();
    }

    pub fn regenerate(&self) {
        let mut session = self.0.lock().unwrap();
        session.regenerate();
    }

    pub fn id(&self) -> String {
        let session = self.0.lock().unwrap();
        session.id().to_string()
    }

    pub fn all(&self) -> HashMap<String, String> {
        let session = self.0.lock().unwrap();
        session.all().clone()
    }

    pub fn has(&self, key: &str) -> bool {
        let session = self.0.lock().unwrap();
        session.has(key)
    }

    pub fn pull(&self, key: &str) -> Option<String> {
        let mut session = self.0.lock().unwrap();
        session.pull(key)
    }

    pub fn increment(&self, key: &str, by: i64) -> i64 {
        let mut session = self.0.lock().unwrap();
        session.increment(key, by)
    }

    pub fn decrement(&self, key: &str, by: i64) -> i64 {
        let mut session = self.0.lock().unwrap();
        session.decrement(key, by)
    }
}

impl<S> FromRequestParts<S> for SessionHandle
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let session = parts
            .extensions
            .get::<SessionHandle>()
            .cloned()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": "Session not initialized. Add SessionLayer to the router."
                    })),
                )
            })?;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let s = Session::new();
        assert!(!s.id.is_empty());
        assert!(!s.csrf_token.is_empty());
        assert!(s.data.is_empty());
        assert!(!s.modified);
    }

    #[test]
    fn test_session_put_get() {
        let mut s = Session::new();
        s.put("name", "larastvel");
        assert_eq!(s.get("name"), Some("larastvel"));
        assert!(s.is_modified());
    }

    #[test]
    fn test_session_forget() {
        let mut s = Session::new();
        s.put("key", "value");
        s.forget("key");
        assert_eq!(s.get("key"), None);
    }

    #[test]
    fn test_session_flush() {
        let mut s = Session::new();
        s.put("a", "1");
        s.put("b", "2");
        s.flush();
        assert!(s.data.is_empty());
    }

    #[test]
    fn test_session_flash() {
        let mut s = Session::new();
        s.flash("status", "saved");
        assert!(s.flash_new.contains_key("status"));
        assert!(!s.flash_old.contains_key("status"));
    }

    #[test]
    fn test_session_age_flash() {
        let mut s = Session::new();
        s.flash("status", "saved");
        s.age_flash();
        assert!(s.flash_new.is_empty());
        assert_eq!(s.get_flash("status"), Some("saved"));
    }

    #[test]
    fn test_session_get_flash_after_age() {
        let mut s = Session::new();
        s.flash("msg", "hello");
        s.age_flash();
        assert_eq!(s.get_flash("msg"), Some("hello"));
    }

    #[test]
    fn test_session_reflash() {
        let mut s = Session::new();
        s.flash("key", "val");
        s.age_flash();
        s.reflash();
        assert!(s.flash_new.contains_key("key"));
        assert!(s.flash_old.is_empty());
    }

    #[test]
    fn test_session_keep() {
        let mut s = Session::new();
        s.flash("a", "1");
        s.flash("b", "2");
        s.age_flash();
        s.keep(&["a"]);
        assert!(s.flash_new.contains_key("a"));
        assert!(!s.flash_new.contains_key("b"));
    }

    #[test]
    fn test_session_payload_roundtrip() {
        let mut s = Session::new();
        s.put("name", "larastvel");
        s.flash("msg", "hello");
        let payload = s.to_payload();
        let restored = Session::from_payload(&payload).unwrap();
        assert_eq!(restored.get("name"), Some("larastvel"));
        assert!(restored.flash_new.contains_key("msg"));
        assert_eq!(restored.csrf_token, s.csrf_token);
    }

    #[test]
    fn test_session_payload_invalid() {
        assert!(Session::from_payload("not json").is_none());
        assert!(Session::from_payload("{}").is_none());
    }

    #[test]
    fn test_session_has() {
        let mut s = Session::new();
        assert!(!s.has("key"));
        s.put("key", "value");
        assert!(s.has("key"));
    }

    #[test]
    fn test_session_pull() {
        let mut s = Session::new();
        s.put("key", "value");
        assert_eq!(s.pull("key"), Some("value".to_string()));
        assert!(!s.has("key"));
    }

    #[test]
    fn test_session_increment() {
        let mut s = Session::new();
        assert_eq!(s.increment("count", 1), 1);
        assert_eq!(s.increment("count", 5), 6);
    }

    #[test]
    fn test_session_decrement() {
        let mut s = Session::new();
        s.put("count", "10");
        assert_eq!(s.decrement("count", 3), 7);
    }

    #[test]
    fn test_session_regenerate() {
        let mut s = Session::new();
        let old_id = s.id.clone();
        s.regenerate();
        assert_ne!(s.id, old_id);
        assert!(s.is_modified());
    }

    #[test]
    fn test_session_regenerate_csrf() {
        let mut s = Session::new();
        let old_token = s.csrf_token.clone();
        s.regenerate_csrf();
        assert_ne!(s.csrf_token, old_token);
        assert!(s.is_modified());
    }

    #[test]
    fn test_session_handle_get() {
        let s = SessionHandle::new(Session::new());
        s.put("key", "value");
        assert_eq!(s.get("key"), Some("value".to_string()));
    }

    #[test]
    fn test_session_handle_flash() {
        let s = SessionHandle::new(Session::new());
        s.flash("status", "done");
        let session = s.0.lock().unwrap();
        assert!(session.flash_new.contains_key("status"));
    }

    #[test]
    fn test_session_handle_csrf() {
        let s = SessionHandle::new(Session::new());
        let token = s.csrf_token();
        assert!(!token.is_empty());
    }

    #[test]
    fn test_session_from_data() {
        let data = HashMap::from([("key".to_string(), "val".to_string())]);
        let flash = HashMap::from([("flash_key".to_string(), "flash_val".to_string())]);
        let s = Session::from_data(data, flash, "csrf123".to_string());
        assert_eq!(s.get("key"), Some("val"));
        assert!(s.flash_new.contains_key("flash_key"));
        assert_eq!(s.csrf_token, "csrf123");
    }
}
