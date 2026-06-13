extern crate self as larastvel_core;

pub mod auth;
pub mod config;
pub mod console;
pub mod database;
pub mod encryption;
pub mod events;
pub mod foundation;
pub mod hash;
pub mod http;
pub mod logging;
pub mod mail;
pub mod middleware;
pub mod models;
pub mod pagination;
pub mod routing;
pub mod session;
pub mod storage;
pub mod support;
pub mod validation;
pub mod view;

pub use axum;
pub use sea_orm;
pub use sea_orm_migration;
pub use serde;
pub use serde_json;
pub use tokio;

pub use auth::{Auth, AuthError, AuthenticatedUser, Claims};
pub use config::Config;
pub use console::ConsoleKernel;
pub use database::{DatabaseManager, DatabaseSeeder, Seeder};
pub use events::EventService;
pub use foundation::{Application, Kernel, ServiceProvider};
pub use http::Request;
pub use larastvel_macros::{controller, route, Resource};
pub use routing::Registrar;
