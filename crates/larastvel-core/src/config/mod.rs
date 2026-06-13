use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub view: ViewConfig,
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub url: String,
    pub env: String,
    pub debug: bool,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub driver: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    pub engine: String,
    pub paths: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app: AppConfig {
                name: "larastvel".to_string(),
                url: "http://localhost:8080".to_string(),
                env: "local".to_string(),
                debug: true,
                key: None,
            },
            database: DatabaseConfig {
                driver: "sqlite".to_string(),
                host: "127.0.0.1".to_string(),
                port: 3306,
                database: "larastvel".to_string(),
                username: "root".to_string(),
                password: "".to_string(),
            },
            logging: LoggingConfig {
                level: "debug".to_string(),
                format: "text".to_string(),
            },
            view: ViewConfig {
                engine: "tera".to_string(),
                paths: vec!["resources/views".to_string()],
            },
            extra: HashMap::new(),
        }
    }
}

impl Config {
    pub fn load(base_path: &Path) -> Self {
        let config_path = base_path.join("config.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Config::default()
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split('.').collect();
        match parts.as_slice() {
            ["app", "name"] => Some(self.app.name.clone()),
            ["app", "url"] => Some(self.app.url.clone()),
            ["app", "env"] => Some(self.app.env.clone()),
            ["app", "debug"] => Some(self.app.debug.to_string()),
            ["database", "driver"] => Some(self.database.driver.clone()),
            ["database", "host"] => Some(self.database.host.clone()),
            ["database", "port"] => Some(self.database.port.to_string()),
            ["database", "database"] => Some(self.database.database.clone()),
            ["database", "username"] => Some(self.database.username.clone()),
            ["database", "password"] => Some(self.database.password.clone()),
            ["logging", "level"] => Some(self.logging.level.clone()),
            ["logging", "format"] => Some(self.logging.format.clone()),
            _ => {
                let full_key = parts.join(".");
                self.extra
                    .get(&full_key)
                    .map(|v| v.to_string())
            }
        }
    }
}
