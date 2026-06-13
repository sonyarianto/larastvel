use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router as AxumRouter;
use tracing::info;

use crate::config::Config;
use crate::console::ConsoleKernel;
use crate::database::DatabaseManager;
use crate::routing::{Registrar, RouteDefinition};

pub trait ServiceProvider: Send + Sync {
    fn register(&self, app: &Application);
    fn boot(&self, _app: &Application) {}
    fn provides(&self) -> Vec<&'static str> {
        vec![]
    }
}

#[derive(Clone)]
pub struct Application {
    inner: Arc<Mutex<AppInner>>,
}

struct AppInner {
    instances: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    aliases: HashMap<String, TypeId>,
    config: Config,
    db: Option<DatabaseManager>,
    base_path: PathBuf,
    booted: bool,
    router: Arc<Mutex<AxumRouter>>,
    routes: Arc<Mutex<Vec<RouteDefinition>>>,
}

impl Application {
    pub fn new(base_path: Option<PathBuf>) -> Self {
        let path = base_path.unwrap_or_else(|| PathBuf::from("."));
        let config = Config::load(&path);

        let inner = Arc::new(Mutex::new(AppInner {
            instances: HashMap::new(),
            aliases: HashMap::new(),
            config,
            db: None,
            base_path: path,
            booted: false,
            router: Arc::new(Mutex::new(AxumRouter::new())),
            routes: Arc::new(Mutex::new(vec![])),
        }));

        Self { inner }
    }

    pub fn base_path(&self) -> PathBuf {
        self.inner.lock().unwrap().base_path.clone()
    }

    pub fn config(&self) -> Config {
        self.inner.lock().unwrap().config.clone()
    }

    pub fn boot(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.booted {
            return;
        }
        inner.booted = true;
        info!("Application booted");
    }

    pub fn bind<T: Any + Send + Sync>(&self, instance: T) {
        let id = TypeId::of::<T>();
        self.inner
            .lock()
            .unwrap()
            .instances
            .insert(id, Box::new(instance));
    }

    pub fn singleton<T: Any + Send + Sync + Clone>(&self, instance: T) {
        self.bind(instance);
    }

    pub fn make<T: Any + Send + Sync + Clone>(&self) -> Option<T> {
        let inner = self.inner.lock().unwrap();
        let id = TypeId::of::<T>();
        inner
            .instances
            .get(&id)
            .and_then(|b| b.downcast_ref::<T>())
            .cloned()
    }

    pub fn alias(&self, alias: &str, id: TypeId) {
        self.inner
            .lock()
            .unwrap()
            .aliases
            .insert(alias.to_string(), id);
    }

    pub fn database(&self) -> Option<DatabaseManager> {
        self.inner.lock().unwrap().db.clone()
    }

    pub fn with_database(self, db: DatabaseManager) -> Self {
        self.inner.lock().unwrap().db = Some(db);
        self
    }

    pub fn console_kernel(&self) -> ConsoleKernel {
        ConsoleKernel::new(self.clone())
    }

    pub fn router(&self) -> Registrar {
        let inner = self.inner.lock().unwrap();
        Registrar::new(inner.router.clone(), inner.routes.clone())
    }

    pub async fn run(self) {
        self.boot();

        let router = {
            let inner = self.inner.lock().unwrap();
            let registrar = Registrar::new(inner.router.clone(), inner.routes.clone());
            registrar.build()
        };

        let addr = self
            .config()
            .get("app.url")
            .unwrap_or_else(|| "0.0.0.0:8080".to_string())
            .replace("http://", "")
            .trim_end_matches('/')
            .to_string();
        let addr = if addr.contains(':') {
            addr
        } else {
            format!("{}:8080", addr)
        };

        info!("Larastvel server starting on {}", addr);
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, router).await.unwrap();
    }
}

pub trait Kernel: Send + Sync {
    fn register_providers(&self, app: &Application);
    fn register_routes(&self, app: &Application);
    fn boot(&self, app: &Application);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_application_new_uses_default_config() {
        let app = Application::new(None);
        assert_eq!(app.config().app.name, "larastvel");
    }

    #[test]
    fn test_application_base_path_defaults_to_dot() {
        let app = Application::new(None);
        assert_eq!(app.base_path(), std::path::PathBuf::from("."));
    }

    #[test]
    fn test_application_base_path_custom() {
        let app = Application::new(Some(std::path::PathBuf::from("/tmp")));
        assert_eq!(app.base_path(), std::path::PathBuf::from("/tmp"));
    }

    #[test]
    fn test_application_bind_and_make() {
        let app = Application::new(None);
        app.bind(42i32);
        let val: Option<i32> = app.make();
        assert_eq!(val, Some(42));
    }

    #[test]
    fn test_application_make_nonexistent() {
        let app = Application::new(None);
        let val: Option<String> = app.make();
        assert!(val.is_none());
    }

    #[test]
    fn test_application_singleton() {
        let app = Application::new(None);
        app.singleton("shared".to_string());
        let val: Option<String> = app.make();
        assert_eq!(val, Some("shared".to_string()));
    }

    #[test]
    fn test_application_boot_twice_is_idempotent() {
        let app = Application::new(None);
        app.boot();
        app.boot();
    }

    #[test]
    fn test_application_database_is_none_by_default() {
        let app = Application::new(None);
        assert!(app.database().is_none());
    }

    #[test]
    fn test_application_router_returns_registrar() {
        let app = Application::new(None);
        let registrar = app.router();
        registrar.get("/ping", || async { "pong" });
        let routes = registrar.list_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].uri, "/ping");
    }

    #[test]
    fn test_service_provider_trait() {
        struct TestProvider;
        impl ServiceProvider for TestProvider {
            fn register(&self, _app: &Application) {}
        }
        let provider = TestProvider;
        assert!(provider.provides().is_empty());
    }
}
