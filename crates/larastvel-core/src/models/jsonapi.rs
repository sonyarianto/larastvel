use std::collections::{HashMap, HashSet};

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::Response as AxumResponse;
use serde::Serialize;
use serde_json::{json, Value};

/// Parsed JSON:API query string context — the Rust equivalent of Laravel's
/// automatic handling of `?include=` and `?fields[type]=` parameters.
///
/// Parse it from a request's query string and pass it to a
/// [`JsonApiItem`] / [`JsonApiCollection`] via [`JsonApiItem::with_query`]:
///
/// ```rust,ignore
/// let query = JsonApiQuery::parse(uri.query().unwrap_or(""));
/// PostResource::make(post).with_query(&query).include("author", ...)
/// ```
///
/// The default (or [`JsonApiQuery::ignored`]) query performs no filtering and
/// no relationship inclusion — the equivalent of Laravel's
/// `ignoreFieldsAndIncludesInQueryString()`.
#[derive(Debug, Clone, Default)]
pub struct JsonApiQuery {
    fields: HashMap<String, HashSet<String>>,
    includes: HashSet<String>,
}

impl JsonApiQuery {
    /// Parse `include` / `fields[type]` parameters from a raw query string.
    ///
    /// Example: `include=author,comments&fields[posts]=title,created_at`.
    pub fn parse(raw: &str) -> Self {
        let mut query = Self::default();
        for pair in raw.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, value) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
            if key == "include" {
                for inc in value.split(',').filter(|s| !s.is_empty()) {
                    query.includes.insert(inc.to_string());
                }
            } else if let Some(rest) = key.strip_prefix("fields[") {
                if let Some(type_) = rest.strip_suffix(']') {
                    query.fields.entry(type_.to_string()).or_default().extend(
                        value
                            .split(',')
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                    );
                }
            }
        }
        query
    }

    /// A query that ignores the query string entirely — full attributes,
    /// no relationship inclusion (Laravel's `ignoreFieldsAndIncludesInQueryString`).
    pub fn ignored() -> Self {
        Self::default()
    }

    /// Whether the given relationship path was requested via `include=`.
    ///
    /// Nested paths imply their ancestors: requesting `author.posts` also
    /// includes `author`.
    pub fn includes(&self, path: &str) -> bool {
        if self.includes.contains(path) {
            return true;
        }
        let prefix = format!("{path}.");
        self.includes.iter().any(|inc| inc.starts_with(&prefix))
    }

    /// The sparse fieldset requested for a resource type, if any.
    pub fn fieldset(&self, type_: &str) -> Option<&HashSet<String>> {
        self.fields.get(type_)
    }

    /// Filter an `attributes` object by the sparse fieldset for `type_`.
    ///
    /// Per the JSON:API spec this only ever filters `attributes`, never
    /// `relationships`.
    pub fn filter_attributes(&self, type_: &str, attributes: Value) -> Value {
        match self.fields.get(type_) {
            Some(fields) => match attributes {
                Value::Object(map) => Value::Object(
                    map.into_iter()
                        .filter(|(key, _)| fields.contains(key))
                        .collect(),
                ),
                other => other,
            },
            None => attributes,
        }
    }

    /// Whether any include requests are present.
    pub fn has_includes(&self) -> bool {
        !self.includes.is_empty()
    }
}

/// Laravel's `whenIncluded()` — emit a relationship value only when the
/// client requested it via `?include=`.
pub fn when_included(query: &JsonApiQuery, relationship: &str, value: Value) -> Option<Value> {
    query.includes(relationship).then_some(value)
}

/// Laravel's `whenNotIncluded()` — emit a relationship value only when the
/// client did *not* request it via `?include=`.
pub fn when_not_included(query: &JsonApiQuery, relationship: &str, value: Value) -> Option<Value> {
    (!query.includes(relationship)).then_some(value)
}

/// The Rust equivalent of Laravel 13's `Illuminate\Http\Resources\JsonApi\JsonApiResource`.
///
/// Implementors define how a model is transformed into a JSON:API resource
/// object (`id`, `type`, `attributes`, `relationships`, `links`, `meta`).
/// Sparse fieldsets and compound documents are handled automatically.
///
/// # Example
///
/// ```rust,ignore
/// struct PostResource;
///
/// impl JsonApiResource<Post> for PostResource {
///     fn attributes(model: &Post) -> Value {
///         json!({ "title": model.title, "body": model.body })
///     }
///
///     fn relationships(model: &Post, query: &JsonApiQuery) -> Value {
///         json!({
///             "author": when_included(query, "author", json!({
///                 "type": "users", "id": model.author_id.to_string(),
///             })),
///         })
///     }
/// }
/// ```
pub trait JsonApiResource<T: Serialize + Send + Sync + 'static>: Sized {
    /// The resource's `attributes` object (required).
    fn attributes(model: &T) -> Value;

    /// The resource's `id`. Defaults to the model's serialized `id` field
    /// stringified, per the spec (ids must be strings).
    fn id(model: &T) -> String {
        match serde_json::to_value(model)
            .ok()
            .and_then(|value| value.get("id").cloned())
        {
            Some(Value::String(s)) => s,
            Some(value) => value.to_string(),
            None => String::new(),
        }
    }

    /// The resource's `type`. Defaults to the model's type name lowercased,
    /// e.g. `User` → `user`. Override for pluralised / custom types.
    fn type_() -> String {
        let full = std::any::type_name::<T>();
        let last = full.rsplit("::").next().unwrap_or(full);
        let last = last.split('<').next().unwrap_or(last);
        last.to_ascii_lowercase()
    }

    /// The resource's `relationships` object. Use [`when_included`] /
    /// [`when_not_included`] to make entries conditional on the request.
    fn relationships(_model: &T, _query: &JsonApiQuery) -> Value {
        json!({})
    }

    /// The resource's `links` object (e.g. a `self` link).
    fn links(_model: &T) -> Value {
        json!({})
    }

    /// The resource's `meta` object.
    fn meta(_model: &T) -> Value {
        json!({})
    }

    /// Create a single-resource document builder.
    fn make(model: T) -> JsonApiItem<T, Self> {
        JsonApiItem {
            inner: model,
            query: JsonApiQuery::default(),
            included: vec![],
            additional: Value::Object(Default::default()),
            _marker: std::marker::PhantomData,
        }
    }

    /// Create a collection document builder.
    fn collection(models: Vec<T>) -> JsonApiCollection<T, Self> {
        JsonApiCollection {
            inner: models,
            query: JsonApiQuery::default(),
            included: vec![],
            links: Value::Object(Default::default()),
            meta: Value::Object(Default::default()),
            _marker: std::marker::PhantomData,
        }
    }
}

type Included = Vec<Value>;

type IncludedEntry = (
    String,
    Box<dyn Fn(&JsonApiQuery) -> Vec<Value> + Send + Sync>,
);

fn resource_object<T: Serialize + Send + Sync + 'static, R: JsonApiResource<T>>(
    model: &T,
    query: &JsonApiQuery,
) -> Value {
    let mut object = json!({
        "id": R::id(model),
        "type": R::type_(),
        "attributes": query.filter_attributes(&R::type_(), R::attributes(model)),
    });
    let relationships = filter_null_entries(R::relationships(model, query));
    if !is_empty_object(&relationships) {
        object["relationships"] = relationships;
    }
    let links = R::links(model);
    if !is_empty_object(&links) {
        object["links"] = links;
    }
    let meta = R::meta(model);
    if !is_empty_object(&meta) {
        object["meta"] = meta;
    }
    object
}

fn is_empty_object(value: &Value) -> bool {
    value.as_object().is_none_or(|map| map.is_empty())
}

/// Remove `null` entries (conditional values that were not requested),
/// mirroring Laravel's handling of conditional relationships.
fn filter_null_entries(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).collect())
        }
        other => other,
    }
}

fn included_resource_objects(included: &[IncludedEntry], query: &JsonApiQuery) -> Included {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut objects: Vec<Value> = Vec::new();
    for (path, resolve) in included {
        if !query.includes(path) {
            continue;
        }
        for object in resolve(query) {
            let key = (
                object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                object
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            );
            if seen.insert(key) {
                objects.push(object);
            }
        }
    }
    objects
}

/// A single-resource JSON:API document builder.
///
/// ```rust,ignore
/// let query = JsonApiQuery::parse("include=author&fields[posts]=title");
/// let doc = PostResource::make(post)
///     .with_query(&query)
///     .include("author", AuthorResource::make(author))
///     .to_array();
/// ```
pub struct JsonApiItem<T: Serialize + Send + Sync + 'static, R: JsonApiResource<T>> {
    inner: T,
    query: JsonApiQuery,
    included: Vec<IncludedEntry>,
    additional: Value,
    _marker: std::marker::PhantomData<R>,
}

impl<T: Serialize + Send + Sync + 'static, R: JsonApiResource<T>> JsonApiItem<T, R> {
    /// Apply the parsed query string context (sparse fieldsets, includes).
    pub fn with_query(mut self, query: &JsonApiQuery) -> Self {
        self.query = query.clone();
        self
    }

    /// Ignore the query string — return full attributes and no includes.
    pub fn ignore_query_string(mut self) -> Self {
        self.query = JsonApiQuery::ignored();
        self
    }

    /// Register a related single resource to appear in the top-level
    /// `included` array when `path` is requested via `?include=`.
    pub fn include<
        T2: Serialize + Send + Sync + 'static,
        R2: JsonApiResource<T2> + Send + Sync + 'static,
    >(
        mut self,
        path: &str,
        child: JsonApiItem<T2, R2>,
    ) -> Self {
        let path = path.to_string();
        self.included
            .push((path, Box::new(move |query| child.resource_objects(query))));
        self
    }

    /// Register a related collection to appear in the top-level `included`
    /// array when `path` is requested via `?include=`.
    pub fn include_collection<
        T2: Serialize + Send + Sync + 'static,
        R2: JsonApiResource<T2> + Send + Sync + 'static,
    >(
        mut self,
        path: &str,
        child: JsonApiCollection<T2, R2>,
    ) -> Self {
        let path = path.to_string();
        self.included
            .push((path, Box::new(move |query| child.resource_objects(query))));
        self
    }

    /// Merge additional top-level keys (e.g. `meta`, `links`) into the
    /// document — the equivalent of Laravel's `additional()`.
    pub fn additional(mut self, data: Value) -> Self {
        self.additional = data;
        self
    }

    /// Build the resource object.
    pub fn resource_object(&self, query: &JsonApiQuery) -> Value {
        resource_object::<T, R>(&self.inner, query)
    }

    /// The resource objects for the document (itself).
    pub fn resource_objects(&self, query: &JsonApiQuery) -> Vec<Value> {
        vec![self.resource_object(query)]
    }

    /// Render the full document: `{ "data": {...}, "included": [...] }`.
    pub fn to_array(&self) -> Value {
        let mut document = json!({ "data": self.resource_object(&self.query) });
        let included = included_resource_objects(&self.included, &self.query);
        if !included.is_empty() {
            document["included"] = Value::Array(included);
        }
        merge_document(&mut document, &self.additional);
        document
    }

    /// Render the document as a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.to_array())
    }

    /// Consume the builder and return the inner model.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

/// A collection-resource JSON:API document builder.
///
/// ```rust,ignore
/// let doc = PostResource::collection(posts)
///     .with_query(&query)
///     .include("author", AuthorResource::make(author))
///     .with_links(json!({ "self": "/api/posts" }))
///     .to_array();
/// ```
pub struct JsonApiCollection<T: Serialize + Send + Sync + 'static, R: JsonApiResource<T>> {
    inner: Vec<T>,
    query: JsonApiQuery,
    included: Vec<IncludedEntry>,
    links: Value,
    meta: Value,
    _marker: std::marker::PhantomData<R>,
}

impl<T: Serialize + Send + Sync + 'static, R: JsonApiResource<T>> JsonApiCollection<T, R> {
    /// Apply the parsed query string context (sparse fieldsets, includes).
    pub fn with_query(mut self, query: &JsonApiQuery) -> Self {
        self.query = query.clone();
        self
    }

    /// Ignore the query string — return full attributes and no includes.
    pub fn ignore_query_string(mut self) -> Self {
        self.query = JsonApiQuery::ignored();
        self
    }

    /// Register a related single resource for the top-level `included` array.
    pub fn include<
        T2: Serialize + Send + Sync + 'static,
        R2: JsonApiResource<T2> + Send + Sync + 'static,
    >(
        mut self,
        path: &str,
        child: JsonApiItem<T2, R2>,
    ) -> Self {
        let path = path.to_string();
        self.included
            .push((path, Box::new(move |query| child.resource_objects(query))));
        self
    }

    /// Register a related collection for the top-level `included` array.
    pub fn include_collection<
        T2: Serialize + Send + Sync + 'static,
        R2: JsonApiResource<T2> + Send + Sync + 'static,
    >(
        mut self,
        path: &str,
        child: JsonApiCollection<T2, R2>,
    ) -> Self {
        let path = path.to_string();
        self.included
            .push((path, Box::new(move |query| child.resource_objects(query))));
        self
    }

    /// Set the top-level `links` object (e.g. pagination links).
    pub fn with_links(mut self, links: Value) -> Self {
        self.links = links;
        self
    }

    /// Set additional top-level `meta` (merged with the automatic `count`).
    pub fn with_meta(mut self, meta: Value) -> Self {
        self.meta = meta;
        self
    }

    /// The resource objects for every model in the collection.
    pub fn resource_objects(&self, query: &JsonApiQuery) -> Vec<Value> {
        self.inner
            .iter()
            .map(|model| resource_object::<T, R>(model, query))
            .collect()
    }

    /// Render the full document: `{ "data": [...], "meta": { "count": N },
    /// "links": {...}, "included": [...] }`.
    pub fn to_array(&self) -> Value {
        let mut document = json!({
            "data": self.resource_objects(&self.query),
            "meta": {
                "count": self.inner.len(),
            },
        });
        merge_document_at(&mut document, &self.links, "links");
        merge_document_at(&mut document, &self.meta, "meta");
        let included = included_resource_objects(&self.included, &self.query);
        if !included.is_empty() {
            document["included"] = Value::Array(included);
        }
        document
    }

    /// Render the document as a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.to_array())
    }

    /// Consume the builder and return the inner models.
    pub fn into_inner(self) -> Vec<T> {
        self.inner
    }
}

fn merge_document(document: &mut Value, source: &Value) {
    if let Value::Object(ref mut target) = document {
        if let Value::Object(ref source_map) = source {
            for (key, value) in source_map {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn merge_document_at(document: &mut Value, source: &Value, key: &str) {
    if let Value::Object(ref mut target) = document {
        if let Value::Object(ref source_map) = source {
            let existing = target.get_mut(key).and_then(|value| value.as_object_mut());
            match existing {
                Some(existing) => {
                    for (k, v) in source_map {
                        existing.insert(k.clone(), v.clone());
                    }
                }
                None => {
                    target.insert(key.to_string(), source.clone());
                }
            }
        }
    }
}

fn into_response(document: Value) -> AxumResponse<Body> {
    AxumResponse::builder()
        .status(axum::http::StatusCode::OK)
        .header(CONTENT_TYPE, "application/vnd.api+json")
        .body(Body::from(
            serde_json::to_vec(&document).unwrap_or_default(),
        ))
        .unwrap_or_default()
}

impl<T: Serialize + Send + Sync + 'static, R: JsonApiResource<T>> axum::response::IntoResponse
    for JsonApiItem<T, R>
{
    fn into_response(self) -> AxumResponse<Body> {
        into_response(self.to_array())
    }
}

impl<T: Serialize + Send + Sync + 'static, R: JsonApiResource<T>> axum::response::IntoResponse
    for JsonApiCollection<T, R>
{
    fn into_response(self) -> AxumResponse<Body> {
        into_response(self.to_array())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Debug, Clone, Serialize)]
    struct Post {
        id: u64,
        title: String,
        body: String,
        author_id: u64,
        secret_note: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Author {
        id: u64,
        name: String,
        twitter_handle: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct Comment {
        id: u64,
        body: String,
    }

    struct PostResource;

    impl JsonApiResource<Post> for PostResource {
        fn attributes(model: &Post) -> Value {
            json!({
                "title": model.title,
                "body": model.body,
                "secret_note": model.secret_note,
            })
        }

        fn relationships(model: &Post, query: &JsonApiQuery) -> Value {
            json!({
                "author": when_included(query, "author", json!({
                    "data": { "type": "authors", "id": model.author_id.to_string() },
                })),
                "comments": when_included(query, "comments", json!({
                    "data": json!([]),
                })),
            })
        }

        fn links(model: &Post) -> Value {
            json!({ "self": format!("/api/posts/{}", model.id) })
        }
    }

    struct AuthorResource;

    impl JsonApiResource<Author> for AuthorResource {
        fn attributes(model: &Author) -> Value {
            json!({ "name": model.name, "twitter_handle": model.twitter_handle })
        }
    }

    struct CommentResource;

    impl JsonApiResource<Comment> for CommentResource {
        fn attributes(model: &Comment) -> Value {
            json!({ "body": model.body })
        }
    }

    fn post() -> Post {
        Post {
            id: 1,
            title: "Hello".into(),
            body: "World".into(),
            author_id: 7,
            secret_note: "hidden".into(),
        }
    }

    #[test]
    fn test_query_parse_include_and_fields() {
        let query =
            JsonApiQuery::parse("include=author,comments&fields[posts]=title&fields[authors]=name");
        assert!(query.includes("author"));
        assert!(query.includes("comments"));
        assert!(!query.includes("tags"));
        assert_eq!(query.fieldset("posts").unwrap().len(), 1);
        assert!(query.fieldset("posts").unwrap().contains("title"));
        assert!(query.fieldset("authors").unwrap().contains("name"));
        assert!(query.fieldset("users").is_none());
    }

    #[test]
    fn test_query_parse_nested_includes_imply_ancestors() {
        let query = JsonApiQuery::parse("include=author.posts");
        assert!(query.includes("author"));
        assert!(query.includes("author.posts"));
        assert!(!query.includes("comments"));
    }

    #[test]
    fn test_query_default_is_ignored() {
        let query = JsonApiQuery::ignored();
        assert!(!query.has_includes());
        assert!(query.fieldset("posts").is_none());
    }

    #[test]
    fn test_default_id_and_type() {
        let item = PostResource::make(post());
        let object = item.resource_object(&JsonApiQuery::default());
        assert_eq!(object["id"], json!("1"));
        assert_eq!(object["type"], json!("post"));
    }

    #[test]
    fn test_custom_id_and_type_overrides() {
        struct CustomResource;
        impl JsonApiResource<Author> for CustomResource {
            fn attributes(model: &Author) -> Value {
                json!({ "name": model.name })
            }
            fn id(_model: &Author) -> String {
                "custom-id".into()
            }
            fn type_() -> String {
                "people".into()
            }
        }
        let author = Author {
            id: 3,
            name: "Tim".into(),
            twitter_handle: "@tim".into(),
        };
        let object = CustomResource::make(author).resource_object(&JsonApiQuery::default());
        assert_eq!(object["id"], json!("custom-id"));
        assert_eq!(object["type"], json!("people"));
    }

    #[test]
    fn test_resource_object_shape() {
        let object = PostResource::make(post()).resource_object(&JsonApiQuery::default());
        assert_eq!(object["attributes"]["title"], json!("Hello"));
        assert_eq!(object["links"]["self"], json!("/api/posts/1"));
        assert!(object.get("relationships").is_none());
        assert!(object.get("meta").is_none());
    }

    #[test]
    fn test_sparse_fieldsets_filter_attributes_only() {
        let query = JsonApiQuery::parse("include=author&fields[post]=title&fields[author]=name");
        let object = PostResource::make(post()).resource_object(&query);
        assert_eq!(object["attributes"], json!({ "title": "Hello" }));
        let author = Author {
            id: 7,
            name: "Tim".into(),
            twitter_handle: "@tim".into(),
        };
        let author_object = AuthorResource::make(author).resource_object(&query);
        assert_eq!(author_object["attributes"], json!({ "name": "Tim" }));
    }

    #[test]
    fn test_relationships_require_include_request() {
        let query = JsonApiQuery::parse("include=author");
        let object = PostResource::make(post()).resource_object(&query);
        assert!(object["relationships"]["author"]["data"]["id"] == json!("7"));
        assert!(object["relationships"]["comments"]["data"].is_null());
    }

    #[test]
    fn test_compound_document_dedupes_included() {
        let query = JsonApiQuery::parse("include=author,comments");
        let author = Author {
            id: 7,
            name: "Tim".into(),
            twitter_handle: "@tim".into(),
        };
        let comments = vec![
            Comment {
                id: 1,
                body: "First".into(),
            },
            Comment {
                id: 2,
                body: "Second".into(),
            },
        ];
        let doc = PostResource::make(post())
            .with_query(&query)
            .include("author", AuthorResource::make(author.clone()))
            .include("author", AuthorResource::make(author))
            .include_collection("comments", CommentResource::collection(comments))
            .to_array();
        let included = doc["included"].as_array().unwrap();
        assert_eq!(included.len(), 3);
        assert!(included
            .iter()
            .any(|r| r["type"] == "author" && r["id"] == "7"));
        assert!(included
            .iter()
            .any(|r| r["type"] == "comment" && r["id"] == "1"));
        assert!(included
            .iter()
            .any(|r| r["type"] == "comment" && r["id"] == "2"));
        assert!(doc["data"]["relationships"]["author"]["data"]["id"] == json!("7"));
    }

    #[test]
    fn test_included_respects_sparse_fieldsets() {
        let query = JsonApiQuery::parse("include=author&fields[author]=name");
        let author = Author {
            id: 7,
            name: "Tim".into(),
            twitter_handle: "@tim".into(),
        };
        let doc = PostResource::make(post())
            .with_query(&query)
            .include("author", AuthorResource::make(author))
            .to_array();
        let included = doc["included"].as_array().unwrap();
        assert_eq!(included[0]["attributes"], json!({ "name": "Tim" }));
    }

    #[test]
    fn test_no_included_when_not_requested() {
        let query = JsonApiQuery::parse("fields[post]=title");
        let author = Author {
            id: 7,
            name: "Tim".into(),
            twitter_handle: "@tim".into(),
        };
        let doc = PostResource::make(post())
            .with_query(&query)
            .include("author", AuthorResource::make(author))
            .to_array();
        assert!(doc.get("included").is_none());
    }

    #[test]
    fn test_ignore_query_string() {
        let query = JsonApiQuery::parse("include=author&fields[post]=title");
        let author = Author {
            id: 7,
            name: "Tim".into(),
            twitter_handle: "@tim".into(),
        };
        let doc = PostResource::make(post())
            .with_query(&query)
            .ignore_query_string()
            .include("author", AuthorResource::make(author))
            .to_array();
        assert!(doc.get("included").is_none());
        assert_eq!(
            doc["data"]["attributes"],
            json!({ "title": "Hello", "body": "World", "secret_note": "hidden" })
        );
    }

    #[test]
    fn test_collection_shape_meta_and_links() {
        let posts = vec![post()];
        let doc = PostResource::collection(posts)
            .with_links(json!({ "self": "/api/posts" }))
            .with_meta(json!({ "total": 42 }))
            .to_array();
        assert_eq!(doc["data"].as_array().unwrap().len(), 1);
        assert_eq!(doc["meta"]["count"], json!(1));
        assert_eq!(doc["meta"]["total"], json!(42));
        assert_eq!(doc["links"]["self"], json!("/api/posts"));
    }

    #[test]
    fn test_collection_compound_document() {
        let query = JsonApiQuery::parse("include=author");
        let author = Author {
            id: 7,
            name: "Tim".into(),
            twitter_handle: "@tim".into(),
        };
        let doc = PostResource::collection(vec![post()])
            .with_query(&query)
            .include("author", AuthorResource::make(author))
            .to_array();
        assert_eq!(
            doc["data"][0]["relationships"]["author"]["data"]["id"],
            json!("7")
        );
        assert_eq!(doc["included"][0]["type"], json!("author"));
    }

    #[test]
    fn test_additional_merges_top_level() {
        let doc = PostResource::make(post())
            .additional(json!({ "meta": { "requested_at": "2026-01-01" } }))
            .to_array();
        assert_eq!(doc["meta"]["requested_at"], json!("2026-01-01"));
    }

    #[test]
    fn test_when_not_included_helper() {
        let query = JsonApiQuery::parse("include=author");
        assert!(when_not_included(&query, "comments", json!({})).is_some());
        assert!(when_not_included(&query, "author", json!({})).is_none());
        assert!(when_included(&query, "author", json!({})).is_some());
        assert!(when_included(&query, "comments", json!({})).is_none());
    }

    #[test]
    fn test_into_response_content_type() {
        use axum::response::IntoResponse;
        let response = PostResource::make(post()).into_response();
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/vnd.api+json"
        );
        let body_bytes = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(axum::body::to_bytes(response.into_body(), 64 * 1024))
            .unwrap();
        let parsed: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(parsed["data"]["type"], json!("post"));
    }

    #[test]
    fn test_into_inner() {
        let item = PostResource::make(post());
        assert_eq!(item.into_inner().id, 1);
        let collection = PostResource::collection(vec![post()]);
        assert_eq!(collection.into_inner().len(), 1);
    }

    // -----------------------------------------------------------------------
    // #[json_api_resource] macro tests
    // -----------------------------------------------------------------------

    use larastvel_macros::json_api_resource;

    struct PostApiResource;

    #[json_api_resource(Post)]
    impl PostApiResource {
        fn attributes(model: &Post) -> Value {
            json!({ "title": model.title })
        }

        fn id(model: &Post) -> String {
            format!("post-{}", model.id)
        }

        fn type_() -> String {
            "articles".into()
        }

        fn relationships(model: &Post, query: &JsonApiQuery) -> Value {
            json!({
                "author": when_included(query, "author", json!({
                    "data": { "type": "authors", "id": model.author_id.to_string() },
                })),
            })
        }
    }

    #[test]
    fn test_json_api_resource_macro() {
        let query = JsonApiQuery::parse("include=author");
        let object = PostApiResource::make(post()).resource_object(&query);
        assert_eq!(object["id"], json!("post-1"));
        assert_eq!(object["type"], json!("articles"));
        assert_eq!(object["attributes"], json!({ "title": "Hello" }));
        assert_eq!(object["relationships"]["author"]["data"]["id"], json!("7"));
    }
}
