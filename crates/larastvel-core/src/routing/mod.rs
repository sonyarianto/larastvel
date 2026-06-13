use std::future::Future;
use std::sync::{Arc, Mutex};

use axum::{
    response::{Html, IntoResponse, Json},
    routing::get,
    Router as AxumRouter,
};

use crate::foundation::Application;

#[derive(Clone)]
pub struct Registrar {
    _app: Application,
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
    pub fn new(app: Application) -> Self {
        Self {
            _app: app,
            routes: Arc::new(Mutex::new(vec![])),
            group_prefix: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get(&self, uri: &str, handler: impl IntoRouteHandler) {
        self.add_route("GET", uri, handler);
    }

    pub fn post(&self, uri: &str, handler: impl IntoRouteHandler) {
        self.add_route("POST", uri, handler);
    }

    pub fn put(&self, uri: &str, handler: impl IntoRouteHandler) {
        self.add_route("PUT", uri, handler);
    }

    pub fn patch(&self, uri: &str, handler: impl IntoRouteHandler) {
        self.add_route("PATCH", uri, handler);
    }

    pub fn delete(&self, uri: &str, handler: impl IntoRouteHandler) {
        self.add_route("DELETE", uri, handler);
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

    fn add_route(&self, method: &str, uri: &str, _handler: impl IntoRouteHandler) {
        let prefix = self.group_prefix.lock().unwrap().clone();
        let full_uri = match &prefix {
            Some(p) => format!("/{}{}", p.trim_start_matches('/'), uri),
            None => uri.to_string(),
        };

        let def = RouteDefinition {
            method: method.to_string(),
            uri: full_uri,
            handler_name: _handler.name(),
            middleware: vec![],
        };

        self.routes.lock().unwrap().push(def);
    }

    pub fn register_routes(&self, _app: Application) {
        // Routes are registered by the kernel
    }

    pub fn build(&self) -> AxumRouter {
        AxumRouter::new()
            .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
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
