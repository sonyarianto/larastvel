use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    handler::Handler,
    response::{Html, IntoResponse, Json, Response},
    routing::{any, delete, get, patch, post, put, MethodRouter},
    Router as AxumRouter,
};

type MiddlewareFactory = Arc<dyn Fn(MethodRouter) -> MethodRouter + Send + Sync>;

#[derive(Clone)]
pub struct Registrar {
    router: Arc<Mutex<AxumRouter>>,
    routes: Arc<Mutex<Vec<RouteDefinition>>>,
    group_prefix: Arc<Mutex<Option<String>>>,
    middleware_aliases: Arc<Mutex<HashMap<String, MiddlewareFactory>>>,
    current_middleware: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RouteDefinition {
    pub method: String,
    pub uri: String,
    pub handler_name: String,
    pub middleware: Vec<String>,
}

impl Registrar {
    pub(crate) fn new(
        router: Arc<Mutex<AxumRouter>>,
        routes: Arc<Mutex<Vec<RouteDefinition>>>,
    ) -> Self {
        Self {
            router,
            routes,
            group_prefix: Arc::new(Mutex::new(None)),
            middleware_aliases: Arc::new(Mutex::new(HashMap::new())),
            current_middleware: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a named middleware alias.
    ///
    /// The closure receives a `MethodRouter` and must return a `MethodRouter`
    /// with the middleware applied via `.layer()` or other transformations.
    ///
    /// ```rust,ignore
    /// registrar.middleware("session", |r| r.layer(SessionLayer::new(config)));
    /// registrar.middleware("csrf", |r| r.layer(CsrfLayer::new()));
    /// ```
    pub fn middleware(
        &self,
        name: &str,
        f: impl Fn(MethodRouter) -> MethodRouter + Send + Sync + 'static,
    ) {
        self.middleware_aliases
            .lock()
            .unwrap()
            .insert(name.to_string(), Arc::new(f));
    }

    /// Set middleware to apply to all routes registered after this call.
    ///
    /// Each name must have been previously registered via
    /// [`middleware`](Self::middleware).
    ///
    /// ```rust,ignore
    /// registrar.with_middleware(vec!["session", "csrf"]);
    /// ```
    pub fn with_middleware(&self, names: Vec<&str>) {
        *self.current_middleware.lock().unwrap() =
            names.into_iter().map(|n| n.to_string()).collect();
    }

    pub fn get<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = std::any::type_name::<H>().to_string();
        let method_router = get(handler);
        self.add_method_route("GET", &uri, method_router, &name);
    }

    pub fn post<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = std::any::type_name::<H>().to_string();
        let method_router = post(handler);
        self.add_method_route("POST", &uri, method_router, &name);
    }

    pub fn put<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = std::any::type_name::<H>().to_string();
        let method_router = put(handler);
        self.add_method_route("PUT", &uri, method_router, &name);
    }

    pub fn patch<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = std::any::type_name::<H>().to_string();
        let method_router = patch(handler);
        self.add_method_route("PATCH", &uri, method_router, &name);
    }

    pub fn delete<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = std::any::type_name::<H>().to_string();
        let method_router = delete(handler);
        self.add_method_route("DELETE", &uri, method_router, &name);
    }

    /// Register a WebSocket upgrade handler.
    ///
    /// The handler should accept `WebSocketUpgrade` as its first argument
    /// and extract shared state via `Extension<T>` (not `State<T>`).
    ///
    /// ```ignore
    /// use larastvel_core::broadcasting::ws_handler;
    /// use larastvel_core::axum::Extension;
    ///
    /// router.ws("/ws", ws_handler);
    /// ```
    ///
    /// The `SubscriberRegistry` must be provided as an `Extension` layer on
    /// the final router:
    ///
    /// ```ignore
    /// router.layer(Extension(registry));
    /// ```
    pub fn ws<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = std::any::type_name::<H>().to_string();
        let method_router = any(handler);
        self.add_method_route("WS", &uri, method_router, &name);
    }

    pub fn view(&self, uri: &str, template: &str) {
        let t = template.to_string();
        self.get(uri, move || {
            let t = t.clone();
            async move { Html(t) }
        });
    }

    pub fn group(&self, prefix: &str, f: impl FnOnce(&Registrar)) {
        let prev_prefix = self.group_prefix.lock().unwrap().take();
        let prev_middleware = self.current_middleware.lock().unwrap().clone();
        *self.group_prefix.lock().unwrap() = Some(prefix.to_string());
        f(self);
        *self.group_prefix.lock().unwrap() = prev_prefix;
        *self.current_middleware.lock().unwrap() = prev_middleware;
    }

    fn resolve_uri(&self, uri: &str) -> String {
        let prefix = self.group_prefix.lock().unwrap().clone();
        match &prefix {
            Some(p) => format!("/{}{}", p.trim_start_matches('/'), uri),
            None => {
                if uri.starts_with('/') {
                    uri.to_string()
                } else {
                    format!("/{}", uri)
                }
            }
        }
    }

    fn add_method_route(
        &self,
        method: &str,
        uri: &str,
        method_router: MethodRouter,
        handler_name: &str,
    ) {
        let current = self.current_middleware.lock().unwrap().clone();
        let middleware_names = current.clone();
        let aliases = self.middleware_aliases.lock().unwrap();

        let method_router = current
            .iter()
            .fold(method_router, |r, name| match aliases.get(name) {
                Some(f) => f(r),
                None => r,
            });

        drop(aliases);
        drop(current);

        {
            let mut router = self.router.lock().unwrap();
            *router = std::mem::take(&mut *router).route(uri, method_router);
        }
        self.routes.lock().unwrap().push(RouteDefinition {
            method: method.to_string(),
            uri: uri.to_string(),
            handler_name: handler_name.to_string(),
            middleware: middleware_names,
        });
    }

    pub fn build(&self) -> AxumRouter {
        let mut router = self.router.lock().unwrap();
        let r = std::mem::take(&mut *router);
        r.route(
            "/health",
            get(|| async { Json(serde_json::json!({"status": "ok"})) }),
        )
    }

    pub fn list_routes(&self) -> Vec<RouteDefinition> {
        self.routes.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
pub trait ResourceController: Send + Sync + 'static {
    const RESOURCE_NAME: &'static str;

    async fn index() -> Response {
        Json(serde_json::json!({"data": []})).into_response()
    }

    async fn create() -> Response {
        Json(serde_json::json!({"data": {}})).into_response()
    }

    async fn store() -> Response {
        Json(serde_json::json!({"data": {}})).into_response()
    }

    async fn show(id: String) -> Response {
        Json(serde_json::json!({"data": {"id": id}})).into_response()
    }

    async fn edit(id: String) -> Response {
        Json(serde_json::json!({"data": {"id": id}})).into_response()
    }

    async fn update(id: String) -> Response {
        Json(serde_json::json!({"data": {"id": id}})).into_response()
    }

    async fn destroy(id: String) -> Response {
        Json(serde_json::json!({"data": {"id": id}})).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{controller, route, Resource};
    #[allow(unused_imports)]
    use crate::{delete, get, patch, post, put, ws};
    use axum::body::Body;
    use axum::http::Request;
    use axum::response::Response;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    #[test]
    fn test_registrar_new() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);
        assert!(registrar.list_routes().is_empty());
    }

    #[test]
    fn test_route_resolution_without_prefix() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);
        assert_eq!(registrar.resolve_uri("/foo"), "/foo");
        assert_eq!(registrar.resolve_uri("foo"), "/foo");
    }

    #[test]
    fn test_register_and_build_route() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.get("/test", || async { "hello" });

        let app = registrar.build();

        let response = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/test")
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();

        assert_eq!(response.status(), 200);
    }

    #[test]
    fn test_register_and_build_post_route() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.post("/data", || async { "created" });

        let app = registrar.build();

        let response = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/data")
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();

        assert_eq!(response.status(), 200);
    }

    #[test]
    fn test_health_route_in_build() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        let app = registrar.build();

        let response = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/health")
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();

        assert_eq!(response.status(), 200);
    }

    #[test]
    fn test_list_routes_metadata() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.get("/a", || async { "a" });
        registrar.post("/b", || async { "b" });

        let listed = registrar.list_routes();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].method, "GET");
        assert_eq!(listed[0].uri, "/a");
        assert_eq!(listed[1].method, "POST");
        assert_eq!(listed[1].uri, "/b");
    }

    #[test]
    fn test_group_prefix_affects_uri() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.group("/admin", |r| {
            r.get("/users", || async { "users" });
        });

        let listed = registrar.list_routes();
        assert_eq!(listed[0].uri, "/admin/users");
    }

    #[test]
    fn test_view_shorthand() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.view("/welcome", "welcome");

        let app = registrar.build();

        let response = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/welcome")
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();

        assert_eq!(response.status(), 200);
    }

    #[test]
    fn test_middleware_register_and_apply() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.middleware("add-header", |r| {
            r.layer(axum::middleware::from_fn(
                |req: axum::http::Request<axum::body::Body>,
                 next: axum::middleware::Next| async move {
                    let mut resp = next.run(req).await;
                    resp.headers_mut()
                        .insert("X-Test", "applied".parse().unwrap());
                    resp
                },
            ))
        });

        registrar.with_middleware(vec!["add-header"]);
        registrar.get("/guarded", || async { "ok" });

        let app = registrar.build();
        let resp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.oneshot(
                    axum::http::Request::builder()
                        .method("GET")
                        .uri("/guarded")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();

        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers().get("X-Test").map(|v| v.to_str().unwrap()),
            Some("applied")
        );
    }

    #[test]
    fn test_middleware_unknown_name_is_noop() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.with_middleware(vec!["does-not-exist"]);
        registrar.get("/noop", || async { "ok" });

        let app = registrar.build();
        let resp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.oneshot(
                    axum::http::Request::builder()
                        .method("GET")
                        .uri("/noop")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();

        assert_eq!(resp.status(), 200);
    }

    #[test]
    fn test_group_saves_and_restores_middleware() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.middleware("add-header", |r| {
            r.layer(axum::middleware::from_fn(
                |req: axum::http::Request<axum::body::Body>,
                 next: axum::middleware::Next| async move {
                    let mut resp = next.run(req).await;
                    resp.headers_mut()
                        .insert("X-Test", "applied".parse().unwrap());
                    resp
                },
            ))
        });

        // Outside group — no middleware
        registrar.get("/public", || async { "open" });

        // Inside group — middleware applied
        registrar.group("/admin", |r| {
            r.with_middleware(vec!["add-header"]);
            r.get("/dashboard", || async { "admin" });
        });

        // After group — middleware restored to none
        registrar.get("/also-public", || async { "open" });

        let app = registrar.build();

        // Public routes should NOT have the header
        let resp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.clone().oneshot(
                    axum::http::Request::builder()
                        .method("GET")
                        .uri("/public")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp.headers().get("X-Test").is_none());

        // Admin route should have the header
        let resp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.clone().oneshot(
                    axum::http::Request::builder()
                        .method("GET")
                        .uri("/admin/dashboard")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers().get("X-Test").map(|v| v.to_str().unwrap()),
            Some("applied")
        );

        // After-group public should NOT have the header
        let resp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                app.clone().oneshot(
                    axum::http::Request::builder()
                        .method("GET")
                        .uri("/also-public")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                ),
            )
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp.headers().get("X-Test").is_none());
    }

    #[test]
    fn test_middleware_recorded_in_route_definition() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.middleware("m1", |r| r);
        registrar.with_middleware(vec!["m1"]);
        registrar.get("/mw", || async { "ok" });

        let listed = registrar.list_routes();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].middleware, vec!["m1"]);
    }

    // --- ResourceController trait tests ---

    struct TestResource;

    #[async_trait::async_trait]
    impl ResourceController for TestResource {
        const RESOURCE_NAME: &'static str = "tests";
    }

    #[tokio::test]
    async fn test_resource_controller_index_default() {
        let resp = TestResource::index().await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_resource_controller_show_default() {
        let resp = TestResource::show("42".to_string()).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_resource_controller_store_default() {
        let resp = TestResource::store().await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_resource_controller_update_default() {
        let resp = TestResource::update("1".to_string()).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_resource_controller_destroy_default() {
        let resp = TestResource::destroy("1".to_string()).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_resource_controller_create_default() {
        let resp = TestResource::create().await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_resource_controller_edit_default() {
        let resp = TestResource::edit("1".to_string()).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_resource_controller_custom_resource_name() {
        struct CustomResource;

        #[async_trait::async_trait]
        impl ResourceController for CustomResource {
            const RESOURCE_NAME: &'static str = "custom-items";
        }

        assert_eq!(CustomResource::RESOURCE_NAME, "custom-items");
    }

    #[tokio::test]
    async fn test_resource_controller_custom_index() {
        struct CustomIndex;

        #[async_trait::async_trait]
        impl ResourceController for CustomIndex {
            const RESOURCE_NAME: &'static str = "custom";

            async fn index() -> Response {
                Json(serde_json::json!({"custom": true})).into_response()
            }
        }

        let resp = CustomIndex::index().await;
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["custom"], true);
    }

    #[derive(Resource)]
    struct TestDerivedResource;

    #[tokio::test]
    async fn test_derive_resource_generates_trait_impl() {
        assert_eq!(
            <TestDerivedResource as ResourceController>::RESOURCE_NAME,
            "testderivedresource"
        );
        let resp = TestDerivedResource::index().await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_derive_resource_generates_register_routes() {
        // register_routes should compile and not panic
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);
        TestDerivedResource::register_routes(&registrar);
        let listed = registrar.list_routes();
        assert_eq!(listed.len(), 7);
    }

    #[tokio::test]
    async fn test_derive_resource_routes_respond() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        TestDerivedResource::register_routes(&registrar);

        let app = registrar.build();

        // Test index
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/testderivedresource")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test create
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/testderivedresource/create")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test store
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/testderivedresource")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test show
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/testderivedresource/42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test edit
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/testderivedresource/42/edit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test update
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/testderivedresource/42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test destroy
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/testderivedresource/42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    struct UserController;

    #[async_trait::async_trait]
    impl ResourceController for UserController {
        const RESOURCE_NAME: &'static str = "users";

        async fn index() -> Response {
            Json(serde_json::json!({"users": ["alice", "bob"]})).into_response()
        }

        async fn show(id: String) -> Response {
            Json(serde_json::json!({"user": {"id": id}})).into_response()
        }
    }

    impl UserController {
        fn register_routes(registrar: &Registrar) {
            let name = <Self as ResourceController>::RESOURCE_NAME;

            registrar.get(&format!("/{}", name), Self::__users_index);
            registrar.get(&format!("/{}/{{id}}", name), Self::__users_show);
            registrar.post(&format!("/{}", name), Self::__users_store);
            registrar.put(&format!("/{}/{{id}}", name), Self::__users_update);
            registrar.delete(&format!("/{}/{{id}}", name), Self::__users_destroy);
        }

        async fn __users_index() -> Response {
            <Self as ResourceController>::index().await
        }

        async fn __users_show(axum::extract::Path(id): axum::extract::Path<String>) -> Response {
            <Self as ResourceController>::show(id).await
        }

        async fn __users_store() -> Response {
            <Self as ResourceController>::store().await
        }

        async fn __users_update(axum::extract::Path(id): axum::extract::Path<String>) -> Response {
            <Self as ResourceController>::update(id).await
        }

        async fn __users_destroy(axum::extract::Path(id): axum::extract::Path<String>) -> Response {
            <Self as ResourceController>::destroy(id).await
        }
    }

    #[tokio::test]
    async fn test_manual_resource_controller() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        UserController::register_routes(&registrar);

        let app = registrar.build();

        // Test custom index
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/users")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // Test custom show with path param
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/users/42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[controller]
    struct MyController;

    #[test]
    fn test_controller_attribute_compiles() {
        // Just test that the attribute compiles and generates register_routes
        assert_eq!(std::mem::size_of::<MyController>(), 0);
    }

    #[test]
    fn test_controller_generates_register_routes() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);
        MyController::register_routes(&registrar);
        // No routes registered (it's empty by default from the macro)
        assert!(registrar.list_routes().is_empty());
    }

    // --- #[route] attribute macro tests ---

    struct RoutedController;

    #[route]
    impl RoutedController {
        #[get("/items")]
        async fn index() -> &'static str {
            "index"
        }

        #[post("/items")]
        async fn store() -> &'static str {
            "created"
        }

        #[put("/items/{id}")]
        async fn update() -> &'static str {
            "updated"
        }

        #[delete("/items/{id}")]
        async fn destroy() -> &'static str {
            "deleted"
        }

        // A method without a route attribute — should be ignored
        #[allow(dead_code)]
        async fn internal() -> &'static str {
            "internal"
        }
    }

    #[tokio::test]
    async fn test_route_macro_registers_routes() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);
        RoutedController::register_routes(&registrar);
        let listed = registrar.list_routes();
        assert_eq!(listed.len(), 4);

        let methods: Vec<_> = listed.iter().map(|r| r.method.as_str()).collect();
        assert!(methods.contains(&"GET"));
        assert!(methods.contains(&"POST"));
        assert!(methods.contains(&"PUT"));
        assert!(methods.contains(&"DELETE"));

        let uris: Vec<_> = listed.iter().map(|r| r.uri.as_str()).collect();
        assert!(uris.contains(&"/items"));
        assert!(uris.contains(&"/items/{id}"));
    }

    #[tokio::test]
    async fn test_route_macro_http_methods() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);
        RoutedController::register_routes(&registrar);
        let app = registrar.build();

        // GET /items
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/items")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // POST /items
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/items")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // PUT /items/42
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/items/42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // DELETE /items/42
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/items/42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_route_macro_skips_unmarked_methods() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);
        RoutedController::register_routes(&registrar);
        let listed = registrar.list_routes();
        assert!(!listed.iter().any(|r| r.handler_name.contains("internal")));
    }

    // --- #[can] attribute macro tests ---

    use crate::can;
    use axum::Extension;

    /// A handler with #[can] that returns &'static str.
    #[can("admin")]
    async fn protected_handler() -> &'static str {
        "secret"
    }

    /// #[can] compiles and the function has the expected name.
    #[test]
    fn test_can_attribute_compiles() {
        let _name = stringify!(protected_handler);
    }

    /// Verify #[can] handler works through the registrar when auth passes.
    #[tokio::test]
    async fn test_can_handler_registers_and_routes() {
        use crate::auth::Gate;
        use crate::GateCheck;

        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);

        registrar.get("/protected", protected_handler);

        let gate = Gate::new();
        gate.define("admin", |_, _| GateCheck::Allowed);
        let app = registrar.build().layer(Extension(gate));

        // Without auth token — should fail authentication (401/403)
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        assert!(
            status == 401 || status == 403,
            "expected 401 or 403, got {status}",
        );
    }

    /// Test #[can] used inside a #[route] impl block.
    struct AdminController;

    #[route]
    impl AdminController {
        #[get("/admin")]
        #[can("admin")]
        async fn admin_only() -> &'static str {
            "admin data"
        }
    }

    #[tokio::test]
    async fn test_can_within_route_macro() {
        let router = Arc::new(Mutex::new(AxumRouter::new()));
        let routes = Arc::new(Mutex::new(vec![]));
        let registrar = Registrar::new(router, routes);
        AdminController::register_routes(&registrar);

        let listed = registrar.list_routes();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].uri, "/admin");
    }
}
