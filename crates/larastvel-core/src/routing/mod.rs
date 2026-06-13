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
