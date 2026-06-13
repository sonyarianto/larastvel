use std::path::PathBuf;

use clap::{Parser, Subcommand};
use colored::*;

#[derive(Parser)]
#[command(name = "larastvel", about = "Larastvel Framework CLI", version = "0.1.0")]
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
    /// Create a new Larastvel application
    New {
        name: String,
    },
    /// Generate a new application key
    KeyGenerate,
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
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve { port, host }) => {
            let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
            println!(
                "{}",
                format!("⚡ Larastvel development server starting on http://{}:{}", host, port)
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
        Some(Commands::New { name }) => {
            println!("{}", format!("Creating new Larastvel application: {}", name).green().bold());
            create_project(&name).await;
        }
        Some(Commands::KeyGenerate) => {
            let key = uuid::Uuid::new_v4().to_string();
            println!("{}", "Application key generated:".green());
            println!("  APP_KEY={}", key.cyan());
        }
        Some(Commands::Make { target }) => {
            match target {
                Some(MakeTarget::Model { name }) => {
                    println!("{}", format!("Creating model: {}", name).green());
                }
                Some(MakeTarget::Controller { name }) => {
                    println!("{}", format!("Creating controller: {}", name).green());
                }
                Some(MakeTarget::Migration { name }) => {
                    println!("{}", format!("Creating migration: {}", name).green());
                }
                None => {
                    println!("{}", "Available make targets:".cyan());
                    println!("  make:model      Create a new model");
                    println!("  make:controller  Create a new controller");
                    println!("  make:migration   Create a new migration");
                }
            }
        }
        None => {
            println!("{}", "Larastvel Framework CLI".cyan().bold());
            println!("{}", "Usage: larastvel <command>".dimmed());
            println!();
            println!("Available commands:");
            println!("  serve            Start the development server");
            println!("  route:list       List all registered routes");
            println!("  new              Create a new Larastvel application");
            println!("  key:generate     Generate a new application key");
            println!("  make:model       Create a new model");
            println!("  make:controller  Create a new controller");
            println!("  make:migration   Create a new migration");
            println!("  version          Display framework version");
        }
    }
}

async fn start_server(host: &str, port: u16) {
    let addr = format!("{}:{}", host, port);
    println!("  Server running on http://{}", addr.green());

    let app = larastvel_core::axum::Router::new()
        .route("/health", larastvel_core::axum::routing::get(|| async {
            larastvel_core::axum::response::Json(serde_json::json!({
                "status": "ok",
                "framework": "Larastvel",
                "version": env!("CARGO_PKG_VERSION"),
            }))
        }));

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    larastvel_core::axum::serve(listener, app).await.unwrap();
}

async fn create_project(name: &str) {
    let path = PathBuf::from(name);
    if path.exists() {
        eprintln!("{}", format!("Error: Directory '{}' already exists.", name).red());
        std::process::exit(1);
    }

    std::fs::create_dir_all(&path).unwrap();
    std::fs::create_dir_all(path.join("src")).unwrap();
    std::fs::create_dir_all(path.join("resources/views")).unwrap();
    std::fs::create_dir_all(path.join("resources/js")).unwrap();
    std::fs::create_dir_all(path.join("resources/css")).unwrap();
    std::fs::create_dir_all(path.join("public")).unwrap();
    std::fs::create_dir_all(path.join("routes")).unwrap();
    std::fs::create_dir_all(path.join("config")).unwrap();
    std::fs::create_dir_all(path.join("database/migrations")).unwrap();
    std::fs::create_dir_all(path.join("storage/logs")).unwrap();
    std::fs::create_dir_all(path.join("storage/app")).unwrap();

    let main_rs = format!(r#"use larastvel_core::{{Application, Config, DatabaseManager, logging}};

#[tokio::main]
async fn main() {{
    let app = Application::new(None);
    logging::init(&app.config());

    let db = DatabaseManager::new(&app.config());
    db.connect().await.expect("Failed to connect to database");

    let app = app.with_database(db);

    println!("⚡ {name} starting up...");
    app.run().await;
}}
"#, name = name);

    let cargo_toml = format!(r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
larastvel-core = {{ path = "../crates/larastvel-core" }}
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#, name = name);

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

    println!("{}", format!("✓ Application [{}] created successfully!", name).green().bold());
    println!();
    println!("Next steps:");
    println!("  cd {}", name);
    println!("  npm install");
    println!("  larastvel serve");
}
