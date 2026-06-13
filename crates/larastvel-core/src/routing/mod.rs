use std::future::Future;
use std::sync::{Arc, Mutex};

use axum::{
    handler::Handler,
    response::{Html, IntoResponse, Json},
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
        H: Handler<T, ()> + IntoRouteHandler,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = handler.name();
        let method_router = get(handler);
        self.add_method_route("GET", &uri, method_router, &name);
    }

    pub fn post<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()> + IntoRouteHandler,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = handler.name();
        let method_router = post(handler);
        self.add_method_route("POST", &uri, method_router, &name);
    }

    pub fn put<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()> + IntoRouteHandler,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = handler.name();
        let method_router = put(handler);
        self.add_method_route("PUT", &uri, method_router, &name);
    }

    pub fn patch<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()> + IntoRouteHandler,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = handler.name();
        let method_router = patch(handler);
        self.add_method_route("PATCH", &uri, method_router, &name);
    }

    pub fn delete<H, T>(&self, uri: &str, handler: H)
    where
        H: Handler<T, ()> + IntoRouteHandler,
        T: 'static,
    {
        let uri = self.resolve_uri(uri);
        let name = handler.name();
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

pub trait IntoRouteHandler {
    fn name(&self) -> String;
}

impl<F, Fut, T> IntoRouteHandler for F
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static,
    T: IntoResponse + 'static,
{
    fn name(&self) -> String {
        std::any::type_name::<F>().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
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
    fn test_into_route_handler_for_closure() {
        let handler = || async { "ok" };
        let name = handler.name();
        assert!(!name.is_empty());
    }
}
