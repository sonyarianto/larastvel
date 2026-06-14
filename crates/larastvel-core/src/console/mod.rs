use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::foundation::Application;

/// A registered CLI command — the Rust equivalent of a Laravel `Artisan`
/// console command.
///
/// # Example
///
/// ```rust,ignore
/// use larastvel_core::console::Command;
///
/// struct InspireCommand;
///
/// impl Command for InspireCommand {
///     fn name(&self) -> &'static str {
///         "inspire"
///     }
///     fn description(&self) -> &'static str {
///         "Display an inspiring quote"
///     }
///     fn handle(&self, _app: &Application, _args: &[String]) -> Result<(), String> {
///         println!("“Simplicity is the ultimate sophistication.” — Leonardo da Vinci");
///         Ok(())
///     }
/// }
/// ```
pub trait Command: Send + Sync {
    /// The command name used on the CLI (e.g. `"make:model"`, `"serve"`).
    fn name(&self) -> &'static str;

    /// A short description shown in `list` / `--help`.
    fn description(&self) -> &'static str;

    /// Execute the command with the given arguments (excluding the binary and
    /// command name).
    fn handle(&self, app: &Application, args: &[String]) -> Result<(), String>;
}

/// The console kernel — registers and dispatches CLI commands.
///
/// Analogous to `App\Console\Kernel` in Laravel. Supports loading commands
/// from a closure, which mirrors the `routes/console.php` pattern.
///
/// # Example
///
/// ```rust,ignore
/// use larastvel_core::console::{ConsoleKernel, Command};
/// use std::sync::Arc;
///
/// let app = larastvel_core::Application::new(None);
/// let kernel = app.console_kernel();
///
/// kernel.add_command(Arc::new(MyCommand));
/// kernel.call("my-command", &[]).await?;
/// ```
pub struct ConsoleKernel {
    app: Application,
    commands: Arc<RwLock<HashMap<&'static str, Arc<dyn Command>>>>,
    loaded: Arc<RwLock<bool>>,
}

impl ConsoleKernel {
    /// Create a new kernel bound to the given application.
    pub fn new(app: Application) -> Self {
        Self {
            app,
            commands: Arc::new(RwLock::new(HashMap::new())),
            loaded: Arc::new(RwLock::new(false)),
        }
    }

    /// Register a command.
    ///
    /// If a command with the same `name()` already exists it is overwritten.
    pub fn add_command(&self, cmd: Arc<dyn Command>) {
        let name = cmd.name();
        self.commands.write().unwrap().insert(name, cmd);
    }

    /// Register multiple commands at once.
    pub fn add_commands(&self, cmds: Vec<Arc<dyn Command>>) {
        let mut map = self.commands.write().unwrap();
        for cmd in cmds {
            map.insert(cmd.name(), cmd);
        }
    }

    /// Returns `true` if a command with the given `name` has been registered.
    pub fn has_command(&self, name: &str) -> bool {
        self.commands.read().unwrap().contains_key(name)
    }

    /// Returns the number of registered commands.
    pub fn command_count(&self) -> usize {
        self.commands.read().unwrap().len()
    }

    /// Returns a list of all registered command names.
    pub fn command_names(&self) -> Vec<String> {
        self.commands
            .read()
            .unwrap()
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    /// Returns the command with the given name, if registered.
    pub fn get_command(&self, name: &str) -> Option<Arc<dyn Command>> {
        self.commands.read().unwrap().get(name).cloned()
    }

    /// Load commands via a closure — the Rust equivalent of defining
    /// Artisan commands in `routes/console.php`.
    ///
    /// The closure receives this kernel so commands can be registered
    /// with [`add_command`](Self::add_command) or
    /// [`add_commands`](Self::add_commands).
    ///
    /// The closure is guaranteed to run at most once.
    pub fn load(&self, f: impl FnOnce(&Self)) {
        let mut loaded = self.loaded.write().unwrap();
        if !*loaded {
            f(self);
            *loaded = true;
        }
    }

    /// Execute a command by name with the given arguments.
    ///
    /// Returns `Ok(())` on success, or `Err(message)` on failure.
    pub fn call(&self, name: &str, args: &[String]) -> Result<(), String> {
        let cmd = self
            .commands
            .read()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Unknown command: {}", name))?;
        cmd.handle(&self.app, args)
    }

    /// Return the underlying application.
    pub fn app(&self) -> &Application {
        &self.app
    }
}

impl Clone for ConsoleKernel {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            commands: self.commands.clone(),
            loaded: self.loaded.clone(),
        }
    }
}

// ============================================================================
// Built-in stub commands
// ============================================================================

macro_rules! stub_command {
    ($name:ident, $cmd:expr, $desc:expr) => {
        pub struct $name;
        impl Command for $name {
            fn name(&self) -> &'static str {
                $cmd
            }
            fn description(&self) -> &'static str {
                $desc
            }
            fn handle(&self, _app: &Application, _args: &[String]) -> Result<(), String> {
                println!("[stub] {} — {}", $cmd, $desc);
                Ok(())
            }
        }
    };
}

stub_command!(
    ServeCommand,
    "serve",
    "Start the Larastvel development server"
);
stub_command!(RouteListCommand, "route:list", "List all registered routes");
stub_command!(MakeModelCommand, "make:model", "Create a new model class");
stub_command!(
    MakeControllerCommand,
    "make:controller",
    "Create a new controller class"
);
stub_command!(
    MakeMigrationCommand,
    "make:migration",
    "Create a new migration file"
);
stub_command!(MigrateCommand, "migrate", "Run the database migrations");
stub_command!(DbSeedCommand, "db:seed", "Seed the database with records");
stub_command!(
    KeyGenerateCommand,
    "key:generate",
    "Generate a new application key"
);

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct GreetCommand;
    impl Command for GreetCommand {
        fn name(&self) -> &'static str {
            "greet"
        }
        fn description(&self) -> &'static str {
            "Greet someone"
        }
        fn handle(&self, _app: &Application, args: &[String]) -> Result<(), String> {
            let name = args.first().map(|s| s.as_str()).unwrap_or("world");
            println!("Hello, {}!", name);
            Ok(())
        }
    }

    struct FailCommand;
    impl Command for FailCommand {
        fn name(&self) -> &'static str {
            "fail"
        }
        fn description(&self) -> &'static str {
            "Always fails"
        }
        fn handle(&self, _app: &Application, _args: &[String]) -> Result<(), String> {
            Err("Something went wrong".to_string())
        }
    }

    #[test]
    fn test_add_and_has_command() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        kernel.add_command(Arc::new(GreetCommand));
        assert!(kernel.has_command("greet"));
        assert!(!kernel.has_command("nonexistent"));
    }

    #[test]
    fn test_command_count() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        assert_eq!(kernel.command_count(), 0);
        kernel.add_command(Arc::new(GreetCommand));
        assert_eq!(kernel.command_count(), 1);
        kernel.add_command(Arc::new(FailCommand));
        assert_eq!(kernel.command_count(), 2);
    }

    #[test]
    fn test_command_names() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        kernel.add_command(Arc::new(GreetCommand));
        kernel.add_command(Arc::new(FailCommand));
        let mut names = kernel.command_names();
        names.sort();
        assert_eq!(names, vec!["fail", "greet"]);
    }

    #[test]
    fn test_get_command_returns_arc() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        kernel.add_command(Arc::new(GreetCommand));
        let cmd = kernel.get_command("greet");
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().name(), "greet");
    }

    #[test]
    fn test_get_command_nonexistent() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        assert!(kernel.get_command("nope").is_none());
    }

    #[test]
    fn test_call_unknown_command() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        let result = kernel.call("unknown", &[]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Unknown command: unknown");
    }

    #[test]
    fn test_call_success() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        kernel.add_command(Arc::new(GreetCommand));
        let result = kernel.call("greet", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_failure() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        kernel.add_command(Arc::new(FailCommand));
        let result = kernel.call("fail", &[]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Something went wrong");
    }

    #[test]
    fn test_call_with_args() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        kernel.add_command(Arc::new(GreetCommand));
        let result = kernel.call("greet", &["Alice".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_commands_bulk() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        kernel.add_commands(vec![Arc::new(GreetCommand), Arc::new(FailCommand)]);
        assert_eq!(kernel.command_count(), 2);
        assert!(kernel.has_command("greet"));
        assert!(kernel.has_command("fail"));
    }

    #[test]
    fn test_last_command_wins_on_name_collision() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);

        struct CmdA;
        impl Command for CmdA {
            fn name(&self) -> &'static str {
                "same"
            }
            fn description(&self) -> &'static str {
                "first"
            }
            fn handle(&self, _app: &Application, _args: &[String]) -> Result<(), String> {
                Ok(())
            }
        }

        struct CmdB;
        impl Command for CmdB {
            fn name(&self) -> &'static str {
                "same"
            }
            fn description(&self) -> &'static str {
                "second"
            }
            fn handle(&self, _app: &Application, _args: &[String]) -> Result<(), String> {
                Ok(())
            }
        }

        kernel.add_command(Arc::new(CmdA));
        kernel.add_command(Arc::new(CmdB));
        let cmd = kernel.get_command("same").unwrap();
        assert_eq!(cmd.description(), "second");
    }

    #[test]
    fn test_load_runs_once() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);
        let counter = std::sync::atomic::AtomicUsize::new(0);

        kernel.load(|k| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            k.add_command(Arc::new(GreetCommand));
        });

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(kernel.has_command("greet"));

        // Second call should be a no-op
        kernel.load(|_k| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_stub_commands_are_registrable() {
        let app = Application::new(None);
        let kernel = ConsoleKernel::new(app);

        let stubs: Vec<Arc<dyn Command>> = vec![
            Arc::new(ServeCommand),
            Arc::new(RouteListCommand),
            Arc::new(MakeModelCommand),
            Arc::new(MakeControllerCommand),
            Arc::new(MakeMigrationCommand),
            Arc::new(MigrateCommand),
            Arc::new(DbSeedCommand),
            Arc::new(KeyGenerateCommand),
        ];

        for stub in &stubs {
            kernel.add_command(stub.clone());
        }

        assert_eq!(kernel.command_count(), 8);
        assert!(kernel.has_command("serve"));
        assert!(kernel.has_command("route:list"));
        assert!(kernel.has_command("make:model"));
        assert!(kernel.has_command("migrate"));
        assert!(kernel.has_command("key:generate"));

        for stub in &stubs {
            let result = kernel.call(stub.name(), &[]);
            assert!(result.is_ok(), "stub {} failed: {:?}", stub.name(), result);
        }
    }
}
