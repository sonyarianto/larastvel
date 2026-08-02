use std::fs;
use std::path::{Path, PathBuf};

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

pub fn init(config: &Config) {
    let level = config
        .get("logging.level")
        .unwrap_or_else(|| "debug".to_string());
    let filter = EnvFilter::try_new(&level).unwrap_or_else(|_| EnvFilter::new("debug"));

    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true);

    match config
        .get("logging.driver")
        .unwrap_or_else(|| "console".to_string())
        .as_str()
    {
        "monthly" => {
            let path = config
                .get("logging.path")
                .unwrap_or_else(|| "logs/laravel.log".to_string());
            let max_files = config
                .get("logging.max_files")
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_FILES);
            prune_old_monthly_files(Path::new(&path), max_files);
            builder.with_writer(MonthlyWriter::new(path)).init();
        }
        _ => builder.init(),
    }
}

/// Default number of monthly log files to retain (matches Laravel's `monthly`
/// channel).
pub const DEFAULT_MAX_FILES: usize = 3;

/// A [`MakeWriter`] that appends to `laravel-YYYY-MM.log`, rotating once per
/// calendar month — the Rust equivalent of Laravel's `monthly` log driver.
///
/// Given the configured path `logs/laravel.log`, the writer appends to
/// `logs/laravel-2026-08.log` throughout August 2026, then creates
/// `logs/laravel-2026-09.log` in September.
#[derive(Debug, Clone)]
pub struct MonthlyWriter {
    path: PathBuf,
}

impl MonthlyWriter {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The configured base path (e.g. `logs/laravel.log`).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Resolve the log file active at `now` for this writer.
    pub fn current_file(&self, now: chrono::DateTime<chrono::Utc>) -> PathBuf {
        monthly_path(&self.path, &now.format("%Y-%m").to_string())
    }
}

impl<'a> MakeWriter<'a> for MonthlyWriter {
    type Writer = fs::File;

    fn make_writer(&'a self) -> Self::Writer {
        let file = self.current_file(chrono::Utc::now());
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .expect("failed to open monthly log file")
    }
}

/// `logs/laravel.log` + `2026-08` → `logs/laravel-2026-08.log`.
/// Paths without an extension get the month appended: `logs/app` →
/// `logs/app-2026-08`.
fn monthly_path(path: &Path, month: &str) -> PathBuf {
    match path.extension() {
        Some(ext) if !ext.is_empty() => {
            let mut stem = path.file_stem().unwrap_or_default().to_os_string();
            stem.push(format!("-{month}"));
            stem.push(".");
            stem.push(ext);
            path.with_file_name(stem)
        }
        _ => {
            let mut s = path.as_os_str().to_os_string();
            s.push(format!("-{month}"));
            PathBuf::from(s)
        }
    }
}

/// Delete the oldest monthly files beyond `max_files`, keeping only the most
/// recent ones. Only files matching the `{stem}-{YYYY-MM}{ext}` pattern of
/// `path` are counted.
fn prune_old_monthly_files(path: &Path, max_files: usize) {
    if max_files == 0 {
        return;
    }
    let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let (prefix, suffix) = match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) if !ext.is_empty() => (
            format!("{}-", stem.to_string_lossy()),
            format!(".{}", ext.to_string_lossy()),
        ),
        (Some(stem), None) => (format!("{}-", stem.to_string_lossy()), String::new()),
        _ => return,
    };
    let mut matches: Vec<(String, String)> = entries
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.starts_with(&prefix) && name.ends_with(&suffix))
        .map(|name| {
            let month = name
                .trim_start_matches(&prefix)
                .trim_end_matches(&suffix)
                .to_string();
            (month, name)
        })
        .filter(|(month, _)| {
            month.len() == 7 && month.bytes().all(|b| b.is_ascii_digit() || b == b'-')
        })
        .collect();
    matches.sort();
    let keep = matches.len().saturating_sub(max_files);
    for (_, name) in matches.into_iter().take(keep) {
        let _ = fs::remove_file(parent.join(name));
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn unique_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("larastvel_log_{}_{}", tag, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_monthly_path_with_extension() {
        assert_eq!(
            monthly_path(Path::new("logs/laravel.log"), "2026-08"),
            PathBuf::from("logs/laravel-2026-08.log")
        );
    }

    #[test]
    fn test_monthly_path_without_extension() {
        assert_eq!(
            monthly_path(Path::new("logs/app"), "2026-08"),
            PathBuf::from("logs/app-2026-08")
        );
    }

    #[test]
    fn test_monthly_path_dotted_dir() {
        assert_eq!(
            monthly_path(Path::new("logs/v1.laravel.log"), "2026-08"),
            PathBuf::from("logs/v1.laravel-2026-08.log")
        );
    }

    #[test]
    fn test_monthly_writer_writes_to_current_month_file() {
        let dir = unique_dir("writer");
        let writer = MonthlyWriter::new(dir.join("laravel.log"));
        let mut file = writer.make_writer();
        writeln!(file, "hello monthly").unwrap();
        drop(file);

        let now = chrono::Utc::now();
        let expected = dir.join(format!("laravel-{}.log", now.format("%Y-%m")));
        assert_eq!(fs::read_to_string(&expected).unwrap(), "hello monthly\n");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_monthly_writer_current_file_uses_fixed_time() {
        use chrono::TimeZone;
        let writer = MonthlyWriter::new("logs/laravel.log");
        let now = chrono::Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap();
        assert_eq!(
            writer.current_file(now),
            PathBuf::from("logs/laravel-2026-08.log")
        );
    }

    #[test]
    fn test_prune_keeps_newest_monthly_files() {
        let dir = unique_dir("prune");
        for month in ["2026-01", "2026-02", "2026-03", "2026-04", "2026-05"] {
            fs::write(dir.join(format!("laravel-{}.log", month)), "x").unwrap();
        }
        // Unrelated files must be left alone.
        fs::write(dir.join("laravel.log"), "x").unwrap();
        fs::write(dir.join("laravel-2026.log"), "x").unwrap();

        prune_old_monthly_files(&dir.join("laravel.log"), 3);

        let remaining: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert!(remaining.contains(&"laravel-2026-03.log".to_string()));
        assert!(remaining.contains(&"laravel-2026-04.log".to_string()));
        assert!(remaining.contains(&"laravel-2026-05.log".to_string()));
        assert!(!remaining.contains(&"laravel-2026-01.log".to_string()));
        assert!(!remaining.contains(&"laravel-2026-02.log".to_string()));
        assert!(remaining.contains(&"laravel.log".to_string()));
        assert!(remaining.contains(&"laravel-2026.log".to_string()));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_prune_noop_with_high_max_files() {
        let dir = unique_dir("prune_noop");
        for month in ["2026-01", "2026-02"] {
            fs::write(dir.join(format!("app-{}.log", month)), "x").unwrap();
        }
        prune_old_monthly_files(&dir.join("app.log"), 10);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }
}
