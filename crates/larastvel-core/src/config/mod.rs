use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub broadcasting: BroadcastingConfig,
    pub cache: CacheConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub password_reset: PasswordResetConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastingConfig {
    pub default: String,
    pub app_id: String,
    pub key: String,
    pub secret: String,
    pub cluster: String,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetConfig {
    pub table: String,
    pub expire_seconds: u64,
    pub throttle_seconds: u64,
}

impl Default for PasswordResetConfig {
    fn default() -> Self {
        Self {
            table: "password_reset_tokens".to_string(),
            expire_seconds: 3600,
            throttle_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub default: String,
    pub prefix: String,
    pub table: String,
    pub file_path: String,
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
            broadcasting: BroadcastingConfig {
                default: "log".to_string(),
                app_id: String::new(),
                key: String::new(),
                secret: String::new(),
                cluster: "mt1".to_string(),
                encrypted: true,
            },
            cache: CacheConfig {
                default: "array".to_string(),
                prefix: String::new(),
                table: "cache".to_string(),
                file_path: "storage/framework/cache/data".to_string(),
            },
            password_reset: PasswordResetConfig {
                table: "password_reset_tokens".to_string(),
                expire_seconds: 3600,
                throttle_seconds: 60,
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
                self.extra.get(&full_key).map(|v| v.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.app.name, "larastvel");
        assert_eq!(config.app.url, "http://localhost:8080");
        assert_eq!(config.app.env, "local");
        assert!(config.app.debug);
        assert_eq!(config.database.driver, "sqlite");
        assert_eq!(config.database.port, 3306);
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.view.engine, "tera");
    }

    #[test]
    fn test_config_get_dot_notation() {
        let config = Config::default();
        assert_eq!(config.get("app.name"), Some("larastvel".to_string()));
        assert_eq!(
            config.get("app.url"),
            Some("http://localhost:8080".to_string())
        );
        assert_eq!(config.get("app.env"), Some("local".to_string()));
        assert_eq!(config.get("app.debug"), Some("true".to_string()));
        assert_eq!(config.get("database.driver"), Some("sqlite".to_string()));
        assert_eq!(config.get("database.port"), Some("3306".to_string()));
        assert_eq!(config.get("logging.level"), Some("debug".to_string()));
    }

    #[test]
    fn test_config_get_unknown_key() {
        let config = Config::default();
        assert_eq!(config.get("nonexistent.key"), None);
    }

    #[test]
    fn test_config_get_extra_key() {
        let mut config = Config::default();
        config.extra.insert(
            "custom.key".to_string(),
            toml::Value::String("value".to_string()),
        );
        assert_eq!(config.get("custom.key"), Some("\"value\"".to_string()));
    }

    #[test]
    fn test_config_roundtrip_toml() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.app.name, config.app.name);
        assert_eq!(parsed.database.driver, config.database.driver);
    }

    #[test]
    fn test_config_load_nonexistent_path() {
        let path = std::path::Path::new("/nonexistent/path");
        let config = Config::load(path);
        assert_eq!(config.app.name, "larastvel");
        assert!(config.extra.is_empty());
    }
}
