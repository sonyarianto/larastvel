use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use serde_json::Value;

pub struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

impl TestResponse {
    pub fn new(status: StatusCode, headers: HeaderMap, body: Bytes) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn content(&self) -> &str {
        std::str::from_utf8(&self.body).unwrap_or("")
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    fn text_content(&self) -> String {
        strip_html_tags(self.content())
    }

    pub fn assert_status(&self, status: StatusCode) -> &Self {
        assert_eq!(
            self.status, status,
            "Expected status {:?}, got {:?}",
            status, self.status
        );
        self
    }

    pub fn assert_ok(&self) -> &Self {
        self.assert_status(StatusCode::OK)
    }

    pub fn assert_created(&self) -> &Self {
        self.assert_status(StatusCode::CREATED)
    }

    pub fn assert_no_content(&self) -> &Self {
        self.assert_status(StatusCode::NO_CONTENT)
    }

    pub fn assert_redirect(&self) -> &Self {
        assert!(
            self.status.is_redirection(),
            "Expected a redirect status (3xx), got {:?}",
            self.status
        );
        self
    }

    pub fn assert_not_found(&self) -> &Self {
        self.assert_status(StatusCode::NOT_FOUND)
    }

    pub fn assert_unauthorized(&self) -> &Self {
        self.assert_status(StatusCode::UNAUTHORIZED)
    }

    pub fn assert_forbidden(&self) -> &Self {
        self.assert_status(StatusCode::FORBIDDEN)
    }

    pub fn assert_unprocessable(&self) -> &Self {
        self.assert_status(StatusCode::UNPROCESSABLE_ENTITY)
    }

    pub fn assert_server_error(&self) -> &Self {
        self.assert_status(StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn assert_see(&self, text: &str) -> &Self {
        let content = self.content();
        assert!(
            content.contains(text),
            "Expected to see '{}' in response body.\nBody:\n{}",
            text,
            content
        );
        self
    }

    pub fn assert_dont_see(&self, text: &str) -> &Self {
        let content = self.content();
        assert!(
            !content.contains(text),
            "Expected NOT to see '{}' in response body.\nBody:\n{}",
            text,
            content
        );
        self
    }

    pub fn assert_see_text(&self, text: &str) -> &Self {
        let stripped = self.text_content();
        assert!(
            stripped.contains(text),
            "Expected to see text '{}' in response.\nStripped body:\n{}",
            text,
            stripped
        );
        self
    }

    pub fn assert_see_in_order(&self, texts: &[&str]) -> &Self {
        let content = self.content();
        let mut pos = 0;
        for text in texts {
            let found = content[pos..].find(text);
            assert!(
                found.is_some(),
                "Expected to see '{}' in order after position {}.\nBody:\n{}",
                text,
                pos,
                content
            );
            pos += found.unwrap() + text.len();
        }
        self
    }

    pub fn assert_json(&self, expected: Value) -> &Self {
        let actual = self.json();
        assert_eq!(
            actual,
            expected,
            "JSON response did not match expected.\nExpected:\n{}\n\nActual:\n{}",
            serde_json::to_string_pretty(&expected).unwrap_or_default(),
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        );
        self
    }

    pub fn assert_json_path(&self, path: &str, expected: Value) -> &Self {
        let actual = self.json();
        let pointer = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path.replace('.', "/"))
        };
        let value = actual.pointer(&pointer);
        assert!(
            value.is_some(),
            "Path '{}' not found in JSON response.\nActual:\n{}",
            path,
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        );
        assert_eq!(
            value.unwrap(),
            &expected,
            "JSON path '{}' did not match.\nExpected:\n{}\n\nActual:\n{}",
            path,
            serde_json::to_string_pretty(&expected).unwrap_or_default(),
            serde_json::to_string_pretty(value.unwrap()).unwrap_or_default()
        );
        self
    }

    pub fn assert_json_missing(&self, path: &str) -> &Self {
        let actual = self.json();
        let pointer = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path.replace('.', "/"))
        };
        assert!(
            actual.pointer(&pointer).is_none(),
            "Path '{}' was found in JSON response, but expected it to be missing.\nActual:\n{}",
            path,
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        );
        self
    }

    pub fn assert_json_count(&self, path: &str, count: usize) -> &Self {
        let actual = self.json();
        let pointer = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path.replace('.', "/"))
        };
        let value = actual.pointer(&pointer);
        assert!(
            value.is_some(),
            "Path '{}' not found in JSON response.\nActual:\n{}",
            path,
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        );
        let arr = value.unwrap().as_array();
        assert!(
            arr.is_some(),
            "Path '{}' is not an array.\nActual:\n{}",
            path,
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        );
        assert_eq!(
            arr.unwrap().len(),
            count,
            "Expected {} items at path '{}', found {}.",
            count,
            path,
            arr.unwrap().len()
        );
        self
    }

    pub fn assert_json_structure(&self, keys: &[&str]) -> &Self {
        let actual = self.json();
        let obj = actual.as_object();
        assert!(
            obj.is_some(),
            "Response is not a JSON object.\nActual:\n{}",
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        );
        for key in keys {
            assert!(
                obj.unwrap().contains_key(*key),
                "Key '{}' not found in JSON object.\nActual:\n{}",
                key,
                serde_json::to_string_pretty(&actual).unwrap_or_default()
            );
        }
        self
    }

    pub fn assert_header(&self, key: &str, value: &str) -> &Self {
        let actual = self.headers.get(key).and_then(|v| v.to_str().ok());
        assert_eq!(
            actual,
            Some(value),
            "Expected header '{}' to be '{}', got '{:?}'.",
            key,
            value,
            actual
        );
        self
    }

    pub fn assert_header_missing(&self, key: &str) -> &Self {
        assert!(
            self.headers.get(key).is_none(),
            "Expected header '{}' to be missing, but found it.",
            key
        );
        self
    }
}

fn strip_html_tags(input: &str) -> String {
    let re = regex::Regex::new(r"<[^>]*>").unwrap();
    let spaced = re.replace_all(input, " ");
    let compact = regex::Regex::new(r"\s+").unwrap();
    compact.replace_all(&spaced, " ").trim().to_string()
}
