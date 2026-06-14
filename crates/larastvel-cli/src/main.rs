use std::path::PathBuf;

use clap::{Parser, Subcommand};
use colored::*;

#[derive(Parser)]
#[command(
    name = "larastvel",
    about = "Larastvel Framework CLI",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the development server
    Serve {
        #[arg(short, long, default_value = "8080")]
        port: u16,
        #[arg(short, long)]
        host: Option<String>,
    },
    /// List all registered routes
    #[command(name = "route:list")]
    RouteList,
    /// Display framework version
    Version,
    /// Run scheduled tasks
    #[command(name = "schedule:run")]
    ScheduleRun,
    /// Create a new Larastvel application
    New { name: String },
    /// Generate a new application key
    #[command(name = "key:generate")]
    KeyGenerate,
    /// Run database migrations
    Migrate,
    /// Drop all tables and re-run all migrations
    #[command(name = "migrate:fresh")]
    MigrateFresh,
    /// Rollback the last migration (or N steps)
    #[command(name = "migrate:rollback")]
    MigrateRollback {
        #[arg(short, long)]
        steps: Option<u32>,
    },
    /// Run database seeders
    #[command(name = "db:seed")]
    DbSeed,
    /// Create a symbolic link from public/storage to storage/app/public
    #[command(name = "storage:link")]
    StorageLink,
    /// Create a migration for the notifications table
    #[command(name = "notifications:table")]
    NotificationsTable,
    /// Cache config values into a single file for faster loading
    #[command(name = "config:cache")]
    ConfigCache,
    /// Clear the cached config file
    #[command(name = "config:clear")]
    ConfigClear,
    /// Cache all registered routes into a single file
    #[command(name = "route:cache")]
    RouteCache,
    /// Clear the cached routes file
    #[command(name = "route:clear")]
    RouteClear,
    /// Display the current environment variables (.env + config)
    Env,
    /// Put the application into maintenance mode
    Down {
        /// The message to display to visitors
        #[arg(short, long)]
        message: Option<String>,
        /// The number of seconds after which a request may be retried
        #[arg(short, long)]
        retry: Option<u64>,
        /// Allow maintenance mode even if the down file already exists
        #[arg(long)]
        force: bool,
    },
    /// Bring the application out of maintenance mode
    Up,
    /// List all registered scheduled tasks
    #[command(name = "schedule:list")]
    ScheduleList,
    /// Start processing jobs on the queue
    #[command(name = "queue:work")]
    QueueWork {
        /// Process only a single job from the queue
        #[arg(short, long)]
        once: bool,
        /// The name of the queue to work on
        #[arg(short, long, default_value = "default")]
        queue: String,
        /// Sleep duration (in seconds) when no job is available
        #[arg(long, default_value = "3")]
        sleep: u64,
    },
    /// Display help for any command
    Make {
        #[command(subcommand)]
        target: Option<MakeTarget>,
    },
}

#[derive(Subcommand)]
enum MakeTarget {
    /// Create a new model
    Model { name: String },
    /// Create a new controller
    Controller { name: String },
    /// Create a new migration
    Migration { name: String },
    /// Create a new seeder
    Seeder { name: String },
    /// Create a new policy
    Policy { name: String },
    /// Create a new test
    Test { name: String },
    /// Create a new job
    Job { name: String },
    /// Create a new event
    Event { name: String },
    /// Create a new notification
    Notification { name: String },
    /// Create a new validation rule
    Rule { name: String },
    /// Create a new console command
    Command { name: String },
    /// Create a new mail class
    Mail { name: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve { port, host }) => {
            let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
            println!(
                "{}",
                format!(
                    "⚡ Larastvel development server starting on http://{}:{}",
                    host, port
                )
                .green()
                .bold()
            );
            println!("{}", "  Press Ctrl+C to stop.".dimmed());
            start_server(&host, port).await;
        }
        Some(Commands::RouteList) => {
            println!("{}", "Route List".cyan().bold());
            println!("{}", "  No routes defined yet.".dimmed());
        }
        Some(Commands::Version) => {
            println!("Larastvel Framework v{}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::ScheduleRun) => {
            run_schedule_command().await;
        }
        Some(Commands::New { name }) => {
            println!(
                "{}",
                format!("Creating new Larastvel application: {}", name)
                    .green()
                    .bold()
            );
            create_project(&name).await;
        }
        Some(Commands::KeyGenerate) => {
            let key = uuid::Uuid::new_v4().to_string();
            println!("{}", "Application key generated:".green());
            println!("  APP_KEY={}", key.cyan());
        }
        Some(Commands::Migrate) => {
            run_migrate_command("migrate");
        }
        Some(Commands::MigrateFresh) => {
            run_migrate_command("migrate:fresh");
        }
        Some(Commands::MigrateRollback { steps }) => {
            let cmd = match steps {
                Some(n) => format!("migrate:rollback --steps {}", n),
                None => "migrate:rollback".to_string(),
            };
            run_migrate_command(&cmd);
        }
        Some(Commands::DbSeed) => {
            run_seed_command();
        }
        Some(Commands::StorageLink) => {
            storage_link();
        }
        Some(Commands::NotificationsTable) => {
            create_notifications_table();
        }
        Some(Commands::ConfigCache) => {
            config_cache();
        }
        Some(Commands::ConfigClear) => {
            config_clear();
        }
        Some(Commands::RouteCache) => {
            route_cache().await;
        }
        Some(Commands::RouteClear) => {
            route_clear();
        }
        Some(Commands::Env) => {
            env_display();
        }
        Some(Commands::Down {
            message,
            retry,
            force,
        }) => {
            maintenance_down(message, retry, force);
        }
        Some(Commands::Up) => {
            maintenance_up();
        }
        Some(Commands::ScheduleList) => {
            schedule_list().await;
        }
        Some(Commands::QueueWork { once, queue, sleep }) => {
            queue_work(once, &queue, sleep).await;
        }
        Some(Commands::Make { target }) => match target {
            Some(MakeTarget::Model { name }) => {
                make_model(&name);
            }
            Some(MakeTarget::Controller { name }) => {
                make_controller(&name);
            }
            Some(MakeTarget::Migration { name }) => {
                make_migration(&name);
            }
            Some(MakeTarget::Seeder { name }) => {
                make_seeder(&name);
            }
            Some(MakeTarget::Policy { name }) => {
                make_policy(&name);
            }
            Some(MakeTarget::Test { name }) => {
                make_test(&name);
            }
            Some(MakeTarget::Job { name }) => {
                make_job(&name);
            }
            Some(MakeTarget::Event { name }) => {
                make_event(&name);
            }
            Some(MakeTarget::Notification { name }) => {
                make_notification(&name);
            }
            Some(MakeTarget::Rule { name }) => {
                make_rule(&name);
            }
            Some(MakeTarget::Command { name }) => {
                make_command(&name);
            }
            Some(MakeTarget::Mail { name }) => {
                make_mail(&name);
            }
            None => {
                println!("{}", "Available make targets:".cyan());
                println!("  make:model       Create a new model");
                println!("  make:controller  Create a new controller");
                println!("  make:migration   Create a new migration");
                println!("  make:seeder      Create a new seeder");
                println!("  make:policy      Create a new policy");
                println!("  make:test        Create a new test");
                println!("  make:job         Create a new job");
                println!("  make:event       Create a new event");
                println!("  make:notification Create a new notification");
                println!("  make:rule        Create a new validation rule");
                println!("  make:command     Create a new console command");
                println!("  make:mail        Create a new mail class");
            }
        },
        None => {
            println!("{}", "Larastvel Framework CLI".cyan().bold());
            println!("{}", "Usage: larastvel <command>".dimmed());
            println!();
            println!("Available commands:");
            println!("  serve            Start the development server");
            println!("  route:list       List all registered routes");
            println!("  new              Create a new Larastvel application");
            println!("  key:generate     Generate a new application key");
            println!("  migrate          Run database migrations");
            println!("  migrate:fresh    Drop all tables and re-run migrations");
            println!("  migrate:rollback Rollback the last migration");
            println!("  make:model       Create a new model");
            println!("  make:controller  Create a new controller");
            println!("  make:migration   Create a new migration");
            println!("  make:seeder      Create a new seeder");
            println!("  make:policy      Create a new policy");
            println!("  make:test        Create a new test");
            println!("  make:job         Create a new job");
            println!("  make:event       Create a new event");
            println!("  make:notification Create a new notification");
            println!("  make:rule        Create a new validation rule");
            println!("  make:command     Create a new console command");
            println!("  schedule:run     Run scheduled tasks");
            println!("  db:seed          Run database seeders");
            println!("  storage:link     Create a symbolic link from public/storage to storage/app/public");
            println!("  notifications:table  Create a migration for the notifications table");
            println!("  config:cache     Cache config values into a single file");
            println!("  config:clear     Clear the cached config file");
            println!("  route:cache      Cache all registered routes into a single file");
            println!("  route:clear      Clear the cached routes file");
            println!(
                "  env              Display the current environment variables (.env + config)"
            );
            println!("  down             Put the application into maintenance mode");
            println!("  up               Bring the application out of maintenance mode");
            println!("  schedule:list    List all registered scheduled tasks");
            println!("  queue:work       Start processing jobs on the queue");
            println!("  version          Display framework version");
        }
    }
}

async fn start_server(host: &str, port: u16) {
    let addr = format!("{}:{}", host, port);
    println!("  Server running on http://{}", addr.green());

    let app = larastvel_core::axum::Router::new().route(
        "/health",
        larastvel_core::axum::routing::get(|| async {
            larastvel_core::axum::response::Json(serde_json::json!({
                "status": "ok",
                "framework": "Larastvel",
                "version": env!("CARGO_PKG_VERSION"),
            }))
        }),
    );

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    larastvel_core::axum::serve(listener, app).await.unwrap();
}

async fn create_project(name: &str) {
    let path = PathBuf::from(name);
    if path.exists() {
        eprintln!(
            "{}",
            format!("Error: Directory '{}' already exists.", name).red()
        );
        std::process::exit(1);
    }

    std::fs::create_dir_all(&path).unwrap();
    std::fs::create_dir_all(path.join("src/models")).unwrap();
    std::fs::create_dir_all(path.join("src/controllers")).unwrap();
    std::fs::create_dir_all(path.join("resources/views")).unwrap();
    std::fs::create_dir_all(path.join("resources/js")).unwrap();
    std::fs::create_dir_all(path.join("resources/css")).unwrap();
    std::fs::create_dir_all(path.join("public")).unwrap();
    std::fs::create_dir_all(path.join("routes")).unwrap();
    std::fs::create_dir_all(path.join("config")).unwrap();
    std::fs::create_dir_all(path.join("database/migrations")).unwrap();
    std::fs::create_dir_all(path.join("storage/logs")).unwrap();
    std::fs::create_dir_all(path.join("storage/app")).unwrap();

    let main_rs = format!(
        r#"use larastvel_core::{{Application, Config, DatabaseManager, logging}};

mod controllers;
mod models;
mod routes;

#[tokio::main]
async fn main() {{
    let app = Application::new(None);
    logging::init(&app.config());

    let db = DatabaseManager::new(&app.config());
    match db.connect().await {{
        Ok(conn) => {{
            tracing::info!("Database connected successfully");
            let _ = larastvel_core::models::set_global_database(conn);
        }}
        Err(e) => tracing::warn!("Database connection failed: {{}} (app will still run)", e),
    }}
    let app = app.with_database(db);

    println!("⚡ {name} starting up...");
    app.run().await;
}}
"#,
        name = name
    );

    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
larastvel-core = "0.1"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tracing = "0.1"
sea-orm-migration = "1"
"#,
        name = name
    );

    let config_toml = format!(
        r#"[app]
name = "{name}"
url = "http://localhost:8080"
env = "local"
debug = true

[database]
driver = "sqlite"
host = "127.0.0.1"
port = 3306
database = "larastvel.db"
username = "root"
password = ""

[logging]
level = "debug"
format = "text"

[view]
engine = "tera"
paths = ["resources/views"]
"#,
        name = name
    );

    let vite_config = r#"import { defineConfig } from 'vite';
import laravel from 'vite-plugin-laravel';

export default defineConfig({
    plugins: [
        laravel(),
    ],
    server: {
        port: 5173,
        hmr: {
            host: 'localhost',
        },
    },
});
"#;

    let package_json = r#"{
    "private": true,
    "type": "module",
    "scripts": {
        "dev": "vite",
        "build": "vite build",
        "preview": "vite preview"
    },
    "devDependencies": {
        "vite": "^6.0.0",
        "vite-plugin-laravel": "^0.4.0",
        "autoprefixer": "^10.4.0",
        "postcss": "^8.4.0",
        "tailwindcss": "^3.4.0"
    }
}
"#;

    let welcome_view = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ title }}</title>
    <link rel="stylesheet" href="/css/app.css">
    @vite('resources/js/app.js')
</head>
<body>
    <div class="container">
        <h1>{{ title }}</h1>
        <p>{{ description }}</p>
    </div>
</body>
</html>
"#;

    let app_css = r#"@tailwind base;
@tailwind components;
@tailwind utilities;

.container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem;
}
"#;

    let app_js = r#"import './bootstrap';
"#;

    let bootstrap_js = r#"import axios from 'axios';
window.axios = axios;
window.axios.defaults.headers.common['X-Requested-With'] = 'XMLHttpRequest';
"#;

    let tailwind_config = r#"/** @type {import('tailwindcss').Config} */
export default {
    content: [
        './resources/**/*.html',
        './resources/**/*.rs',
        './src/**/*.rs',
    ],
    theme: {
        extend: {},
    },
    plugins: [],
};
"#;

    let postcss_config = r#"export default {
    plugins: {
        tailwindcss: {},
        autoprefixer: {},
    },
};
"#;

    let routes_file = r#"use larastvel_core::routing::Registrar;

pub fn web(router: &Registrar) {
    router.get("/", || async {
        larastvel_core::axum::response::Html("<h1>Welcome to Larastvel</h1>")
    });
}

pub fn api(router: &Registrar) {
    router.group("/api", |r| {
        r.get("/health", || async {
            larastvel_core::axum::response::Json(serde_json::json!({"status": "ok"}))
        });
    });
}
"#;

    let controllers_mod = "pub mod home_controller;\n";

    let home_controller = r#"use larastvel_core::axum::response::{IntoResponse, Json, Response};
use larastvel_core::Resource;
use serde_json::json;

#[derive(Resource)]
pub struct HomeController;

impl larastvel_core::routing::ResourceController for HomeController {
    const RESOURCE_NAME: &'static str = "home";

    async fn index() -> Response {
        Json(json!({"message": "Welcome to Larastvel"})).into_response()
    }
}
"#;

    let env_file = r#"APP_NAME=Larastvel
APP_ENV=local
APP_KEY=
APP_DEBUG=true
APP_URL=http://localhost:8080

DB_CONNECTION=sqlite
DB_HOST=127.0.0.1
DB_PORT=3306
DB_DATABASE=larastvel
DB_USERNAME=root
DB_PASSWORD=
"#;

    let models_mod = "pub mod user;\n";
    let user_model = r#"use larastvel_core::sea_orm;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub password: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct User;

impl larastvel_core::models::DbModel for User {
    type Entity = Entity;
}
"#;

    std::fs::write(path.join("src/controllers/mod.rs"), controllers_mod).unwrap();
    std::fs::write(
        path.join("src/controllers/home_controller.rs"),
        home_controller,
    )
    .unwrap();
    std::fs::write(path.join("src/models/mod.rs"), models_mod).unwrap();
    std::fs::write(path.join("src/models/user.rs"), user_model).unwrap();
    std::fs::write(path.join("Cargo.toml"), cargo_toml).unwrap();
    std::fs::write(path.join("src/main.rs"), main_rs).unwrap();
    std::fs::write(path.join("config.toml"), config_toml).unwrap();
    std::fs::write(path.join("vite.config.js"), vite_config).unwrap();
    std::fs::write(path.join("package.json"), package_json).unwrap();
    std::fs::write(path.join("resources/views/welcome.html"), welcome_view).unwrap();
    std::fs::write(path.join("resources/css/app.css"), app_css).unwrap();
    std::fs::write(path.join("resources/js/app.js"), app_js).unwrap();
    std::fs::write(path.join("resources/js/bootstrap.js"), bootstrap_js).unwrap();
    std::fs::write(path.join("tailwind.config.js"), tailwind_config).unwrap();
    std::fs::write(path.join("postcss.config.js"), postcss_config).unwrap();
    std::fs::write(path.join("routes/web.rs"), routes_file).unwrap();
    std::fs::write(path.join(".env"), env_file).unwrap();

    println!(
        "{}",
        format!("✓ Application [{}] created successfully!", name)
            .green()
            .bold()
    );
    println!();
    println!("Next steps:");
    println!("  cd {}", name);
    println!("  npm install");
    println!("  larastvel serve");
}

fn make_test(name: &str) {
    let tests_dir = std::path::Path::new("tests");
    std::fs::create_dir_all(tests_dir).unwrap();

    let snake_name = to_snake_case(name);
    let file_name = if snake_name.ends_with("_test") {
        snake_name
    } else {
        format!("{}_test", snake_name)
    };

    let test_content = format!(
        r#"use larastvel_core::TestClient;

/// Test: {name}
#[cfg(test)]
mod tests {{
    use super::*;

    // #[tokio::test]
    // async fn test_example() {{
    //     let client = TestClient::new(app);
    //     let response = client.get("/").await;
    //     response.assert_ok();
    // }}
}}
"#,
        name = name,
    );

    let file_path = tests_dir.join(format!("{}.rs", file_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Test '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, test_content).unwrap();

    println!(
        "{}",
        format!("✓ Test [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Add your test assertions inside the test module.".dimmed()
    );
}

fn make_job(name: &str) {
    let jobs_dir = std::path::Path::new("src/jobs");
    std::fs::create_dir_all(jobs_dir).unwrap();

    let snake_name = to_snake_case(name);
    let job_name = if snake_name.ends_with("_job") {
        name.to_string()
    } else {
        format!("{}Job", name)
    };

    let job_content = format!(
        r#"use larastvel_core::queue::{{JobError, ShouldQueue}};
use async_trait::async_trait;

#[derive(Debug)]
pub struct {name};

#[async_trait]
impl ShouldQueue for {name} {{
    fn name(&self) -> &str {{
        "{snake}"
    }}

    async fn handle(&self) -> Result<(), JobError> {{
        // TODO: Implement job logic
        tracing::info!("Job executed: {name}");
        Ok(())
    }}
}}
"#,
        name = job_name,
        snake = snake_name,
    );

    let file_path = jobs_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Job '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, job_content).unwrap();

    // Update mod.rs
    let mod_path = jobs_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Job [{}] created at '{}'.", job_name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Dispatch with: larastvel_core::queue::dispatch(MyJob).await;".dimmed()
    );
}

fn make_event(name: &str) {
    let events_dir = std::path::Path::new("src/events");
    std::fs::create_dir_all(events_dir).unwrap();

    let snake_name = to_snake_case(name);

    let event_content = format!(
        r#"use larastvel_core::events::Listener;
use async_trait::async_trait;

/// Event payload for {name}
#[derive(Debug, Clone)]
pub struct {name}Event {{
    // TODO: Add event data fields
}}

/// Listener for {name}
#[derive(Debug)]
pub struct {name}Listener;

#[async_trait]
impl Listener<{name}Event> for {name}Listener {{
    async fn handle(&self, _event: &{name}Event) {{
        // TODO: Handle the event
        tracing::info!("Event handled: {name}");
    }}
}}
"#,
        name = name,
    );

    let file_path = events_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Event '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, event_content).unwrap();

    // Update mod.rs
    let mod_path = events_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Event [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Register with: EventService::listen::<MyEvent, MyListener>();".dimmed()
    );
    println!(
        "{}",
        "  Dispatch with: larastvel_core::events::dispatch(MyEvent).await;".dimmed()
    );
}

fn make_notification(name: &str) {
    let notifications_dir = std::path::Path::new("src/notifications");
    std::fs::create_dir_all(notifications_dir).unwrap();

    let snake_name = to_snake_case(name);
    let struct_name = if name.ends_with("Notification") {
        name.to_string()
    } else {
        format!("{}Notification", name)
    };

    let notification_content = format!(
        r#"use larastvel_core::mail::{{Mailable, Mailer}};

/// Notification: {name}
#[derive(Debug)]
pub struct {struct_name} {{
    // TODO: Add notification data fields
}}

impl {struct_name} {{
    pub async fn send(&self, mailer: &dyn Mailer, to: &str) -> Result<(), Box<dyn std::error::Error>> {{
        let mailable = Mailable::html(
            vec![to.to_string()],
            "Notification: {name}",
            "<h1>{name}</h1><p>Your notification content here.</p>",
        )
        .from("noreply@example.com");

        mailer.send(mailable).await?;
        Ok(())
    }}
}}
"#,
        struct_name = struct_name,
        name = name,
    );

    let file_path = notifications_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Notification '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, notification_content).unwrap();

    // Update mod.rs
    let mod_path = notifications_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Notification [{}] created at '{}'.",
            struct_name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Customize the email template and data fields.".dimmed()
    );
}

fn make_rule(name: &str) {
    let rules_dir = std::path::Path::new("src/rules");
    std::fs::create_dir_all(rules_dir).unwrap();

    let snake_name = to_snake_case(name);
    let struct_name = if name.ends_with("Rule") {
        name.to_string()
    } else {
        format!("{}Rule", name)
    };

    let rule_content = format!(
        r#"use larastvel_core::validation::{{ValidationRule, ValidationError}};

/// Validation rule: {name}
#[derive(Debug, Clone)]
pub struct {struct_name} {{
    // TODO: Add rule parameters
}}

impl {struct_name} {{
    pub fn new() -> Self {{
        Self {{
            // TODO: Initialize rule parameters
        }}
    }}
}}

impl ValidationRule for {struct_name} {{
    fn name(&self) -> &str {{
        "{snake}"
    }}

    fn validate(&self, _field: &str, _value: &str) -> Result<(), ValidationError> {{
        // TODO: Implement validation logic
        // Return Ok(()) for valid, Err(ValidationError::new("message")) for invalid
        Ok(())
    }}
}}
"#,
        struct_name = struct_name,
        name = name,
        snake = snake_name,
    );

    let file_path = rules_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Rule '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, rule_content).unwrap();

    // Update mod.rs
    let mod_path = rules_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Rule [{}] created at '{}'.",
            struct_name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Add validation logic in the `validate` method.".dimmed()
    );
    println!(
        "{}",
        "  Register with: Validator::extend(MyRule::new());".dimmed()
    );
}

fn make_command(name: &str) {
    let commands_dir = std::path::Path::new("src/commands");
    std::fs::create_dir_all(commands_dir).unwrap();

    let snake_name = to_snake_case(name);
    let struct_name = if name.ends_with("Command") {
        name.to_string()
    } else {
        format!("{}Command", name)
    };

    let command_content = format!(
        r#"use clap::Parser;

/// {name}
#[derive(Debug, Parser)]
pub struct {struct_name} {{
    // TODO: Add command arguments
    // #[arg(short, long)]
    // pub name: Option<String>,
}}

impl {struct_name} {{
    pub async fn execute(&self) -> Result<(), Box<dyn std::error::Error>> {{
        // TODO: Implement command logic
        println!("Command executed: {name}");
        Ok(())
    }}
}}
"#,
        struct_name = struct_name,
        name = name,
    );

    let file_path = commands_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Command '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, command_content).unwrap();

    // Update mod.rs
    let mod_path = commands_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Command [{}] created at '{}'.",
            struct_name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Register the command in your console kernel.".dimmed()
    );
}

/// Convert PascalCase to snake_case.
fn to_snake_case(name: &str) -> String {
    {
        let mut result = String::new();
        for (i, ch) in name.chars().enumerate() {
            {
                if ch.is_uppercase() {
                    {
                        if i > 0 {
                            {
                                result.push('_');
                            }
                        }
                        result.push(ch.to_ascii_lowercase());
                    }
                } else {
                    {
                        result.push(ch);
                    }
                }
            }
        }
        result
    }
}

fn make_policy(name: &str) {
    let policies_dir = std::path::Path::new("src/policies");
    std::fs::create_dir_all(policies_dir).unwrap();

    let snake_name = to_snake_case(name);

    let resource_name = snake_name.strip_suffix("_policy").unwrap_or(&snake_name);

    // Split the model name from the policy name
    // e.g. "PostPolicy" -> resource "post", policy "PostPolicy"
    let policy_name = name;

    let policy_content = format!(
        r#"use larastvel_core::auth::{{AuthenticatedUser, GateCheck, Policy}};

#[derive(Debug)]
pub struct {name};

impl {name} {{
    /// Register this policy with the given gate.
    ///
    /// Call this in your application's service provider:
    /// ```ignore
    /// gate.register_policy("{resource}", std::sync::Arc::new({name}));
    /// ```
    pub fn register(gate: &larastvel_core::auth::Gate) {{
        gate.register_policy("{resource}", std::sync::Arc::new({name}));
    }}
}}

impl Policy for {name} {{
    fn resource(&self) -> &str {{
        "{resource}"
    }}

    fn check(
        &self,
        _user: &AuthenticatedUser,
        ability: &str,
        _args: &[String],
    ) -> Option<GateCheck> {{
        match ability {{
            "view-{resource}" | "create-{resource}" | "update-{resource}" | "delete-{resource}" => {{
                Some(GateCheck::Allowed)
            }}
            _ => None,
        }}
    }}
}}
"#,
        name = policy_name,
        resource = resource_name,
    );

    let file_path = policies_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Policy '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, policy_content).unwrap();

    // Update mod.rs
    let mod_path = policies_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Policy [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Implement check logic in the `check` method for each ability.".dimmed()
    );
    println!(
        "{}",
        "  Register the policy in your AuthServiceProvider: PostPolicy::register(&gate);".dimmed()
    );
}

fn make_model(name: &str) {
    let models_dir = std::path::Path::new("src/models");
    std::fs::create_dir_all(models_dir).unwrap();

    let snake_name = to_snake_case(name);

    let model_content = format!(
        r#"use larastvel_core::sea_orm;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "{table}")]
pub struct Model {{
    #[sea_orm(primary_key)]
    pub id: i32,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {{}}

impl ActiveModelBehavior for ActiveModel {{}}

pub struct {name};

impl larastvel_core::models::DbModel for {name} {{
        type Entity = Entity;
    }}
"#,
        name = name,
        table = snake_name
    );

    let file_path = models_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Model '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, model_content).unwrap();

    // Update mod.rs
    let mod_path = models_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Model [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
}

fn make_controller(name: &str) {
    let controllers_dir = std::path::Path::new("src/controllers");
    std::fs::create_dir_all(controllers_dir).unwrap();

    let snake_name = to_snake_case(name);

    let controller_content = format!(
        r#"use larastvel_core::axum::response::{{IntoResponse, Json, Response}};
use larastvel_core::Resource;
use serde_json::json;

#[derive(Resource)]
pub struct {name};

impl larastvel_core::routing::ResourceController for {name} {{
    const RESOURCE_NAME: &'static str = "{resource_name}";
}}
"#,
        name = name,
        resource_name = snake_name
    );

    let file_path = controllers_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Controller '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, controller_content).unwrap();

    // Update mod.rs
    let mod_path = controllers_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Controller [{}] created at '{}'.",
            name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Override resource methods in the `impl ResourceController` block to add custom logic."
            .dimmed()
    );
    println!(
        "{}",
        "  Register routes: MyController::register_routes(&registrar);".dimmed()
    );
}

fn make_seeder(name: &str) {
    let seeders_dir = std::path::Path::new("src/database/seeders");
    std::fs::create_dir_all(seeders_dir).unwrap();

    let snake_name = to_snake_case(name);

    let seeder_content = format!(
        r#"use larastvel_core::database::DatabaseSeeder;
use larastvel_core::sea_orm::DbConn;

pub struct {name};

impl {name} {{
    pub async fn run(conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {{
        // TODO: Insert seed data here
        // Example:
        // use sea_orm::{{ActiveModelTrait, Set}};
        // use crate::models::user::ActiveModel as UserActiveModel;
        //
        // let user = UserActiveModel {{
        //     name: Set("Admin".to_string()),
        //     email: Set("admin@example.com".to_string()),
        //     password: Set(larastvel_core::hash::make("password")?),
        //     ..Default::default()
        // }};
        // user.insert(conn).await?;

        tracing::info!("Seeded: {name}");
        Ok(())
    }}
}}
"#,
        name = name,
    );

    let file_path = seeders_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Seeder '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, seeder_content).unwrap();

    // Update mod.rs
    let mod_path = seeders_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Seeder [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Implement the `run` method to insert seed data.".dimmed()
    );
}

fn run_seed_command() {
    println!("{}", "Running database seeders...".green().bold());
    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--seed"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("{}", "✓ Database seeding completed.".green());
        }
        _ => {
            eprintln!(
                "{}",
                "Seeding failed. Make sure you're in the project root directory.".red()
            );
            eprintln!("{}", "You can also run: cargo run -- --seed".dimmed());
        }
    }
}

fn run_migrate_command(subcommand: &str) {
    println!("{}", format!("Running '{}'...", subcommand).green().bold());
    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--migrate", subcommand])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("{}", "✓ Migration completed successfully.".green());
        }
        _ => {
            eprintln!(
                "{}",
                "Migration failed. Make sure you're in the project root directory.".red()
            );
            eprintln!(
                "{}",
                "You can also run: cargo run -- --migrate <command>".dimmed()
            );
        }
    }
}

fn make_migration(name: &str) {
    let migrations_dir = std::path::Path::new("src/database/migrations");
    std::fs::create_dir_all(migrations_dir).unwrap();

    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();
    let version = format!("m{}", secs);
    let snake_name = to_snake_case(name);
    let file_name = format!("{}_{}", version, snake_name);

    let migration_content = r#"use larastvel_core::sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        todo!("Implement up migration");
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        todo!("Implement down migration");
    }
}
"#
    .to_string();

    let file_path = migrations_dir.join(format!("{}.rs", file_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Migration '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, migration_content).unwrap();

    // Register in mod.rs
    let mod_path = migrations_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", file_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Migration [{}] created at '{}'.",
            name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Don't forget to register the migration in src/database/migrator.rs".dimmed()
    );
}

fn make_mail(name: &str) {
    let mails_dir = std::path::Path::new("src/mails");
    std::fs::create_dir_all(mails_dir).unwrap();

    let snake_name = to_snake_case(name);
    let struct_name = if name.ends_with("Mail") {
        name.to_string()
    } else {
        format!("{}Mail", name)
    };

    let mail_content = format!(
        r#"use larastvel_core::mail::{{Mailable, Mailer}};

/// Mailable: {name}
#[derive(Debug)]
pub struct {struct_name} {{
    // TODO: Add email data fields
}}

impl {struct_name} {{
    pub fn new() -> Self {{
        Self {{
            // TODO: Initialize with data
        }}
    }}

    pub async fn send(&self, mailer: &dyn Mailer, to: &str) -> Result<(), Box<dyn std::error::Error>> {{
        let mailable = Mailable::html(
            vec![to.to_string()],
            "{name}",
            "<h1>{name}</h1><p>Your message here.</p>",
        )
        .from("noreply@example.com");

        mailer.send(mailable).await?;
        Ok(())
    }}
}}
"#,
        struct_name = struct_name,
        name = struct_name,
    );

    let file_path = mails_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Mail '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, mail_content).unwrap();

    // Update mod.rs
    let mod_path = mails_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Mail [{}] created at '{}'.",
            struct_name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Customize the email template and data fields.".dimmed()
    );
    println!(
        "{}",
        "  Send with: my_mail.send(&mailer, \"user@example.com\").await?;".dimmed()
    );
}

fn config_cache() {
    let cache_dir = std::path::Path::new("bootstrap/cache");
    std::fs::create_dir_all(cache_dir).unwrap();

    let config = larastvel_core::config::Config::load(std::path::Path::new("."));

    let cache_path = cache_dir.join("config.json");
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&cache_path, json).unwrap();

    println!(
        "{}",
        format!(
            "✓ Config cached successfully to '{}'.",
            cache_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Use config:clear to remove the cached file."
            .to_string()
            .dimmed()
    );
}

async fn schedule_list() {
    println!("{}", "Scheduled Tasks".cyan().bold());

    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--schedule:list"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            // Output is handled by the user's app
        }
        _ => {
            eprintln!(
                "{}",
                "Failed to list scheduled tasks. Make sure you're in the project root directory."
                    .red()
            );
            eprintln!(
                "{}",
                "In your application's main.rs, add a --schedule:list argument handler:".dimmed()
            );
            eprintln!(
                "{}",
                "  let events = schedule.events();".to_string().dimmed()
            );
            eprintln!("{}", "  for event in events {".to_string().dimmed());
            eprintln!(
                "{}",
                "    println!(\"  {}  {}\", event.description(), \"schedule expression\");"
                    .to_string()
                    .dimmed()
            );
            eprintln!("{}", "  }".to_string().dimmed());
        }
    }
}

async fn route_cache() {
    println!("{}", "⚡ Caching routes...".green().bold());

    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--route:cache"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!(
                "{}",
                format!("✓ Routes cached to '{}'.", "bootstrap/cache/routes.json")
                    .green()
                    .bold()
            );
            println!(
                "{}",
                "  Use route:clear to remove the cached routes file.".dimmed()
            );
        }
        _ => {
            eprintln!(
                "{}",
                "Failed to cache routes. Make sure you're in the project root directory.".red()
            );
            eprintln!(
                "{}",
                "In your application's main.rs, add a --route:cache argument handler:".dimmed()
            );
            eprintln!(
                "{}",
                "  let registrar = Registrar::new(Arc::new(Mutex::new(AxumRouter::new())), Arc::new(Mutex::new(vec![])));".to_string()
                .dimmed()
            );
            eprintln!("{}", "  routes::web(&registrar);".dimmed());
            eprintln!("{}", "  routes::api(&registrar);".dimmed());
            eprintln!(
                "{}",
                "  let routes_json = serde_json::to_string_pretty(&registrar.list_routes()).unwrap();".to_string()
                .dimmed()
            );
            eprintln!(
                "{}",
                "  std::fs::create_dir_all(\"bootstrap/cache\").unwrap();".dimmed()
            );
            eprintln!(
                "{}",
                "  std::fs::write(\"bootstrap/cache/routes.json\", routes_json).unwrap();".dimmed()
            );
        }
    }
}

fn route_clear() {
    let cache_path = std::path::Path::new("bootstrap/cache/routes.json");
    if cache_path.exists() {
        match std::fs::remove_file(cache_path) {
            Ok(_) => {
                println!(
                    "{}",
                    format!("✓ Cached routes cleared from '{}'.", cache_path.display())
                        .green()
                        .bold()
                );
            }
            Err(e) => {
                eprintln!("{}", format!("Error clearing routes cache: {}", e).red());
            }
        }
    } else {
        println!(
            "{}",
            "No cached routes found. Run 'route:cache' first.".yellow()
        );
    }
}

fn config_clear() {
    let cache_path = std::path::Path::new("bootstrap/cache/config.json");
    if cache_path.exists() {
        match std::fs::remove_file(cache_path) {
            Ok(_) => {
                println!(
                    "{}",
                    format!("✓ Cached config cleared from '{}'.", cache_path.display())
                        .green()
                        .bold()
                );
            }
            Err(e) => {
                eprintln!("{}", format!("Error clearing config cache: {}", e).red());
            }
        }
    } else {
        println!(
            "{}",
            "No cached config found. Run 'config:cache' first.".yellow()
        );
    }
}

fn env_display() {
    println!("{}", "Environment".cyan().bold());
    println!();

    // --- .env file ---
    let env_path = std::path::Path::new(".env");
    println!("{}", ".env".yellow().bold());

    if env_path.exists() {
        if let Ok(content) = std::fs::read_to_string(env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(pos) = line.find('=') {
                    let key = &line[..pos];
                    let value = &line[pos + 1..];
                    let display = if key.ends_with("_KEY")
                        || key.ends_with("_SECRET")
                        || key.ends_with("_PASSWORD")
                        || key == "APP_KEY"
                        || key == "DB_PASSWORD"
                    {
                        "******"
                    } else if value.is_empty() {
                        "(empty)"
                    } else {
                        value
                    };
                    println!("  {} = {}", key.cyan(), display);
                }
            }
        } else {
            println!("  {}", "(unable to read .env)".red());
        }
    } else {
        println!("  {}", "(no .env file found)".dimmed());
    }

    println!();

    // --- Config (config.toml) ---
    println!("{}", "Config (config.toml)".yellow().bold());

    let config_path = std::path::Path::new("config.toml");
    if config_path.exists() {
        let config = larastvel_core::config::Config::load(std::path::Path::new("."));
        println!("  {} = {}", "APP_NAME".cyan(), config.app.name);
        println!("  {} = {}", "APP_URL".cyan(), config.app.url);
        println!("  {} = {}", "APP_ENV".cyan(), config.app.env);
        println!("  {} = {}", "APP_DEBUG".cyan(), config.app.debug);
        if config.app.key.is_some() {
            println!("  {} = (masked)", "APP_KEY".cyan());
        } else {
            println!("  {} = (not set)", "APP_KEY".cyan());
        }
        println!();
        println!(
            "  {} = {} ({}://{}:{}/{})",
            "DB_CONNECTION".cyan(),
            config.database.driver,
            config.database.driver,
            config.database.host,
            config.database.port,
            config.database.database,
        );
        println!("  {} = {}", "DB_USERNAME".cyan(), config.database.username);
        println!("  {} = ******", "DB_PASSWORD".cyan());
        println!();
        println!("  {} = {}", "LOG_LEVEL".cyan(), config.logging.level);
        println!("  {} = {}", "LOG_FORMAT".cyan(), config.logging.format);
        println!();
        println!("  {} = {}", "VIEW_ENGINE".cyan(), config.view.engine);
        println!(
            "  {} = {}",
            "VIEW_PATHS".cyan(),
            config.view.paths.join(", ")
        );
    } else {
        println!("  {}", "(no config.toml found)".dimmed());
    }
}

fn maintenance_down(message: Option<String>, retry: Option<u64>, force: bool) {
    let down_file = std::path::Path::new("storage/framework/down");

    if down_file.exists() && !force {
        eprintln!(
            "{}",
            format!(
                "Application is already in maintenance mode at '{}'.",
                down_file.display()
            )
            .yellow()
        );
        eprintln!(
            "{}",
            "  Use --force to overwrite the existing down file.".dimmed()
        );
        return;
    }

    std::fs::create_dir_all(down_file.parent().unwrap()).unwrap();

    let payload = serde_json::json!({
        "message": message.unwrap_or_else(|| "Application is in maintenance mode.".to_string()),
        "retry": retry,
        "time": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });

    let content = serde_json::to_string_pretty(&payload).unwrap();
    std::fs::write(down_file, content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Application is now in maintenance mode at '{}'.",
            down_file.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Use 'up' to bring the application back online.".dimmed()
    );
}

fn maintenance_up() {
    let down_file = std::path::Path::new("storage/framework/down");

    if !down_file.exists() {
        println!("{}", "Application is not in maintenance mode.".yellow());
        return;
    }

    match std::fs::remove_file(down_file) {
        Ok(_) => {
            println!(
                "{}",
                format!(
                    "✓ Application is now live. Maintenance mode cleared from '{}'.",
                    down_file.display()
                )
                .green()
                .bold()
            );
        }
        Err(e) => {
            eprintln!(
                "{}",
                format!("Error clearing maintenance mode: {}", e).red()
            );
        }
    }
}

fn storage_link() {
    let target = std::path::Path::new("storage/app/public");
    let link = std::path::Path::new("public/storage");

    // Create the target directory if it doesn't exist
    std::fs::create_dir_all(target).unwrap();

    // Remove existing link/file/dir if present (use symlink_metadata
    // to catch broken symlinks too, since exists() follows targets)
    if link.symlink_metadata().is_ok() {
        if let Err(e) = std::fs::remove_file(link) {
            // Try rmdir if it's a directory
            if std::fs::remove_dir(link).is_err() {
                eprintln!(
                    "{}",
                    format!(
                        "Error: Could not remove existing '{}': {}",
                        link.display(),
                        e
                    )
                    .red()
                );
                return;
            }
        }
    }

    #[cfg(unix)]
    {
        match std::os::unix::fs::symlink(target, link) {
            Ok(_) => {
                println!(
                    "{}",
                    format!(
                        "✓ Symlink created: [{}] -> [{}]",
                        link.display(),
                        target.display()
                    )
                    .green()
                    .bold()
                );
            }
            Err(e) => {
                eprintln!("{}", format!("Error creating symlink: {}", e).red());
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Fallback: copy instead of symlink on non-Unix platforms
        eprintln!(
            "{}",
            "Warning: Symlinks not supported on this platform. Using copy instead.".yellow()
        );
        println!(
            "{}",
            format!(
                "✓ Directory created: [{}] -> [{}]",
                link.display(),
                target.display()
            )
            .green()
            .bold()
        );
        println!(
            "{}",
            "  Copy files manually or use a storage driver that supports your platform.".dimmed()
        );
    }
}

fn create_notifications_table() {
    let migrations_dir = std::path::Path::new("database/migrations");
    std::fs::create_dir_all(migrations_dir).unwrap();

    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();
    let version = format!("m{}", secs);
    let file_name = format!("{}_create_notifications_table", version);

    let migration_content = r#"use larastvel_core::sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Notifications::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Notifications::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Notifications::Type).string().not_null())
                    .col(
                        ColumnDef::new(Notifications::NotifiableType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::NotifiableId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Notifications::Data).text().not_null())
                    .col(                        ColumnDef::new(Notifications::ReadAt).timestamp().null())
                    .col(
                        ColumnDef::new(Notifications::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::UpdatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add index on (notifiable_type, notifiable_id) for polymorphic lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_notifications_notifiable")
                    .table(Notifications::Table)
                    .col(Notifications::NotifiableType)
                    .col(Notifications::NotifiableId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Notifications::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Notifications {
    Table,
    Id,
    Type,
    NotifiableType,
    NotifiableId,
    Data,
    ReadAt,
    CreatedAt,
    UpdatedAt,
}
"#
    .to_string();

    let file_path = migrations_dir.join(format!("{}.rs", file_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Migration '{}' already exists '{}'.",
                "create_notifications_table",
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, migration_content).unwrap();

    // Register in mod.rs
    let mod_path = migrations_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", file_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Notifications table migration created at '{}'.",
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Don't forget to register the migration in your migrator.".dimmed()
    );
}

async fn queue_work(once: bool, queue: &str, sleep: u64) {
    println!(
        "{}",
        format!(
            "⚡ Queue worker starting [queue: {}, sleep: {}s]...",
            queue, sleep
        )
        .green()
        .bold()
    );
    println!("{}", "  Press Ctrl+C to stop.".dimmed());

    let status = std::process::Command::new("cargo")
        .args([
            "run",
            "--",
            &format!("--queue:work={}", queue),
            &format!("--queue-sleep={}", sleep),
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            if once {
                println!("{}", "✓ Queue worker completed.".green());
            }
        }
        _ => {
            eprintln!(
                "{}",
                "Failed to start queue worker. Make sure you're in the project root directory."
                    .red()
            );
            eprintln!(
                "{}",
                "In your application's main.rs, add a --queue:work argument handler:".dimmed()
            );
            eprintln!(
                "{}",
                "  let mut db = DatabaseManager::new(&app.config());"
                    .to_string()
                    .dimmed()
            );
            eprintln!(
                "{}",
                "  let conn = db.connect().await?;".to_string().dimmed()
            );
            eprintln!(
                "{}",
                "  let queue = DatabaseQueue::new(\"default\", conn, resolver);"
                    .to_string()
                    .dimmed()
            );
            eprintln!(
                "{}",
                "  let worker = QueueWorker::new(Arc::new(queue));"
                    .to_string()
                    .dimmed()
            );
            eprintln!("{}", "  worker.work().await;".dimmed());
        }
    }

    if once {
        println!(
            "{}",
            "  Pass --once to process a single job, or omit it to keep the worker running."
                .dimmed()
        );
    } else {
        println!(
            "{}",
            "  Use --once to process a single job, or omit it to keep the worker running.".dimmed()
        );
    }
}

async fn run_schedule_command() {
    println!("{}", "⚡ Running scheduled tasks...".green().bold());
    println!("{}", "  Press Ctrl+C to stop.".dimmed());

    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--schedule:run"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("{}", "✓ Scheduled tasks completed.".green());
        }
        _ => {
            eprintln!(
                "{}",
                "Failed to run scheduled tasks. Make sure you're in the project root directory."
                    .red()
            );
            eprintln!(
                "{}",
                "In your application's main.rs, add a --schedule:run argument handler:".dimmed()
            );
            eprintln!("{}", "  let schedule = Schedule::new();".dimmed());
            eprintln!(
                "{}",
                "  schedule.call(\"* * * * *\", \"log stats\", || async { Ok(()) });".dimmed()
            );
            eprintln!(
                "{}",
                "  let manager = ScheduleManager::new(schedule);".dimmed()
            );
            eprintln!("{}", "  manager.run_due_async().await;".dimmed());
        }
    }
}
