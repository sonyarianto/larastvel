pub mod arr;
pub mod collection;
pub mod datetime;
pub mod helpers;
pub mod http_client;
pub mod str;
pub mod vite;

pub use arr::Arr;
pub use collection::{collect as collect_items, Collection};
pub use datetime::{now, today, Dt};
pub use helpers::*;
pub use http_client::{Http, PendingRequest, Response};
pub use str::Str;
pub use vite::Vite;

use std::path::PathBuf;

pub fn base_path(path: Option<&str>) -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match path {
        Some(p) => base.join(p),
        None => base,
    }
}

pub fn storage_path(path: Option<&str>) -> PathBuf {
    base_path(Some("storage")).join(path.unwrap_or(""))
}

pub fn app_path(path: Option<&str>) -> PathBuf {
    base_path(Some("src")).join(path.unwrap_or(""))
}

pub fn config_path(path: Option<&str>) -> PathBuf {
    base_path(Some("config")).join(path.unwrap_or(""))
}

pub fn resource_path(path: Option<&str>) -> PathBuf {
    base_path(Some("resources")).join(path.unwrap_or(""))
}

pub fn public_path(path: Option<&str>) -> PathBuf {
    base_path(Some("public")).join(path.unwrap_or(""))
}
