//! Signed URLs — Laravel's `URL::signedRoute()` / `URL::temporarySignedRoute()`.
//!
//! A URL is signed by appending an HMAC-SHA256 signature over its canonical
//! path + query (sorted, `expires` included when present), keyed with the
//! application key. `has_valid_signature()` recomputes the signature and
//! enforces expiry, so tampered or expired links are rejected.

use sha2::{Digest, Sha256};
use std::time::Duration;

/// Error returned when a signed URL cannot be built.
#[derive(Debug)]
pub enum SignedUrlError {
    /// The application key is empty; generate one with `key:generate`.
    MissingKey,
}

impl std::fmt::Display for SignedUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingKey => write!(f, "cannot sign URL: application key is not set"),
        }
    }
}

impl std::error::Error for SignedUrlError {}

const HMAC_BLOCK_SIZE: usize = 64;

/// HMAC-SHA256 (RFC 2104) built on `sha2` — no extra dependencies.
pub(crate) fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut key = key.to_vec();
    if key.len() > HMAC_BLOCK_SIZE {
        key = Sha256::digest(&key).to_vec();
    }
    key.resize(HMAC_BLOCK_SIZE, 0);

    let mut ipad = vec![0x36u8; HMAC_BLOCK_SIZE];
    let mut opad = vec![0x5cu8; HMAC_BLOCK_SIZE];
    for (i, b) in key.iter().enumerate() {
        ipad[i] ^= b;
        opad[i] ^= b;
    }

    let mut inner = Vec::with_capacity(HMAC_BLOCK_SIZE + message.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(message);
    let inner_hash = Sha256::digest(&inner);

    let mut outer = Vec::with_capacity(HMAC_BLOCK_SIZE + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    Sha256::digest(&outer).to_vec()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn canonical_query(params: &[(String, String)]) -> String {
    let mut pairs: Vec<&(String, String)> = params.iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&")
}

/// Build a signed URL — Laravel's `URL::signedRoute()`.
///
/// `path` is the route path (`/users/5`), `params` are additional query
/// parameters. With a `ttl`, an `expires` timestamp is embedded and the link
/// stops validating after the duration — `temporarySignedRoute()`.
///
/// ```rust,ignore
/// let url = signed_route("/verify", &[("user", "5")], None, &config.app.key.unwrap().into_bytes())?;
/// ```
pub fn signed_route(
    path: &str,
    params: &[(&str, &str)],
    ttl: Option<Duration>,
    key: &[u8],
) -> Result<String, SignedUrlError> {
    if key.is_empty() {
        return Err(SignedUrlError::MissingKey);
    }

    let mut pairs: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    if let Some(ttl) = ttl {
        pairs.push((
            "expires".to_string(),
            (unix_now() + ttl.as_secs()).to_string(),
        ));
    }

    let query = canonical_query(&pairs);
    let full = if query.is_empty() {
        path.to_string()
    } else {
        format!("{}?{}", path, query)
    };

    let signature = hex::encode(hmac_sha256(key, full.as_bytes()));
    let sep = if query.is_empty() { "?" } else { "&" };
    Ok(format!("{}{}signature={}", full, sep, signature))
}

/// Verify a signed path + query string (e.g. the URI of an incoming request)
/// — Laravel's `SignedRequest` middleware. Returns false for tampered,
/// re-signed-with-different-key, or expired URLs.
///
/// ```rust,ignore
/// if has_valid_signature(request_uri, &key) {
///     // proceed with the verified action
/// }
/// ```
pub fn has_valid_signature(path_and_query: &str, key: &[u8]) -> bool {
    if key.is_empty() {
        return false;
    }

    let (path, query) = match path_and_query.split_once('?') {
        Some((p, q)) => (p, q),
        None => (path_and_query, ""),
    };

    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut provided_signature: Option<String> = None;
    for pair in query.split('&').filter(|s| !s.is_empty()) {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        if k == "signature" {
            provided_signature = Some(v.to_string());
        } else {
            pairs.push((k.to_string(), v.to_string()));
        }
    }

    let Some(signature) = provided_signature else {
        return false;
    };

    if let Some(expires) = pairs.iter().find(|(k, _)| k == "expires") {
        let Ok(expires) = expires.1.parse::<u64>() else {
            return false;
        };
        if unix_now() > expires {
            return false;
        }
    }

    let canonical = canonical_query(&pairs);
    let full = if canonical.is_empty() {
        path.to_string()
    } else {
        format!("{}?{}", path, canonical)
    };
    let expected = hex::encode(hmac_sha256(key, full.as_bytes()));

    constant_time_eq(expected.as_bytes(), signature.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"test-application-key-32-bytes-long!!";

    #[test]
    fn test_signed_route_roundtrip() {
        let url = signed_route("/users/5", &[("user", "5")], None, KEY).unwrap();
        assert!(url.starts_with("/users/5?user=5&signature="));
        assert!(has_valid_signature(&url, KEY));
    }

    #[test]
    fn test_signed_route_canonical_param_order() {
        let a = signed_route("/x", &[("a", "1"), ("b", "2")], None, KEY).unwrap();
        let b = signed_route("/x", &[("b", "2"), ("a", "1")], None, KEY).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_tampered_param_invalid() {
        let url = signed_route("/users/5", &[("user", "5")], None, KEY).unwrap();
        let tampered = url.replace("user=5", "user=6");
        assert!(!has_valid_signature(&tampered, KEY));
    }

    #[test]
    fn test_wrong_key_invalid() {
        let url = signed_route("/users/5", &[], None, KEY).unwrap();
        assert!(!has_valid_signature(&url, b"a-different-key"));
    }

    #[test]
    fn test_missing_signature_invalid() {
        assert!(!has_valid_signature("/users/5?user=5", KEY));
    }

    #[test]
    fn test_expired_url_invalid() {
        let past = unix_now() - 10;
        let full = format!("/verify?expires={}", past);
        let sig = hex::encode(hmac_sha256(KEY, full.as_bytes()));
        let url = format!("{}&signature={}", full, sig);
        assert!(!has_valid_signature(&url, KEY));
    }

    #[test]
    fn test_signed_route_missing_key_errors() {
        let result = signed_route("/x", &[], None, b"");
        assert!(matches!(result, Err(SignedUrlError::MissingKey)));
    }

    #[test]
    fn test_hmac_matches_rfc_vector() {
        // RFC 4231 test case 1: key=0x0b x20, data="Hi There"
        let key = [0x0bu8; 20];
        let message = b"Hi There";
        let expected = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
        assert_eq!(hex::encode(hmac_sha256(&key, message)), expected);
    }
}
