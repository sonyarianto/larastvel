use std::sync::{Arc, Mutex};

use axum::{
    handler::Handler,
    response::{Html, IntoResponse, Json, Response},
    routing::{delete, get, patch, post, put, MethodRouter},
    Router as AxumRouter,
};

#[derive(Clone)]
pub struct Registrar {
    router: Arc<Mutex<AxumRouter>>,
    routes: Arc<Mutex<Vec<RouteDefinition>>>,
    group_prefix: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Debug)]
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
        }
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

    pub fn view(&self, uri: &str, template: &str) {
        let t = template.to_string();
        self.get(uri, move || {
            let t = t.clone();
            async move { Html(t) }
        });
    }

    pub fn group(&self, prefix: &str, f: impl FnOnce(&Registrar)) {
        let prev_prefix = self.group_prefix.lock().unwrap().take();
        *self.group_prefix.lock().unwrap() = Some(prefix.to_string());
        f(self);
        *self.group_prefix.lock().unwrap() = prev_prefix;
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
        {
            let mut router = self.router.lock().unwrap();
            *router = std::mem::take(&mut *router).route(uri, method_router);
        }
        self.routes.lock().unwrap().push(RouteDefinition {
            method: method.to_string(),
            uri: uri.to_string(),
            handler_name: handler_name.to_string(),
            middleware: vec![],
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
    use crate::{controller, Resource};
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
}
