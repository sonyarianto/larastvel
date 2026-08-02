//! HTTP cookies and Laravel's queueable `CookieJar` (parity with
//! `Illuminate\Cookie\CookieJar`).
//!
//! Cookies are queued with [`CookieJar::queue`] and later flushed into the
//! response's `Set-Cookie` headers. Queued cookies are keyed by their
//! `(name, path)` pair so the same cookie can be queued with a different
//! path without clobbering an earlier one — mirroring the Laravel 13.24
//! `CookieJar::queued()` fix.

use std::collections::HashMap;

/// A single HTTP cookie and its attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// The cookie name.
    pub name: String,
    /// The cookie value.
    pub value: String,
    /// The cookie path (default `/`).
    pub path: String,
    /// The cookie domain, if any.
    pub domain: Option<String>,
    /// Mark the cookie as only transmittable over HTTPS.
    pub secure: bool,
    /// Mark the cookie as inaccessible to JavaScript.
    pub http_only: bool,
    /// Restrict the cookie to a same-site context.
    pub same_site: SameSite,
    /// Maximum lifetime in seconds (`None` = session cookie).
    pub max_age: Option<i64>,
}

/// The `SameSite` restriction applied to a cookie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SameSite {
    /// Only sent for same-site requests.
    Strict,
    /// Sent on same-site requests and top-level cross-site navigations.
    Lax,
    /// Always sent.
    #[default]
    None,
}

impl Cookie {
    /// Build a new cookie with defaults matching Laravel
    /// (`path = /`, `same_site = Lax`, `http_only = true`).
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            path: "/".to_string(),
            domain: None,
            secure: false,
            http_only: true,
            same_site: SameSite::Lax,
            max_age: None,
        }
    }

    /// Set the cookie path.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the cookie domain.
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Mark the cookie secure (HTTPS only).
    pub fn with_secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// Mark the cookie HttpOnly.
    pub fn with_http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    /// Set the SameSite policy.
    pub fn with_same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }

    /// Set the cookie lifetime in seconds.
    pub fn with_max_age(mut self, seconds: i64) -> Self {
        self.max_age = Some(seconds);
        self
    }

    /// Render this cookie as a single `Set-Cookie` header value.
    pub fn to_set_cookie(&self) -> String {
        let mut out = format!("{}={}", self.name, self.value);
        out.push_str(&format!("; Path={}", self.path));
        if let Some(domain) = &self.domain {
            out.push_str(&format!("; Domain={}", domain));
        }
        if self.secure {
            out.push_str("; Secure");
        }
        if self.http_only {
            out.push_str("; HttpOnly");
        }
        let same_site = match self.same_site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None",
        };
        out.push_str(&format!("; SameSite={}", same_site));
        if let Some(max_age) = self.max_age {
            out.push_str(&format!("; Max-Age={}", max_age));
        }
        out
    }
}

/// The key queued cookies are stored under: `(name, path)`.
pub type CookieKey = (String, String);

/// A queueable cookie jar.
#[derive(Debug, Clone, Default)]
pub struct CookieJar {
    queued: HashMap<CookieKey, Cookie>,
}

impl CookieJar {
    /// A new, empty cookie jar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a cookie for the response.
    ///
    /// Queued cookies are keyed by `(name, path)`, so re-queueing the same
    /// name on a different path keeps both entries.
    pub fn queue(&mut self, cookie: impl Into<Cookie>) {
        let cookie = cookie.into();
        let key = (cookie.name.clone(), cookie.path.clone());
        self.queued.insert(key, cookie);
    }

    /// Un-queue a cookie by name and (optionally) path.
    pub fn unqueue(&mut self, name: &str, path: Option<&str>) {
        let path = path.unwrap_or("/").to_string();
        self.queued.remove(&(name.to_string(), path));
    }

    /// The queued cookies, keyed by `(name, path)`.
    pub fn get_queued_cookies(&self) -> &HashMap<CookieKey, Cookie> {
        &self.queued
    }

    /// Retrieve a queued cookie by name and (optionally) path.
    ///
    /// When no path is given, the default `/` is used — matching Laravel's
    /// `CookieJar::queued($name)` return. If a cookie with the same name was
    /// queued under a different path, its own key is returned.
    pub fn queued(&self, name: &str, path: Option<&str>) -> Option<&Cookie> {
        let path = path.unwrap_or("/").to_string();
        self.queued
            .get(&(name.to_string(), path))
            .or_else(|| self.queued.values().find(|c| c.name == name))
    }

    /// All queued cookies as a list.
    pub fn queued_cookies(&self) -> impl Iterator<Item = &Cookie> {
        self.queued.values()
    }

    /// Whether the jar holds a queued cookie with the given name (any path).
    pub fn has_queued(&self, name: &str) -> bool {
        self.queued.keys().any(|(n, _)| n == name)
    }

    /// Flush a queued cookie (e.g. after writing it to a response).
    pub fn forget(&mut self, name: &str, path: Option<&str>) {
        self.unqueue(name, path);
    }

    /// Render all queued cookies as `Set-Cookie` header values.
    pub fn to_set_cookie_headers(&self) -> Vec<String> {
        self.queued.values().map(|c| c.to_set_cookie()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_and_queued_by_default_path() {
        let mut jar = CookieJar::new();
        jar.queue(Cookie::new("session", "abc"));
        let cookie = jar.queued("session", None);
        assert!(cookie.is_some());
        assert_eq!(cookie.unwrap().value, "abc");
    }

    #[test]
    fn same_name_different_paths_do_not_clobber() {
        let mut jar = CookieJar::new();
        jar.queue(Cookie::new("token", "root").path("/"));
        jar.queue(Cookie::new("token", "admin").path("/admin"));

        assert_eq!(jar.queued("token", Some("/")).unwrap().value, "root");
        assert_eq!(jar.queued("token", Some("/admin")).unwrap().value, "admin");
        assert_eq!(jar.queued_cookies().count(), 2);
    }

    #[test]
    fn queued_falls_back_to_any_path_for_name() {
        let mut jar = CookieJar::new();
        jar.queue(Cookie::new("token", "v").path("/weird"));
        assert_eq!(jar.queued("token", None).unwrap().value, "v");
    }

    #[test]
    fn unqueue_removes_by_name_and_path() {
        let mut jar = CookieJar::new();
        jar.queue(Cookie::new("a", "1").path("/"));
        jar.queue(Cookie::new("a", "2").path("/admin"));
        jar.unqueue("a", Some("/admin"));
        assert_eq!(jar.queued_cookies().count(), 1);
        assert_eq!(jar.queued("a", Some("/")).unwrap().value, "1");
    }

    #[test]
    fn forget_removes_cookie() {
        let mut jar = CookieJar::new();
        jar.queue(Cookie::new("sid", "x"));
        jar.forget("sid", None);
        assert!(!jar.has_queued("sid"));
    }

    #[test]
    fn to_set_cookie_header_renders_attributes() {
        let cookie = Cookie::new("name", "value")
            .path("/app")
            .domain("example.com")
            .with_secure(true)
            .with_http_only(true)
            .with_same_site(SameSite::Strict)
            .with_max_age(3600);
        let header = cookie.to_set_cookie();
        assert!(header.starts_with("name=value; Path=/app"));
        assert!(header.contains("Domain=example.com"));
        assert!(header.contains("Secure"));
        assert!(header.contains("HttpOnly"));
        assert!(header.contains("SameSite=Strict"));
        assert!(header.contains("Max-Age=3600"));
    }

    #[test]
    fn builder_queues_cookie() {
        let mut jar = CookieJar::new();
        jar.queue(Cookie::new("legacy", "v").path("/legacy"));
        jar.queue(Cookie::new("legacy", "root"));
        assert_eq!(jar.queued("legacy", Some("/")).unwrap().value, "root");
        assert_eq!(jar.queued("legacy", None).unwrap().value, "root");
        assert_eq!(jar.queued_cookies().count(), 2);
    }
}
