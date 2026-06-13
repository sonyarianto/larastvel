extern crate self as larastvel_core;

pub mod auth;
pub mod config;
pub mod console;
pub mod database;
pub mod encryption;
pub mod foundation;
pub mod hash;
pub mod http;
pub mod logging;
pub mod middleware;
pub mod models;
pub mod routing;
pub mod support;
pub mod validation;
pub mod view;

pub use axum;
pub use sea_orm_migration;
pub use serde;
pub use serde_json;
pub use sea_orm;
pub use tokio;

pub use foundation::{Application, Kernel, ServiceProvider};
pub use routing::Registrar;
pub use auth::{Auth, AuthError, AuthenticatedUser, Claims};
pub use database::{DatabaseManager, DatabaseSeeder, Seeder};
pub use config::Config;
pub use console::ConsoleKernel;
pub use http::Request;
pub use larastvel_macros::{Resource, controller, route};
