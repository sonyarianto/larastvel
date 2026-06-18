use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub app: AppConfig,
    #[serde(default)]
    pub broadcasting: BroadcastingConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub password_reset: PasswordResetConfig,
    #[serde(default)]
    pub view: ViewConfig,
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_app_name")]
    pub name: String,
    #[serde(default = "default_app_url")]
    pub url: String,
    #[serde(default = "default_app_env")]
    pub env: String,
    #[serde(default = "default_app_debug")]
    pub debug: bool,
    #[serde(default)]
    pub key: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: default_app_name(),
            url: default_app_url(),
            env: default_app_env(),
            debug: default_app_debug(),
            key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_driver")]
    pub driver: String,
    #[serde(default = "default_db_host")]
    pub host: String,
    #[serde(default = "default_db_port")]
    pub port: u16,
    #[serde(default = "default_db_database")]
    pub database: String,
    #[serde(default = "default_db_username")]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            driver: default_db_driver(),
            host: default_db_host(),
            port: default_db_port(),
            database: default_db_database(),
            username: default_db_username(),
            password: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    #[serde(default = "default_view_engine")]
    pub engine: String,
    #[serde(default = "default_view_paths")]
    pub paths: Vec<String>,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            engine: default_view_engine(),
            paths: default_view_paths(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastingConfig {
    #[serde(default = "default_broadcast_default")]
    pub default: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub secret: String,
    #[serde(default = "default_broadcast_cluster")]
    pub cluster: String,
    #[serde(default = "default_broadcast_encrypted")]
    pub encrypted: bool,
}

impl Default for BroadcastingConfig {
    fn default() -> Self {
        Self {
            default: default_broadcast_default(),
            app_id: String::new(),
            key: String::new(),
            secret: String::new(),
            cluster: default_broadcast_cluster(),
            encrypted: default_broadcast_encrypted(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetConfig {
    #[serde(default = "default_reset_table")]
    pub table: String,
    #[serde(default = "default_reset_expire")]
    pub expire_seconds: u64,
    #[serde(default = "default_reset_throttle")]
    pub throttle_seconds: u64,
}

impl Default for PasswordResetConfig {
    fn default() -> Self {
        Self {
            table: default_reset_table(),
            expire_seconds: default_reset_expire(),
            throttle_seconds: default_reset_throttle(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_default")]
    pub default: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default = "default_cache_table")]
    pub table: String,
    #[serde(default = "default_cache_file_path")]
    pub file_path: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default: default_cache_default(),
            prefix: String::new(),
            table: default_cache_table(),
            file_path: default_cache_file_path(),
        }
    }
}

// --- Default value helpers ---

fn default_app_name() -> String {
    "larastvel".to_string()
}
fn default_app_url() -> String {
    "http://localhost:8080".to_string()
}
fn default_app_env() -> String {
    "local".to_string()
}
fn default_app_debug() -> bool {
    true
}

fn default_db_driver() -> String {
    "sqlite".to_string()
}
fn default_db_host() -> String {
    "127.0.0.1".to_string()
}
fn default_db_port() -> u16 {
    3306
}
fn default_db_database() -> String {
    "larastvel".to_string()
}
fn default_db_username() -> String {
    "root".to_string()
}

fn default_log_level() -> String {
    "debug".to_string()
}
fn default_log_format() -> String {
    "text".to_string()
}

fn default_view_engine() -> String {
    "tera".to_string()
}
fn default_view_paths() -> Vec<String> {
    vec!["resources/views".to_string()]
}

fn default_broadcast_default() -> String {
    "log".to_string()
}
fn default_broadcast_cluster() -> String {
    "mt1".to_string()
}
fn default_broadcast_encrypted() -> bool {
    true
}

fn default_reset_table() -> String {
    "password_reset_tokens".to_string()
}
fn default_reset_expire() -> u64 {
    3600
}
fn default_reset_throttle() -> u64 {
    60
}

fn default_cache_default() -> String {
    "array".to_string()
}
fn default_cache_table() -> String {
    "cache".to_string()
}
fn default_cache_file_path() -> String {
    "storage/framework/cache/data".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app: AppConfig {
                name: default_app_name(),
                url: default_app_url(),
                env: default_app_env(),
                debug: default_app_debug(),
                key: None,
            },
            database: DatabaseConfig {
                driver: default_db_driver(),
                host: default_db_host(),
                port: default_db_port(),
                database: default_db_database(),
                username: default_db_username(),
                password: String::new(),
            },
            logging: LoggingConfig {
                level: default_log_level(),
                format: default_log_format(),
            },
            view: ViewConfig {
                engine: default_view_engine(),
                paths: default_view_paths(),
            },
            broadcasting: BroadcastingConfig {
                default: default_broadcast_default(),
                app_id: String::new(),
                key: String::new(),
                secret: String::new(),
                cluster: default_broadcast_cluster(),
                encrypted: default_broadcast_encrypted(),
            },
            cache: CacheConfig {
                default: default_cache_default(),
                prefix: String::new(),
                table: default_cache_table(),
                file_path: default_cache_file_path(),
            },
            password_reset: PasswordResetConfig {
                table: default_reset_table(),
                expire_seconds: default_reset_expire(),
                throttle_seconds: default_reset_throttle(),
            },
            extra: HashMap::new(),
        }
    }
}

impl Config {
    /// Load configuration from `config/` directory (preferred) or legacy `config.toml`.
    ///
    /// Looks for a `config/` directory at `base_path`. Each `.toml` file within
    /// becomes a config section keyed by its filename stem (e.g. `app.toml` → `[app]`).
    /// Missing files fall back to their `Default` defaults.
    ///
    /// If no `config/` directory exists, falls back to the legacy `config.toml`
    /// single-file format for backward compatibility.
    pub fn load(base_path: &Path) -> Self {
        let config_dir = base_path.join("config");
        if config_dir.is_dir() {
            let mut merged = String::new();
            let mut entries: Vec<_> = std::fs::read_dir(&config_dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
                .collect();
            entries.sort_by_key(|e| e.file_name());

            for entry in &entries {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    let path = entry.path();
                    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                    merged.push_str(&format!("[{}]\n{}\n", stem, content));
                }
            }

            if !merged.is_empty() {
                if let Ok(config) = toml::from_str(&merged) {
                    return config;
                }
            }
        }

        // Fallback: single config.toml for backward compatibility
        let config_path = base_path.join("config.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path).unwrap_or_default();
            return toml::from_str(&content).unwrap_or_default();
        }

        Config::default()
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

    #[test]
    fn test_config_load_from_dir() {
        let dir = std::env::temp_dir().join("larastvel_config_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();

        std::fs::write(
            dir.join("config").join("app.toml"),
            r#"name = "TestApp"
url = "http://test:8080"
env = "testing"
debug = false
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("config").join("database.toml"),
            r#"driver = "postgres"
host = "pg.example.com"
port = 5432
database = "testdb"
username = "testuser"
password = "secret"
"#,
        )
        .unwrap();

        let config = Config::load(&dir);
        assert_eq!(config.app.name, "TestApp");
        assert_eq!(config.app.url, "http://test:8080");
        assert_eq!(config.app.env, "testing");
        assert!(!config.app.debug);
        assert_eq!(config.database.driver, "postgres");
        assert_eq!(config.database.host, "pg.example.com");
        assert_eq!(config.database.port, 5432);
        assert_eq!(config.database.database, "testdb");
        assert_eq!(config.database.username, "testuser");
        assert_eq!(config.database.password, "secret");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_load_from_dir_falls_back_to_single_file() {
        let dir = std::env::temp_dir().join("larastvel_config_fallback_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("config.toml"),
            r#"[app]
name = "FallbackApp"
url = "http://fallback:8080"
env = "staging"
debug = true

[database]
driver = "mysql"
host = "mysql.example.com"
port = 3306
database = "fallbackdb"
username = "root"
password = ""
"#,
        )
        .unwrap();

        let config = Config::load(&dir);
        assert_eq!(config.app.name, "FallbackApp");
        assert_eq!(config.app.url, "http://fallback:8080");
        assert_eq!(config.app.env, "staging");
        assert!(config.app.debug);
        assert_eq!(config.database.driver, "mysql");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_load_partial_dir_uses_defaults_for_missing_sections() {
        let dir = std::env::temp_dir().join("larastvel_config_partial_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();

        // Only app.toml — database, logging, etc. should use defaults
        std::fs::write(
            dir.join("config").join("app.toml"),
            r#"name = "Partial"
"#,
        )
        .unwrap();

        let config = Config::load(&dir);
        assert_eq!(config.app.name, "Partial");
        assert_eq!(config.app.url, "http://localhost:8080"); // default
        assert_eq!(config.database.driver, "sqlite"); // default
        assert!(config.extra.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
