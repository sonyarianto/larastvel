use colored::*;

pub fn env_display() {
    println!("{}", "Environment".cyan().bold());
    println!();

    // --- .env file ---
    let env_path = std::path::Path::new(".env");
    println!("{}", ".env".yellow().bold());

    if env_path.exists() {
        if let Ok(content) = std::fs::read_to_string(env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(pos) = line.find('=') {
                    let key = &line[..pos];
                    let value = &line[pos + 1..];
                    let display = if key.ends_with("_KEY")
                        || key.ends_with("_SECRET")
                        || key.ends_with("_PASSWORD")
                        || key == "APP_KEY"
                        || key == "DB_PASSWORD"
                    {
                        "******"
                    } else if value.is_empty() {
                        "(empty)"
                    } else {
                        value
                    };
                    println!("  {} = {}", key.cyan(), display);
                }
            }
        } else {
            println!("  {}", "(unable to read .env)".red());
        }
    } else {
        println!("  {}", "(no .env file found)".dimmed());
    }

    println!();

    // --- Config (config/ or config.toml) ---
    let config_dir = std::path::Path::new("config");
    let legacy_config = std::path::Path::new("config.toml");
    if config_dir.is_dir() {
        println!("{}", "Config (config/)".yellow().bold());
    } else if legacy_config.exists() {
        println!("{}", "Config (config.toml — legacy format)".yellow().bold());
    } else {
        println!("{}", "Config".yellow().bold());
    }

    if config_dir.is_dir() || legacy_config.exists() {
        let config = larastvel_core::config::Config::load(std::path::Path::new("."));
        println!("  {} = {}", "APP_NAME".cyan(), config.app.name);
        println!("  {} = {}", "APP_URL".cyan(), config.app.url);
        println!("  {} = {}", "APP_ENV".cyan(), config.app.env);
        println!("  {} = {}", "APP_DEBUG".cyan(), config.app.debug);
        if config.app.key.is_some() {
            println!("  {} = (masked)", "APP_KEY".cyan());
        } else {
            println!("  {} = (not set)", "APP_KEY".cyan());
        }
        println!();
        println!(
            "  {} = {} ({}://{}:{}/{})",
            "DB_CONNECTION".cyan(),
            config.database.driver,
            config.database.driver,
            config.database.host,
            config.database.port,
            config.database.database,
        );
        println!("  {} = {}", "DB_USERNAME".cyan(), config.database.username);
        println!("  {} = ******", "DB_PASSWORD".cyan());
        println!();
        println!("  {} = {}", "LOG_LEVEL".cyan(), config.logging.level);
        println!("  {} = {}", "LOG_FORMAT".cyan(), config.logging.format);
        println!();
        println!("  {} = {}", "VIEW_ENGINE".cyan(), config.view.engine);
        println!(
            "  {} = {}",
            "VIEW_PATHS".cyan(),
            config.view.paths.join(", ")
        );
    } else {
        println!("  {}", "(no config found)".dimmed());
    }
}
