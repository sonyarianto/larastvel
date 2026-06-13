//! # RouteServiceProvider
//!
//! Loads route files (web, api, etc.) during the boot phase.
//!
//! In Rust, route files are compiled into the binary (not loaded at runtime
//! like PHP), so this provider takes route-loading closures instead of file
//! paths.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use larastvel_core::foundation::{Application, RouteServiceProvider};
//!
//! let app = Application::new(None);
//! app.register_provider(std::sync::Arc::new(
//!     RouteServiceProvider::new()
//!         .web(|r| {
//!             r.get("/", || async { "Home" });
//!         })
//!         .api(|r| {
//!             r.get("/health", || async { "ok" });
//!         })
//! ));
//! ```

use std::sync::Arc;

use crate::foundation::{Application, ServiceProvider};
use crate::routing::Registrar;

type RouteLoader = Arc<dyn Fn(&Registrar) + Send + Sync>;

/// Loads route files (web, api, etc.) during the boot phase.
///
/// This is the Rust equivalent of Laravel's
/// `app/Providers/RouteServiceProvider.php`.
pub struct RouteServiceProvider {
    groups: Vec<(String, RouteLoader)>,
}

impl RouteServiceProvider {
    /// Create a new `RouteServiceProvider` with no route groups.
    pub fn new() -> Self {
        Self { groups: vec![] }
    }

    /// Register a route group loaded by a closure.
    ///
    /// The closure receives a `&Registrar` to register routes.
    /// It will be called during `boot()`.
    pub fn group(
        mut self,
        name: &str,
        f: impl Fn(&Registrar) + Send + Sync + 'static,
    ) -> Self {
        self.groups.push((name.to_string(), Arc::new(f)));
        self
    }

    /// Register the "web" route group.
    ///
    /// ```rust,ignore
    /// .web(|r| routes::web::web(r))
    /// ```
    pub fn web(self, f: impl Fn(&Registrar) + Send + Sync + 'static) -> Self {
        self.group("web", f)
    }

    /// Register the "api" route group.
    ///
    /// ```rust,ignore
    /// .api(|r| routes::api::api(r))
    /// ```
    pub fn api(self, f: impl Fn(&Registrar) + Send + Sync + 'static) -> Self {
        self.group("api", f)
    }

    /// Register the "console" route group.
    ///
    /// ```rust,ignore
    /// .console(|r| routes::console::console(r))
    /// ```
    pub fn console(self, f: impl Fn(&Registrar) + Send + Sync + 'static) -> Self {
        self.group("console", f)
    }
}

impl Default for RouteServiceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _app: &Application) {
        // Routes are registered in boot() after all container bindings.
    }

    fn boot(&self, app: &Application) {
        let registrar = app.router();
        for (_name, loader) in &self.groups {
            loader(&registrar);
        }
    }

    fn provides(&self) -> Vec<&'static str> {
        vec!["routes"]
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::Application;

    #[test]
    fn test_route_service_provider_boots_routes() {
        let provider = RouteServiceProvider::new().web(|r| {
            r.get("/test-route", || async { "hello" });
        });

        let app = Application::new(None);
        provider.boot(&app);

        let routes = app.router().list_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, "GET");
        assert_eq!(routes[0].uri, "/test-route");
    }

    #[test]
    fn test_route_service_provider_multiple_groups() {
        let provider = RouteServiceProvider::new()
            .web(|r| {
                r.get("/", || async { "home" });
            })
            .api(|r| {
                r.get("/health", || async { "ok" });
            });

        let app = Application::new(None);
        provider.boot(&app);

        let routes = app.router().list_routes();
        assert_eq!(routes.len(), 2);
    }

    #[test]
    fn test_route_service_provider_group_name() {
        let provider = RouteServiceProvider::new()
            .group("admin", |r| {
                r.get("/admin", || async { "admin" });
            });

        assert_eq!(provider.groups[0].0, "admin");
    }

    #[test]
    fn test_route_service_provider_web_shorthand() {
        let provider = RouteServiceProvider::new().web(|r| {
            r.get("/welcome", || async { "welcome" });
        });

        // Just verify it doesn't panic and routes are registered
        let app = Application::new(None);
        provider.boot(&app);
        assert_eq!(app.router().list_routes().len(), 1);
    }

    #[test]
    fn test_route_service_provider_api_shorthand() {
        let provider = RouteServiceProvider::new().api(|r| {
            r.get("/ping", || async { "pong" });
        });

        let app = Application::new(None);
        provider.boot(&app);
        assert_eq!(app.router().list_routes().len(), 1);
    }

    #[test]
    fn test_route_service_provider_default() {
        let provider = RouteServiceProvider::default();
        let app = Application::new(None);
        provider.boot(&app);
        assert!(app.router().list_routes().is_empty());
    }

    #[test]
    fn test_service_provider_trait_impl() {
        let provider = RouteServiceProvider::new();
        assert_eq!(provider.provides(), vec!["routes"]);
    }

    #[test]
    fn test_route_service_provider_boot_twice() {
        let provider = RouteServiceProvider::new().web(|r| {
            r.get("/boot-first", || async { "first" });
        });

        let app = Application::new(None);
        provider.boot(&app);
        assert_eq!(app.router().list_routes().len(), 1);

        // Boot again with a different route (Axum rejects overlapping routes)
        let provider2 = RouteServiceProvider::new().web(|r| {
            r.get("/boot-second", || async { "second" });
        });
        provider2.boot(&app);
        assert_eq!(app.router().list_routes().len(), 2, "second boot should add more routes");
    }
}
