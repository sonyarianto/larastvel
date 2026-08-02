use colored::*;
use std::path::PathBuf;

pub async fn create_project(name: &str) {
    let path = PathBuf::from(name);
    if path.exists() {
        eprintln!(
            "{}",
            format!("Error: Directory '{}' already exists.", name).red()
        );
        std::process::exit(1);
    }

    std::fs::create_dir_all(path.join("src/database/migrations")).unwrap();
    std::fs::create_dir_all(path.join("src/models")).unwrap();
    std::fs::create_dir_all(path.join("src/routes")).unwrap();
    std::fs::create_dir_all(path.join("resources/views")).unwrap();
    std::fs::create_dir_all(path.join("resources/js")).unwrap();
    std::fs::create_dir_all(path.join("resources/css")).unwrap();
    std::fs::create_dir_all(path.join("public")).unwrap();
    std::fs::create_dir_all(path.join("config")).unwrap();
    std::fs::create_dir_all(path.join("storage/logs")).unwrap();
    std::fs::create_dir_all(path.join("storage/app")).unwrap();
    std::fs::create_dir_all(path.join("tests")).unwrap();

    let main_rs = r#"use larastvel_core::{Application, DatabaseManager, logging};

mod database;
mod models;
mod routes;

#[tokio::main]
async fn main() {
    let app = Application::new(None);
    logging::init(&app.config());

    let db = DatabaseManager::new(&app.config());
    match db.connect().await {
        Ok(conn) => {
            tracing::info!("Database connected successfully");
            let _ = larastvel_core::models::set_global_database(conn);
        }
        Err(e) => tracing::warn!("Database connection failed: {} (app will still run)", e),
    }

    if let Err(e) = db.migrate::<database::migrator::Migrator>().await {
        tracing::warn!("Migration failed: {} (app will still run)", e);
    }

    let app = app.with_database(db);

    let router = app.router();
    routes::web::web(&router);
    routes::api::api(&router);

    println!("⚡ starting up...");
    app.run().await;
}
"#;

    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.2.0"
edition = "2021"

[dependencies]
larastvel-core = "0.2"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tracing = "0.1"
sea-orm-migration = "1"
"#,
        name = name
    );

    let config_app = format!(
        r#"name = "{name}"
url = "http://localhost:8080"
env = "local"
debug = true
"#,
        name = name
    );

    let config_database = r#"driver = "sqlite"
host = "127.0.0.1"
port = 3306
database = "larastvel.db"
username = "root"
password = ""
"#;

    let config_logging = r#"level = "debug"
format = "text"
"#;

    let config_view = r#"engine = "tera"
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

    let routes_mod = r#"pub mod web;
pub mod api;
"#;

    let routes_web = r#"use larastvel_core::routing::Registrar;

pub fn web(router: &Registrar) {
    router.get("/", || async {
        larastvel_core::axum::response::Html("<h1>Welcome to Larastvel</h1>")
    });
}
"#;

    let routes_api = r#"use larastvel_core::routing::Registrar;

pub fn api(router: &Registrar) {
    router.group("/api", |r| {
        r.get("/health", || async {
            larastvel_core::axum::response::Json(serde_json::json!({
                "status": "ok",
                "framework": "Larastvel",
            }))
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

    let models_mod = "pub mod user;\n";
    let user_model = r#"use larastvel_core::table;

#[table("users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub password: String,
    pub email_verified_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}
"#;

    let database_mod = r#"pub mod migrator;
pub mod migrations;
"#;

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

    let database_migrations_mod = r#"pub mod m20220101_000001_create_users_table;
"#;

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
                    .col(ColumnDef::new(Users::EmailVerifiedAt).date_time().null())
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
    EmailVerifiedAt,
    CreatedAt,
    UpdatedAt,
}
"#;

    std::fs::write(path.join("Cargo.toml"), cargo_toml).unwrap();
    std::fs::write(path.join("src/main.rs"), main_rs).unwrap();
    std::fs::write(path.join("src/models/mod.rs"), models_mod).unwrap();
    std::fs::write(path.join("src/models/user.rs"), user_model).unwrap();
    std::fs::write(path.join("src/routes/mod.rs"), routes_mod).unwrap();
    std::fs::write(path.join("src/routes/web.rs"), routes_web).unwrap();
    std::fs::write(path.join("src/routes/api.rs"), routes_api).unwrap();
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
    std::fs::write(path.join("config/app.toml"), config_app).unwrap();
    std::fs::write(path.join("config/database.toml"), config_database).unwrap();
    std::fs::write(path.join("config/logging.toml"), config_logging).unwrap();
    std::fs::write(path.join("config/view.toml"), config_view).unwrap();
    std::fs::write(path.join("vite.config.js"), vite_config).unwrap();
    std::fs::write(path.join("package.json"), package_json).unwrap();
    std::fs::write(path.join("resources/views/welcome.html"), welcome_view).unwrap();
    std::fs::write(path.join("resources/css/app.css"), app_css).unwrap();
    std::fs::write(path.join("resources/js/app.js"), app_js).unwrap();
    std::fs::write(path.join("resources/js/bootstrap.js"), bootstrap_js).unwrap();
    std::fs::write(path.join("tailwind.config.js"), tailwind_config).unwrap();
    std::fs::write(path.join("postcss.config.js"), postcss_config).unwrap();
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
