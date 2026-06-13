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
    RouteList,
    /// Display framework version
    Version,
    /// Run scheduled tasks
    ScheduleRun,
    /// Create a new Larastvel application
    New { name: String },
    /// Generate a new application key
    KeyGenerate,
    /// Run database migrations
    Migrate,
    /// Drop all tables and re-run all migrations
    MigrateFresh,
    /// Rollback the last migration (or N steps)
    MigrateRollback {
        #[arg(short, long)]
        steps: Option<u32>,
    },
    /// Run database seeders
    DbSeed,
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
            None => {
                println!("{}", "Available make targets:".cyan());
                println!("  make:model       Create a new model");
                println!("  make:controller  Create a new controller");
                println!("  make:migration   Create a new migration");
                println!("  make:seeder      Create a new seeder");
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
            println!("  schedule:run     Run scheduled tasks");
            println!("  db:seed          Run database seeders");
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
larastvel-core = {{ path = "../crates/larastvel-core" }}
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tracing = "0.1"
sea-orm-migration = "1"
"#,
        name = name
    );

    let config_toml = r#"[app]
name = "larastvel"
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
"#;

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

fn make_model(name: &str) {
    let models_dir = std::path::Path::new("src/models");
    std::fs::create_dir_all(models_dir).unwrap();

    let snake_name = {
        let mut result = String::new();
        for (i, ch) in name.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
            } else {
                result.push(ch);
            }
        }
        result
    };

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

    let snake_name = {
        let mut result = String::new();
        for (i, ch) in name.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
            } else {
                result.push(ch);
            }
        }
        result
    };

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

    let snake_name = {
        let mut result = String::new();
        for (i, ch) in name.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
            } else {
                result.push(ch);
            }
        }
        result
    };

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
    let snake_name = {
        let mut result = String::new();
        for (i, ch) in name.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
            } else {
                result.push(ch);
            }
        }
        result
    };
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
"#.to_string();

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

async fn run_schedule_command() {
    println!("{}", "Running scheduled tasks...".green().bold());
    println!("{}", "  Register your schedule in your application's kernel.".dimmed());
}
