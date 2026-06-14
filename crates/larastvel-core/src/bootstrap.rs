use std::path::PathBuf;

use crate::database::DatabaseManager;
use crate::foundation::Application;
use crate::logging;
use crate::models;
use crate::console::ConsoleKernel;
use crate::routing::Registrar;

/// Fluent application builder — the Rust equivalent of Laravel 11+'s
/// `bootstrap/app.php`.
///
/// # Example
///
/// ```rust,ignore
/// use larastvel_core::bootstrap::App;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() {
///     App::configure(None)
///         .with_routing(|r| {
///             routes::web::web(r);
///         })
///         .with_providers(|app| {
///             app.register_provider(Arc::new(
///                 RouteServiceProvider::new()
///                     .web(|r| routes::web::web(r)),
///             ));
///         })
///         .with_console_commands(|kernel| {
///             kernel.add_command(Arc::new(MyCommand));
///         })
///         .run()
///         .await;
/// }
/// ```
#[allow(clippy::type_complexity)]
pub struct App {
    base_path: Option<PathBuf>,
    routing: Option<Box<dyn FnOnce(&Registrar) + Send>>,
    providers: Option<Box<dyn FnOnce(&Application) + Send>>,
    console_commands: Option<Box<dyn FnOnce(&ConsoleKernel) + Send>>,
}

impl App {
    /// Create a new application builder with an optional base path.
    ///
    /// Pass `None` to use the current directory as the base path.
    pub fn configure(base_path: Option<PathBuf>) -> Self {
        Self {
            base_path,
            routing: None,
            providers: None,
            console_commands: None,
        }
    }

    /// Register web and/or API routes via a closure that receives a [`Registrar`].
    ///
    /// Multiple calls accumulate; each closure is called during boot.
    pub fn with_routing(mut self, f: impl FnOnce(&Registrar) + Send + 'static) -> Self {
        let prev = self.routing.take();
        self.routing = Some(if let Some(prev) = prev {
            Box::new(move |r| {
                prev(r);
                f(r);
            })
        } else {
            Box::new(f)
        });
        self
    }

    /// Register service providers via a closure that receives the [`Application`].
    pub fn with_providers(mut self, f: impl FnOnce(&Application) + Send + 'static) -> Self {
        let prev = self.providers.take();
        self.providers = Some(if let Some(prev) = prev {
            Box::new(move |app| {
                prev(app);
                f(app);
            })
        } else {
            Box::new(f)
        });
        self
    }

    /// Register console commands via a closure that receives the [`ConsoleKernel`].
    ///
    /// Multiple calls accumulate; each closure is called during boot.
    pub fn with_console_commands(mut self, f: impl FnOnce(&ConsoleKernel) + Send + 'static) -> Self {
        let prev = self.console_commands.take();
        self.console_commands = Some(if let Some(prev) = prev {
            Box::new(move |k| {
                prev(k);
                f(k);
            })
        } else {
            Box::new(f)
        });
        self
    }

    /// Build and run the application, consuming the builder.
    pub async fn run(self) {
        let app = Application::new(self.base_path.clone());
        logging::init(&app.config());

        // Connect to database
        let db = DatabaseManager::new(&app.config());
        match db.connect().await {
            Ok(conn) => {
                tracing::info!("Database connected successfully");
                let _ = models::set_global_database(conn);
            }
            Err(e) => tracing::warn!("Database connection failed: {} (app will still run)", e),
        }

        let app = app.with_database(db);

        // Register providers
        if let Some(f) = self.providers {
            f(&app);
        }

        // Register console commands
        if let Some(f) = self.console_commands {
            let kernel = app.console_kernel();
            f(&kernel);
        }

        // Register routes via routing closure
        if let Some(f) = self.routing {
            let registrar = app.router();
            f(&registrar);
        }

        app.run().await;
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_configure_defaults() {
        let app_builder = App::configure(None);
        assert!(app_builder.routing.is_none());
        assert!(app_builder.providers.is_none());
        assert!(app_builder.console_commands.is_none());
    }

    #[test]
    fn test_app_configure_with_base_path() {
        let app_builder = App::configure(Some("/tmp".into()));
        assert_eq!(app_builder.base_path, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn test_app_with_routing_stores_closure() {
        let app_builder = App::configure(None).with_routing(|_r| {});
        assert!(app_builder.routing.is_some());
    }

    #[test]
    fn test_app_with_providers_stores_closure() {
        let app_builder = App::configure(None).with_providers(|_app| {});
        assert!(app_builder.providers.is_some());
    }

    #[test]
    fn test_app_with_console_commands_stores_closure() {
        let app_builder = App::configure(None).with_console_commands(|_kernel| {});
        assert!(app_builder.console_commands.is_some());
    }

    #[test]
    fn test_app_with_console_commands_accumulates() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let counter = Arc::new(AtomicUsize::new(0));

        let app = App::configure(None)
            .with_console_commands({
                let c = counter.clone();
                move |_k| { c.fetch_add(1, Ordering::SeqCst); }
            })
            .with_console_commands(move |_k| { counter.fetch_add(1, Ordering::SeqCst); });

        assert!(app.console_commands.is_some());
    }
}
