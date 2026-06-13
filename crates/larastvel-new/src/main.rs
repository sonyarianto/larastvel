use std::path::PathBuf;

use clap::Parser;
use colored::*;

#[derive(Parser)]
#[command(name = "larastvel-new", about = "Create a new Larastvel application")]
struct Cli {
    name: String,

    #[arg(short, long)]
    path: Option<String>,

    #[arg(long)]
    vite: bool,

    #[arg(long, default_value = "sqlite")]
    database: String,

    #[arg(short, long)]
    force: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let project_path = match &cli.path {
        Some(p) => PathBuf::from(p).join(&cli.name),
        None => PathBuf::from(&cli.name),
    };

    if project_path.exists() {
        if cli.force {
            std::fs::remove_dir_all(&project_path).unwrap();
        } else {
            eprintln!(
                "{}",
                format!(
                    "Error: Directory '{}' already exists.",
                    project_path.display()
                )
                .red()
            );
            std::process::exit(1);
        }
    }

    println!(
        "{}",
        format!("Creating Larastvel application: {}...", cli.name)
            .green()
            .bold()
    );

    create_project(&project_path, &cli.name, &cli.database, cli.vite);
}

fn create_project(path: &PathBuf, name: &str, database: &str, with_vite: bool) {
    let dirs = [
        "src/database/migrations",
        "src/models",
        "src/routes",
        "resources/views",
        "resources/js",
        "resources/css",
        "public",
        "routes",
        "config",
        "storage/logs",
        "storage/app",
        "tests",
    ];

    for dir in &dirs {
        std::fs::create_dir_all(path.join(dir)).unwrap();
    }

    // Cargo.toml
    let cargo = format!(
        r#"[package]
name = "{}"
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
        name
    );

    // src/main.rs
    let main_rs = format!(
        r#"use larastvel_core::{{Application, Config, DatabaseManager, logging}};
use larastvel_core::routing::Registrar;

mod database;
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

    if let Err(e) = db.migrate::<database::migrator::Migrator>().await {{
        tracing::warn!("Migration failed: {{}} (app will still run)", e);
    }}

    let app = app.with_database(db);

    let router = app.router();
    routes::web(&router);
    routes::api(&router);

    println!("⚡ {name} starting up...");
    app.run().await;
}}
"#,
        name = name
    );

    // models/mod.rs
    let models_mod = r#"pub mod user;
"#;

    // models/user.rs
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

    // routes/mod.rs
    let routes_mod = r#"pub mod web;
pub mod api;
"#;

    // routes/web.rs
    let routes_web = r#"use larastvel_core::routing::Registrar;

pub fn web(router: &Registrar) {
    router.get("/", || async {
        axum::response::Html("<h1>Welcome to Larastvel</h1>")
    });
}
"#;

    // routes/api.rs
    let routes_api = r#"use larastvel_core::routing::Registrar;

pub fn api(router: &Registrar) {
    router.group("/api", |r| {
        r.get("/health", || async {
            axum::response::Json(serde_json::json!({"status": "ok", "framework": "Larastvel"}))
        });
    });
}
"#;

    // config.toml
    let config_toml = format!(
        r#"[app]
name = "{}"
url = "http://localhost:8080"
env = "local"
debug = true

[database]
driver = "{}"
host = "127.0.0.1"
port = 3306
database = "{}.db"
username = "root"
password = ""

[logging]
level = "debug"
format = "text"

[view]
engine = "tera"
paths = ["resources/views"]
"#,
        name, database, name
    );

    // .env
    let env = format!(
        r#"APP_NAME={}
APP_ENV=local
APP_KEY=
APP_DEBUG=true
APP_URL=http://localhost:8080

DB_CONNECTION={}
DB_HOST=127.0.0.1
DB_PORT=3306
DB_DATABASE={}
DB_USERNAME=root
DB_PASSWORD=
"#,
        name, database, name
    );

    // database/mod.rs
    let database_mod = r#"pub mod migrator;
pub mod migrations;
"#;

    // database/migrator.rs
    let database_migrator = r#"use larastvel_core::sea_orm_migration::prelude::*;

use super::migrations;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(migrations::m20220101_000001_create_users_table::Migration)]
    }
}
"#;

    // database/migrations/mod.rs
    let database_migrations_mod = r#"pub mod m20220101_000001_create_users_table;
"#;

    // database/migrations/m20220101_000001_create_users_table.rs
    let database_users_migration = r#"use larastvel_core::sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Users::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Users::Name).string().not_null())
                    .col(ColumnDef::new(Users::Email).string().not_null())
                    .col(ColumnDef::new(Users::Password).string().not_null())
                    .col(ColumnDef::new(Users::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(Users::UpdatedAt).date_time().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
    Name,
    Email,
    Password,
    CreatedAt,
    UpdatedAt,
}
"#;

    // Write files
    std::fs::write(path.join("Cargo.toml"), cargo).unwrap();
    std::fs::write(path.join("src/main.rs"), main_rs).unwrap();
    std::fs::write(path.join("src/models/mod.rs"), models_mod).unwrap();
    std::fs::write(path.join("src/models/user.rs"), user_model).unwrap();
    std::fs::write(path.join("src/database/mod.rs"), database_mod).unwrap();
    std::fs::write(path.join("src/database/migrator.rs"), database_migrator).unwrap();
    std::fs::write(
        path.join("src/database/migrations/mod.rs"),
        database_migrations_mod,
    )
    .unwrap();
    std::fs::write(
        path.join("src/database/migrations/m20220101_000001_create_users_table.rs"),
        database_users_migration,
    )
    .unwrap();
    std::fs::write(path.join("src/routes/mod.rs"), routes_mod).unwrap();
    std::fs::write(path.join("src/routes/web.rs"), routes_web).unwrap();
    std::fs::write(path.join("src/routes/api.rs"), routes_api).unwrap();
    std::fs::write(path.join("config.toml"), config_toml).unwrap();
    std::fs::write(path.join(".env"), env).unwrap();

    if with_vite {
        setup_vite(path);
    }

    println!();
    println!("{}", "✓ Application created successfully!".green().bold());
    println!();
    println!("{}", "Next steps:".cyan());
    println!("  cd {}", path.file_name().unwrap().to_string_lossy());
    println!("  cargo build");
    if with_vite {
        println!("  npm install && npm run dev");
    }
    println!("  larastvel serve");
}

fn setup_vite(path: &PathBuf) {
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
        "@vitejs/plugin-vue": "^5.0.0",
        "vue": "^3.5.0",
        "autoprefixer": "^10.4.0",
        "postcss": "^8.4.0",
        "tailwindcss": "^3.4.0",
        "axios": "^1.7.0"
    }
}
"#;

    let vite_config = r#"import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
    plugins: [vue()],
    server: {
        port: 5173,
        hmr: { host: 'localhost' },
    },
    build: {
        outDir: 'public/build',
        manifest: true,
    },
});
"#;

    let tailwind = r#"/** @type {import('tailwindcss').Config} */
export default {
    content: [
        './resources/**/*.{html,vue,js,ts,jsx,tsx}',
    ],
    theme: { extend: {} },
    plugins: [],
};
"#;

    let postcss = r#"export default {
    plugins: {
        tailwindcss: {},
        autoprefixer: {},
    },
};
"#;

    let app_css = r#"@tailwind base;
@tailwind components;
@tailwind utilities;
"#;

    let app_js = r#"import { createApp } from 'vue';
import App from './App.vue';
import './bootstrap';
import './css/app.css';

createApp(App).mount('#app');
"#;

    let app_vue = r#"<template>
    <div class="min-h-screen bg-gray-100 flex items-center justify-center">
        <div class="text-center">
            <h1 class="text-4xl font-bold text-gray-800">Larastvel</h1>
            <p class="mt-4 text-gray-600">Welcome to your new Larastvel application</p>
        </div>
    </div>
</template>

<script>
export default {
    name: 'App',
}
</script>
"#;

    let bootstrap_js = r#"import axios from 'axios';
window.axios = axios;
window.axios.defaults.headers.common['X-Requested-With'] = 'XMLHttpRequest';
"#;

    let welcome_html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ title }}</title>
    <link rel="stylesheet" href="/css/app.css">
    @vite('resources/js/app.js')
</head>
<body>
    <div id="app"></div>
</body>
</html>
"#;

    std::fs::write(path.join("package.json"), package_json).unwrap();
    std::fs::write(path.join("vite.config.js"), vite_config).unwrap();
    std::fs::write(path.join("tailwind.config.js"), tailwind).unwrap();
    std::fs::write(path.join("postcss.config.js"), postcss).unwrap();
    std::fs::write(path.join("resources/css/app.css"), app_css).unwrap();
    std::fs::write(path.join("resources/js/app.js"), app_js).unwrap();
    std::fs::write(path.join("resources/js/App.vue"), app_vue).unwrap();
    std::fs::write(path.join("resources/js/bootstrap.js"), bootstrap_js).unwrap();
    std::fs::write(path.join("resources/views/welcome.html"), welcome_html).unwrap();
}
