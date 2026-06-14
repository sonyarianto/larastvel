use colored::*;

pub fn config_cache() {
    let cache_dir = std::path::Path::new("bootstrap/cache");
    std::fs::create_dir_all(cache_dir).unwrap();

    let config = larastvel_core::config::Config::load(std::path::Path::new("."));

    let cache_path = cache_dir.join("config.json");
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&cache_path, json).unwrap();

    println!(
        "{}",
        format!(
            "✓ Config cached successfully to '{}'.",
            cache_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Use config:clear to remove the cached file."
            .to_string()
            .dimmed()
    );
}

pub fn config_clear() {
    let cache_path = std::path::Path::new("bootstrap/cache/config.json");
    if cache_path.exists() {
        match std::fs::remove_file(cache_path) {
            Ok(_) => {
                println!(
                    "{}",
                    format!("✓ Cached config cleared from '{}'.", cache_path.display())
                        .green()
                        .bold()
                );
            }
            Err(e) => {
                eprintln!("{}", format!("Error clearing config cache: {}", e).red());
            }
        }
    } else {
        println!(
            "{}",
            "No cached config found. Run 'config:cache' first.".yellow()
        );
    }
}
