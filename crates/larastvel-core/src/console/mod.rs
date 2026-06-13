use std::collections::HashMap;

use crate::foundation::Application;

pub struct ConsoleKernel {
    app: Application,
    commands: HashMap<String, Box<dyn CommandRegister>>,
}

pub trait CommandRegister: Send + Sync {
    fn signature(&self) -> String;
    fn description(&self) -> String;
    fn handle(&self, app: &Application);
}

impl ConsoleKernel {
    pub fn new(app: Application) -> Self {
        Self {
            app,
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, cmd: Box<dyn CommandRegister>) {
        let sig = cmd.signature();
        self.commands.insert(sig, cmd);
    }

    pub fn run(&self, args: &[String]) {
        if args.len() < 2 {
            self.print_help();
            return;
        }

        let command = &args[1];
        if command == "list" || command == "--help" || command == "-h" {
            self.print_help();
            return;
        }

        if let Some(cmd) = self.commands.get(command) {
            cmd.handle(&self.app);
        } else {
            eprintln!("Unknown command: {}", command);
            self.print_help();
        }
    }

    fn print_help(&self) {
        println!("Larastvel Framework CLI");
        println!();
        println!("Available commands:");
        for (name, cmd) in &self.commands {
            println!("  {:<20} {}", name, cmd.description());
        }
    }
}

pub struct ServeCommand;
impl CommandRegister for ServeCommand {
    fn signature(&self) -> String {
        "serve".to_string()
    }
    fn description(&self) -> String {
        "Start the Larastvel development server".to_string()
    }
    fn handle(&self, _app: &Application) {
        println!("Starting Larastvel development server...");
    }
}

pub struct RouteListCommand;
impl CommandRegister for RouteListCommand {
    fn signature(&self) -> String {
        "route:list".to_string()
    }
    fn description(&self) -> String {
        "List all registered routes".to_string()
    }
    fn handle(&self, _app: &Application) {
        println!("Route List:");
    }
}

pub struct MakeModelCommand;
impl CommandRegister for MakeModelCommand {
    fn signature(&self) -> String {
        "make:model".to_string()
    }
    fn description(&self) -> String {
        "Create a new Eloquent model class".to_string()
    }
    fn handle(&self, _app: &Application) {
        println!("Creating model...");
    }
}

pub struct MakeControllerCommand;
impl CommandRegister for MakeControllerCommand {
    fn signature(&self) -> String {
        "make:controller".to_string()
    }
    fn description(&self) -> String {
        "Create a new controller class".to_string()
    }
    fn handle(&self, _app: &Application) {
        println!("Creating controller...");
    }
}

pub struct MakeMigrationCommand;
impl CommandRegister for MakeMigrationCommand {
    fn signature(&self) -> String {
        "make:migration".to_string()
    }
    fn description(&self) -> String {
        "Create a new migration file".to_string()
    }
    fn handle(&self, _app: &Application) {
        println!("Creating migration...");
    }
}

pub struct MigrateCommand;
impl CommandRegister for MigrateCommand {
    fn signature(&self) -> String {
        "migrate".to_string()
    }
    fn description(&self) -> String {
        "Run the database migrations".to_string()
    }
    fn handle(&self, _app: &Application) {
        println!("Running migrations...");
    }
}

pub struct DbSeedCommand;
impl CommandRegister for DbSeedCommand {
    fn signature(&self) -> String {
        "db:seed".to_string()
    }
    fn description(&self) -> String {
        "Seed the database with records".to_string()
    }
    fn handle(&self, _app: &Application) {
        println!("Seeding database...");
    }
}

pub struct KeyGenerateCommand;
impl CommandRegister for KeyGenerateCommand {
    fn signature(&self) -> String {
        "key:generate".to_string()
    }
    fn description(&self) -> String {
        "Generate a new application key".to_string()
    }
    fn handle(&self, _app: &Application) {
        let key = uuid::Uuid::new_v4().to_string();
        println!("Application key [{}] generated.", key);
    }
}
