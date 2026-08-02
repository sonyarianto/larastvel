use colored::*;

pub fn about() {
    let config = larastvel_core::config::Config::load(std::path::Path::new("."));

    println!("{}", "Larastvel Framework".cyan().bold());
    println!();

    let rust_version = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!(
        "  {}",
        format!("Application Name: {}", config.app.name).green()
    );
    println!(
        "  {}",
        format!("Larastvel Version: {}", env!("CARGO_PKG_VERSION")).green()
    );
    println!("  {}", format!("Rust Version: {}", rust_version).dimmed());
    println!("  {}", format!("Environment: {}", config.app.env).dimmed());
    println!("  {}", format!("Debug Mode: {}", config.app.debug).dimmed());
    println!("  {}", format!("URL: {}", config.app.url).dimmed());
    println!();
    println!(
        "  {}",
        format!("Database: {}", config.database.driver).dimmed()
    );
    println!(
        "  {}",
        format!("Cache Driver: {}", config.cache.default).dimmed()
    );
}
