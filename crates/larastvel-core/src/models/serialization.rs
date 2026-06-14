use std::collections::HashMap;

use serde::Serialize;
use serde_json::{json, Value};

/// Trait for model serialization — Laravel's `toArray()` / `toJson()` pattern.
///
/// Models that implement this trait get automatic conversion to JSON with
/// support for:
/// - `hidden()` — fields to exclude from output (like `$hidden`)
/// - `appends()` — computed attributes to include (like `$appends`)
/// - `get_appended()` — compute values for appended attributes
///
/// # Example
///
/// ```rust,ignore
/// impl SerializesToArray for User {
///     fn hidden() -> Vec<&'static str> { vec!["password"] }
///     fn appends() -> Vec<&'static str> { vec!["full_name"] }
///     fn get_appended(&self, key: &str) -> Option<Value> {
///         match key {
///             "full_name" => Some(json!(format!("{} {}", self.first_name, self.last_name))),
///             _ => None,
///         }
///     }
/// }
/// ```
pub trait SerializesToArray: Send + Sync + Serialize {
    /// Fields to exclude from serialization output (like `$hidden`).
    fn hidden() -> Vec<&'static str>
    where
        Self: Sized,
    {
        vec![]
    }

    /// Computed attributes to append to serialization output (like `$appends`).
    fn appends() -> Vec<&'static str>
    where
        Self: Sized,
    {
        vec![]
    }

    /// Return the value for a computed attribute.
    ///
    /// Only called for keys returned by `appends()`.
    fn get_appended(&self, _key: &str) -> Option<Value> {
        None
    }

    /// Convert the model to a JSON [`Value`], respecting `hidden` / `appends`.
    fn to_array(&self) -> Value
    where
        Self: Sized,
    {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Value::Object(ref mut map) = value {
            for key in Self::hidden() {
                map.remove(key);
            }
            for key in Self::appends() {
                if let Some(val) = self.get_appended(key) {
                    map.insert(key.to_string(), val);
                }
            }
        }
        value
    }

    /// Convert the model to a pretty-printed JSON string.
    fn to_json(&self) -> Result<String, serde_json::Error>
    where
        Self: Sized,
    {
        serde_json::to_string_pretty(&self.to_array())
    }

    /// Convert the model to a compact JSON string.
    fn to_json_compact(&self) -> Result<String, serde_json::Error>
    where
        Self: Sized,
    {
        serde_json::to_string(&self.to_array())
    }
}

// ---------------------------------------------------------------------------
// API Resources — JsonResource & ResourceCollection
// ---------------------------------------------------------------------------

/// A trait that lets you customise how a model is transformed for API output.
///
/// This is the Rust equivalent of Laravel's `JsonResource::toArray($request)`.
///
/// # Example
///
/// ```rust,ignore
/// struct UserResource;
///
/// impl ApiResource<User> for UserResource {
///     fn transform(model: &User) -> Value {
///         json!({
///             "id": model.id,
///             "email": model.email,
///         })
///     }
/// }
/// ```
pub trait ApiResource<T: Serialize + Send + Sync + 'static>: Sized {
    /// Transform a single model into the API representation.
    fn transform(model: &T) -> Value;

    /// Create a `JsonResource` wrapping the given model.
    fn make(model: T) -> JsonResource<T, Self> {
        JsonResource {
            inner: model,
            _marker: std::marker::PhantomData,
        }
    }

    /// Create a `ResourceCollection` wrapping the given models.
    fn collection(models: Vec<T>) -> ResourceCollection<T, Self> {
        ResourceCollection {
            inner: models,
            _marker: std::marker::PhantomData,
        }
    }
}

/// A single model wrapped for API output.
///
/// Use together with an [`ApiResource`] implementor.
pub struct JsonResource<T: Serialize + Send + Sync + 'static, R: ApiResource<T>> {
    inner: T,
    _marker: std::marker::PhantomData<R>,
}

impl<T: Serialize + Send + Sync + 'static, R: ApiResource<T>> JsonResource<T, R> {
    /// Return the API representation as a JSON [`Value`].
    pub fn to_array(&self) -> Value {
        R::transform(&self.inner)
    }

    /// Return the API representation as a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.to_array())
    }

    /// Consume the resource and return the inner model.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

/// A collection of models wrapped for API output.
///
/// Laravel-compatible JSON structure:
/// ```json
/// { "data": [...], "meta": { "count": ... } }
/// ```
pub struct ResourceCollection<T: Serialize + Send + Sync + 'static, R: ApiResource<T>> {
    inner: Vec<T>,
    _marker: std::marker::PhantomData<R>,
}

impl<T: Serialize + Send + Sync + 'static, R: ApiResource<T>> ResourceCollection<T, R> {
    /// Return the API representation as a JSON [`Value`].
    ///
    /// Includes `data` and `meta` (count) keys.
    pub fn to_array(&self) -> Value {
        let data: Vec<Value> = self.inner.iter().map(|item| R::transform(item)).collect();
        json!({
            "data": data,
            "meta": {
                "count": self.inner.len(),
            }
        })
    }

    /// Return the API representation as a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.to_array())
    }

    /// Add additional meta keys to the response.
    pub fn with_meta(self, extra: HashMap<String, Value>) -> Value {
        let mut base = self.to_array();
        if let Value::Object(ref mut map) = base {
            if let Some(Value::Object(ref mut meta)) = map.get_mut("meta") {
                for (k, v) in extra {
                    meta.insert(k, v);
                }
            }
        }
        base
    }

    /// Consume the collection and return the inner models.
    pub fn into_inner(self) -> Vec<T> {
        self.inner
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use serde::Serialize;

    #[derive(Debug, Clone, Serialize)]
    struct User {
        id: u64,
        name: String,
        email: String,
        password: String,
    }

    impl SerializesToArray for User {
        fn hidden() -> Vec<&'static str> {
            vec!["password"]
        }

        fn appends() -> Vec<&'static str> {
            vec!["greeting"]
        }

        fn get_appended(&self, key: &str) -> Option<Value> {
            match key {
                "greeting" => Some(json!("Hello, ".to_string() + &self.name)),
                _ => None,
            }
        }
    }

    #[test]
    fn test_to_array_hides_fields() {
        let user = User {
            id: 1,
            name: "Alice".into(),
            email: "alice@test.com".into(),
            password: "secret".into(),
        };
        let arr = user.to_array();
        assert!(arr.get("password").is_none());
        assert_eq!(arr.get("id").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(arr.get("name").and_then(|v| v.as_str()), Some("Alice"));
    }

    #[test]
    fn test_to_array_appends_fields() {
        let user = User {
            id: 2,
            name: "Bob".into(),
            email: "bob@test.com".into(),
            password: "hunter2".into(),
        };
        let arr = user.to_array();
        assert_eq!(
            arr.get("greeting").and_then(|v| v.as_str()),
            Some("Hello, Bob")
        );
    }

    #[test]
    fn test_to_json_roundtrip() {
        let user = User {
            id: 3,
            name: "Charlie".into(),
            email: "charlie@test.com".into(),
            password: "p4ss".into(),
        };
        let json_str = user.to_json().unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.get("id").and_then(|v| v.as_u64()), Some(3));
        assert!(parsed.get("password").is_none());
    }

    #[test]
    fn test_to_json_compact_no_newlines() {
        let user = User {
            id: 1,
            name: "Diana".into(),
            email: "diana@test.com".into(),
            password: "x".into(),
        };
        let compact = user.to_json_compact().unwrap();
        assert!(!compact.contains('\n'));
        assert!(compact.contains("Diana"));
    }

    #[test]
    fn test_default_hidden_and_appends() {
        #[derive(Debug, Clone, Serialize)]
        struct Simple {
            id: u64,
            label: String,
        }

        impl SerializesToArray for Simple {}

        let s = Simple {
            id: 10,
            label: "test".into(),
        };
        let arr = s.to_array();
        assert_eq!(arr.get("id").and_then(|v| v.as_u64()), Some(10));
        assert_eq!(arr.get("label").and_then(|v| v.as_str()), Some("test"));
    }

    // -----------------------------------------------------------------------
    // API Resource tests
    // -----------------------------------------------------------------------

    #[derive(Debug, Clone, Serialize)]
    struct Product {
        sku: String,
        price: f64,
        internal_note: String,
    }

    struct ProductResource;

    impl ApiResource<Product> for ProductResource {
        fn transform(model: &Product) -> Value {
            json!({
                "sku": model.sku,
                "price": model.price,
            })
        }
    }

    #[test]
    fn test_json_resource_to_array() {
        let product = Product {
            sku: "ABC-123".into(),
            price: 29.99,
            internal_note: "ignore me".into(),
        };
        let resource = ProductResource::make(product);
        let arr = resource.to_array();
        assert_eq!(arr.get("sku").and_then(|v| v.as_str()), Some("ABC-123"));
        assert_eq!(arr.get("price").and_then(|v| v.as_f64()), Some(29.99));
        assert!(arr.get("internal_note").is_none());
    }

    #[test]
    fn test_resource_collection_to_array() {
        let products = vec![
            Product {
                sku: "P1".into(),
                price: 10.0,
                internal_note: "x".into(),
            },
            Product {
                sku: "P2".into(),
                price: 20.0,
                internal_note: "y".into(),
            },
        ];
        let collection = ProductResource::collection(products);
        let arr = collection.to_array();
        assert!(arr.get("data").is_some());
        assert!(arr.get("meta").is_some());
        let data = arr["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(arr["meta"]["count"].as_u64(), Some(2));
    }

    #[test]
    fn test_resource_collection_with_extra_meta() {
        let products = vec![Product {
            sku: "P1".into(),
            price: 10.0,
            internal_note: "x".into(),
        }];
        let mut extra = HashMap::new();
        extra.insert("page".to_string(), json!(1));
        extra.insert("total".to_string(), json!(100));

        let arr = ProductResource::collection(products).with_meta(extra);
        assert_eq!(arr["meta"]["page"].as_u64(), Some(1));
        assert_eq!(arr["meta"]["total"].as_u64(), Some(100));
        assert_eq!(arr["meta"]["count"].as_u64(), Some(1));
    }

    #[test]
    fn test_json_resource_into_inner() {
        let product = Product {
            sku: "INNER".into(),
            price: 5.0,
            internal_note: "note".into(),
        };
        let resource = ProductResource::make(product);
        let inner = resource.into_inner();
        assert_eq!(inner.sku, "INNER");
    }

    #[test]
    fn test_resource_collection_into_inner() {
        let products = vec![Product {
            sku: "C1".into(),
            price: 1.0,
            internal_note: "n".into(),
        }];
        let collection = ProductResource::collection(products);
        let inner = collection.into_inner();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].sku, "C1");
    }
}
