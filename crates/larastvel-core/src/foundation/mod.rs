pub mod event_service_provider;
pub mod route_service_provider;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router as AxumRouter;
use tracing::info;

use crate::config::Config;
use crate::console::ConsoleKernel;
use crate::database::DatabaseManager;
use crate::encryption::Encrypter;
use crate::routing::{Registrar, RouteDefinition};
use crate::session::csrf::CsrfLayer;
use crate::session::middleware::{SessionConfig, SessionLayer};

pub use event_service_provider::EventServiceProvider;
pub use route_service_provider::RouteServiceProvider;

pub trait ServiceProvider: Send + Sync {
    fn register(&self, app: &Application);
    fn boot(&self, _app: &Application) {}
    fn provides(&self) -> Vec<&'static str> {
        vec![]
    }
}

/// Marker trait for providers that should be lazily registered and booted.
///
/// Implement this trait on your provider to defer `register()` and `boot()`
/// until one of the services listed in `provides()` is first resolved via
/// [`Application::make`] or [`Application::make_by_alias`].
///
/// This matches Laravel's deferred provider pattern.
pub trait DeferrableProvider: ServiceProvider {}

#[derive(Clone)]
pub struct Application {
    inner: Arc<Mutex<AppInner>>,
}

struct AppInner {
    instances: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    aliases: HashMap<String, TypeId>,
    config: Config,
    console: Option<ConsoleKernel>,
    db: Option<DatabaseManager>,
    base_path: PathBuf,
    booted: bool,
    providers: Vec<Arc<dyn ServiceProvider>>,
    deferred_providers: Vec<Arc<dyn ServiceProvider>>,
    deferred_providers_by_name: HashMap<String, Arc<dyn ServiceProvider>>,
    router: Arc<Mutex<AxumRouter>>,
    routes: Arc<Mutex<Vec<RouteDefinition>>>,
    layers: Vec<Box<dyn FnOnce(AxumRouter) -> AxumRouter + Send>>,
}

impl Application {
    pub fn new(base_path: Option<PathBuf>) -> Self {
        let path = base_path.unwrap_or_else(|| PathBuf::from("."));
        let config = Config::load(&path);

        let inner = Arc::new(Mutex::new(AppInner {
            instances: HashMap::new(),
            aliases: HashMap::new(),
            config,
            console: None,
            db: None,
            base_path: path,
            booted: false,
            providers: Vec::new(),
            deferred_providers: Vec::new(),
            deferred_providers_by_name: HashMap::new(),
            router: Arc::new(Mutex::new(AxumRouter::new())),
            routes: Arc::new(Mutex::new(vec![])),
            layers: Vec::new(),
        }));

        Self { inner }
    }

    pub fn base_path(&self) -> PathBuf {
        self.inner.lock().unwrap().base_path.clone()
    }

    pub fn config(&self) -> Config {
        self.inner.lock().unwrap().config.clone()
    }

    /// Register a provider that will be eagerly registered and booted.
    pub fn register_provider(&self, provider: Arc<dyn ServiceProvider>) {
        provider.register(self);
        let mut inner = self.inner.lock().unwrap();
        if inner.booted {
            provider.boot(self);
        }
        inner.providers.push(provider);
    }

    /// Register a provider whose `register()` and `boot()` are deferred
    /// until one of its provided services is first resolved via
    /// [`make`](Self::make) or [`make_by_alias`](Self::make_by_alias).
    ///
    /// The provider **must** return a non-empty list from `provides()`.
    /// If `provides()` is empty the provider will never be activated.
    pub fn register_deferred_provider(&self, provider: Arc<dyn ServiceProvider>) {
        let mut inner = self.inner.lock().unwrap();
        for name in provider.provides() {
            inner
                .deferred_providers_by_name
                .insert(name.to_string(), provider.clone());
        }
        inner.deferred_providers.push(provider);
    }

    /// Boot all eager providers (idempotent).
    pub fn boot(&self) {
        let providers_to_boot = {
            let mut inner = self.inner.lock().unwrap();
            if inner.booted {
                return;
            }
            inner.booted = true;
            inner.providers.clone()
        };

        info!("Application booted");

        for provider in &providers_to_boot {
            provider.boot(self);
        }
    }

    /// Bind an instance into the container by its type.
    pub fn bind<T: Any + Send + Sync>(&self, instance: T) {
        let id = TypeId::of::<T>();
        self.inner
            .lock()
            .unwrap()
            .instances
            .insert(id, Box::new(instance));
    }

    /// Bind a shared (singleton) instance into the container.
    pub fn singleton<T: Any + Send + Sync + Clone>(&self, instance: T) {
        self.bind(instance);
    }

    /// Resolve a previously bound instance by type.
    ///
    /// If the type matches a service provided by a deferred provider, that
    /// provider will be registered and booted before the instance is returned.
    pub fn make<T: Any + Send + Sync + Clone>(&self) -> Option<T> {
        self.boot_deferred_for::<T>();
        let inner = self.inner.lock().unwrap();
        let id = TypeId::of::<T>();
        inner
            .instances
            .get(&id)
            .and_then(|b| b.downcast_ref::<T>())
            .cloned()
    }

    /// Resolve a previously bound instance by string alias.
    ///
    /// If the alias matches a service provided by a deferred provider, that
    /// provider will be registered and booted before the instance is returned.
    pub fn make_by_alias<T: Any + Send + Sync + Clone>(&self, alias: &str) -> Option<T> {
        self.boot_deferred_for_name(alias);
        let inner = self.inner.lock().unwrap();
        let id = inner.aliases.get(alias)?;
        inner
            .instances
            .get(id)
            .and_then(|b| b.downcast_ref::<T>())
            .cloned()
    }

    /// Activate deferred providers whose provided services match `T`'s alias.
    fn boot_deferred_for<T: 'static>(&self) {
        let tid = TypeId::of::<T>();
        let names: Vec<String> = {
            let inner = self.inner.lock().unwrap();
            inner
                .aliases
                .iter()
                .filter(|(_, id)| **id == tid)
                .map(|(name, _)| name.clone())
                .collect()
        };

        for name in &names {
            self.activate_deferred_provider(name);
        }
    }

    /// Activate any deferred provider that provides a service matching `name`.
    fn boot_deferred_for_name(&self, name: &str) {
        self.activate_deferred_provider(name);
    }

    /// Internal: register & boot the deferred provider that provides `name`,
    /// then remove it from the deferred lists.
    fn activate_deferred_provider(&self, name: &str) {
        let provider = {
            let mut inner = self.inner.lock().unwrap();
            inner.deferred_providers_by_name.remove(name)
        };

        if let Some(p) = provider {
            p.register(self);
            p.boot(self);
            // Remove from deferred_providers vec too
            let mut inner = self.inner.lock().unwrap();
            inner.deferred_providers.retain(|d| !Arc::ptr_eq(d, &p));
        }
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
        let mut inner = self.inner.lock().unwrap();
        inner
            .console
            .get_or_insert_with(|| ConsoleKernel::new(self.clone()))
            .clone()
    }

    pub fn router(&self) -> Registrar {
        let inner = self.inner.lock().unwrap();
        Registrar::new(inner.router.clone(), inner.routes.clone())
    }

    /// Check if the application has been booted.
    pub fn is_booted(&self) -> bool {
        self.inner.lock().unwrap().booted
    }

    /// Return the number of registered providers (eager + deferred).
    pub fn provider_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.providers.len() + inner.deferred_providers.len()
    }

    /// Return the number of deferred providers awaiting activation.
    pub fn deferred_provider_count(&self) -> usize {
        self.inner.lock().unwrap().deferred_providers.len()
    }

    /// Add a Tower layer or Axum extension to the final router.
    ///
    /// The closure receives the fully-built `Router` (after all routes and
    /// the `/health` endpoint are registered) and must return a router.
    ///
    /// This is used to add middleware, CORS, compression, or Axum
    /// `Extension<T>` state to the application:
    ///
    /// ```rust,ignore
    /// app.with_layer(|router| router.layer(Extension(my_state)));
    /// ```
    pub fn with_layer(&self, f: impl FnOnce(AxumRouter) -> AxumRouter + Send + 'static) {
        self.inner.lock().unwrap().layers.push(Box::new(f));
    }

    pub async fn run(self) {
        self.boot();

        let router = {
            let inner = self.inner.lock().unwrap();
            let registrar = Registrar::new(inner.router.clone(), inner.routes.clone());
            registrar.build()
        };

        // Auto-wire SessionLayer and CsrfLayer when an app key is configured.
        {
            let config = self.config();
            if let Some(key) = &config.app.key {
                if let Ok(encrypter) = Encrypter::new(key.as_bytes()) {
                    let session_layer =
                        SessionLayer::new(SessionConfig::default(), Some(Arc::new(encrypter)));
                    let csrf_layer =
                        CsrfLayer::new().except(vec!["/api/*".to_string(), "/health".to_string()]);

                    let mut inner = self.inner.lock().unwrap();
                    inner
                        .layers
                        .push(Box::new(move |router: AxumRouter| -> AxumRouter {
                            router.layer(csrf_layer).layer(session_layer)
                        }));
                }
            }
        }

        // Apply user-registered layers/extensions to the final router.
        let router = {
            let mut inner = self.inner.lock().unwrap();
            let layers = std::mem::take(&mut inner.layers);
            layers.into_iter().fold(router, |r, layer| layer(r))
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

    // -----------------------------------------------------------------------
    // Deferred provider tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_deferred_provider_not_booted_immediately() {
        struct DeferredTest;
        impl ServiceProvider for DeferredTest {
            fn register(&self, _app: &Application) {}
            fn provides(&self) -> Vec<&'static str> {
                vec!["deferred-service"]
            }
        }

        let app = Application::new(None);
        app.register_deferred_provider(Arc::new(DeferredTest));

        // Deferred provider should not be in the eager providers list
        assert_eq!(app.provider_count(), 1);
        // Provider count is 1 (deferred) + 0 (eager)
        assert_eq!(app.deferred_provider_count(), 1);
    }

    #[test]
    fn test_deferred_provider_activated_on_make_by_alias() {
        static REGISTERED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        static BOOTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

        struct DeferredTest;
        impl ServiceProvider for DeferredTest {
            fn register(&self, app: &Application) {
                REGISTERED.store(true, std::sync::atomic::Ordering::SeqCst);
                app.bind(42i32);
            }
            fn boot(&self, _app: &Application) {
                BOOTED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            fn provides(&self) -> Vec<&'static str> {
                vec!["the-answer"]
            }
        }

        let app = Application::new(None);
        app.alias("the-answer", TypeId::of::<i32>());
        app.register_deferred_provider(Arc::new(DeferredTest));

        assert!(!REGISTERED.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!BOOTED.load(std::sync::atomic::Ordering::SeqCst));

        let val: Option<i32> = app.make();
        assert_eq!(val, Some(42));
        assert!(REGISTERED.load(std::sync::atomic::Ordering::SeqCst));
        assert!(BOOTED.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(app.deferred_provider_count(), 0);
    }

    #[test]
    fn test_deferred_provider_not_activated_for_unrelated_types() {
        static CALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

        struct DeferredTest;
        impl ServiceProvider for DeferredTest {
            fn register(&self, _app: &Application) {
                CALLED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            fn provides(&self) -> Vec<&'static str> {
                vec!["only-service"]
            }
        }

        let app = Application::new(None);
        app.alias("only-service", TypeId::of::<String>());
        app.register_deferred_provider(Arc::new(DeferredTest));

        // Make a different type — should NOT activate the deferred provider
        app.bind(99i32);
        let _: Option<i32> = app.make();

        assert!(!CALLED.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(app.deferred_provider_count(), 1);
    }

    #[test]
    fn test_register_deferred_provider_with_empty_provides_not_activated() {
        // Providers with empty provides() list cannot be activated because
        // there's no service name to trigger deferral. This is expected
        // (programmer error) — our deferred provider requires provides().
        struct EmptyProvides;
        impl ServiceProvider for EmptyProvides {
            fn register(&self, _app: &Application) {}
            fn provides(&self) -> Vec<&'static str> {
                vec![]
            }
        }

        let app = Application::new(None);
        app.register_deferred_provider(Arc::new(EmptyProvides));
        assert_eq!(app.deferred_provider_count(), 1);
        // Never activated since nothing can trigger it
    }

    #[test]
    fn test_eager_provider_registered_and_booted_normally() {
        static REGISTERED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);

        struct EagerTest;
        impl ServiceProvider for EagerTest {
            fn register(&self, _app: &Application) {
                REGISTERED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            fn provides(&self) -> Vec<&'static str> {
                vec!["eager"]
            }
        }

        let app = Application::new(None);
        app.register_provider(Arc::new(EagerTest));

        assert!(REGISTERED.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(app.provider_count(), 1);
        assert_eq!(app.deferred_provider_count(), 0);
    }

    #[test]
    fn test_mixed_eager_and_deferred_providers() {
        static EAGER_REGISTERED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        static DEFERRED_REGISTERED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);

        struct EagerTest;
        impl ServiceProvider for EagerTest {
            fn register(&self, _app: &Application) {
                EAGER_REGISTERED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            fn provides(&self) -> Vec<&'static str> {
                vec!["eager-svc"]
            }
        }

        struct DeferredTest;
        impl ServiceProvider for DeferredTest {
            fn register(&self, _app: &Application) {
                DEFERRED_REGISTERED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            fn provides(&self) -> Vec<&'static str> {
                vec!["deferred-svc"]
            }
        }

        let app = Application::new(None);
        app.alias("deferred-svc", TypeId::of::<String>());
        app.register_provider(Arc::new(EagerTest));
        app.register_deferred_provider(Arc::new(DeferredTest));

        assert!(EAGER_REGISTERED.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!DEFERRED_REGISTERED.load(std::sync::atomic::Ordering::SeqCst));

        assert_eq!(app.provider_count(), 2);
        assert_eq!(app.deferred_provider_count(), 1);
    }

    #[test]
    fn test_deferred_provider_boot_not_called_if_not_resolved() {
        static BOOTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

        struct DeferredTest;
        impl ServiceProvider for DeferredTest {
            fn register(&self, _app: &Application) {}
            fn boot(&self, _app: &Application) {
                BOOTED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            fn provides(&self) -> Vec<&'static str> {
                vec!["never-resolved"]
            }
        }

        let app = Application::new(None);
        app.alias("never-resolved", TypeId::of::<String>());
        app.register_deferred_provider(Arc::new(DeferredTest));

        // Boot the app — deferred providers should NOT be booted
        app.boot();
        assert!(!BOOTED.load(std::sync::atomic::Ordering::SeqCst));
    }
}
