use tracing_subscriber::EnvFilter;

use crate::config::Config;

pub fn init(config: &Config) {
    let level = config
        .get("logging.level")
        .unwrap_or_else(|| "debug".to_string());
    let filter = EnvFilter::try_new(&level).unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();
}
