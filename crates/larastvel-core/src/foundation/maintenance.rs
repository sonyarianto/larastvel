use std::path::{Path, PathBuf};

/// Constant-time byte comparison — the Rust equivalent of PHP's
/// `hash_equals`. Compares every byte of equal-length inputs without
/// short-circuiting, so the wall-clock time does not reveal how many leading
/// bytes matched.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Reader for the `storage/framework/down` payload written by
/// `larastvel down` — the Rust counterpart of Laravel's maintenance mode
/// manager.
#[derive(Debug, Clone)]
pub struct MaintenanceMode {
    down_file: PathBuf,
}

impl MaintenanceMode {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            down_file: base_path.into().join("storage/framework/down"),
        }
    }

    /// The path of the down file.
    pub fn down_file(&self) -> &Path {
        &self.down_file
    }

    /// Whether the application is currently in maintenance mode.
    pub fn is_down(&self) -> bool {
        self.down_file.exists()
    }

    /// The raw JSON payload of the down file, if readable.
    pub fn payload(&self) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(&self.down_file).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// The bypass secret stored in the down file by `larastvel down
    /// --secret=...`, if any.
    pub fn secret(&self) -> Option<String> {
        self.payload()?
            .get("secret")?
            .as_str()
            .map(|s| s.to_string())
    }

    /// Whether `request_path` (without its leading slash) matches the stored
    /// bypass secret. Compared in constant time — Laravel 13.23's
    /// `hash_equals` fix for the maintenance-mode bypass (PR #60896).
    ///
    /// Mirrors `PreventRequestsDuringMaintenance`: the secret is compared
    /// against `$request->path()`.
    pub fn bypass_secret_matches(&self, request_path: &str) -> bool {
        match self.secret() {
            Some(secret) => constant_time_eq(secret.as_bytes(), request_path.as_bytes()),
            None => false,
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir() -> PathBuf {
        std::env::temp_dir().join(format!("larastvel_mm_{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secret2"));
        assert!(!constant_time_eq(b"a", b"aa"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(constant_time_eq(b"", b""));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn test_constant_time_eq_length_mismatch_is_false() {
        assert!(!constant_time_eq(b"very-long-secret", b"short"));
    }

    #[test]
    fn test_maintenance_mode_not_down_by_default() {
        let dir = unique_dir();
        let mm = MaintenanceMode::new(&dir);
        assert!(!mm.is_down());
        assert!(mm.payload().is_none());
        assert!(mm.secret().is_none());
        assert!(!mm.bypass_secret_matches("anything"));
    }

    #[test]
    fn test_maintenance_mode_bypass_secret() {
        let dir = unique_dir();
        std::fs::create_dir_all(dir.join("storage/framework")).unwrap();
        std::fs::write(
            dir.join("storage/framework/down"),
            serde_json::json!({
                "message": "Down for maintenance",
                "secret": "top-secret-bypass",
                "retry": 60,
            })
            .to_string(),
        )
        .unwrap();

        let mm = MaintenanceMode::new(&dir);
        assert!(mm.is_down());
        assert_eq!(mm.secret().unwrap(), "top-secret-bypass");
        assert!(mm.bypass_secret_matches("top-secret-bypass"));
        assert!(!mm.bypass_secret_matches("top-secret-bypasx"));
        assert!(!mm.bypass_secret_matches("top-secret"));
    }

    #[test]
    fn test_maintenance_mode_invalid_payload_has_no_secret() {
        let dir = unique_dir();
        std::fs::create_dir_all(dir.join("storage/framework")).unwrap();
        std::fs::write(dir.join("storage/framework/down"), "not json").unwrap();
        let mm = MaintenanceMode::new(&dir);
        assert!(mm.is_down());
        assert!(mm.payload().is_none());
        assert!(mm.secret().is_none());
        assert!(!mm.bypass_secret_matches("x"));
    }

    #[test]
    fn test_maintenance_mode_no_secret_field() {
        let dir = unique_dir();
        std::fs::create_dir_all(dir.join("storage/framework")).unwrap();
        std::fs::write(
            dir.join("storage/framework/down"),
            serde_json::json!({ "message": "Down" }).to_string(),
        )
        .unwrap();
        let mm = MaintenanceMode::new(&dir);
        assert!(mm.is_down());
        assert!(mm.secret().is_none());
        assert!(!mm.bypass_secret_matches("anything"));
    }
}
