use colored::*;

pub fn maintenance_down(message: Option<String>, retry: Option<u64>, force: bool) {
    let down_file = std::path::Path::new("storage/framework/down");

    if down_file.exists() && !force {
        eprintln!(
            "{}",
            format!(
                "Application is already in maintenance mode at '{}'.",
                down_file.display()
            )
            .yellow()
        );
        eprintln!(
            "{}",
            "  Use --force to overwrite the existing down file.".dimmed()
        );
        return;
    }

    std::fs::create_dir_all(down_file.parent().unwrap()).unwrap();

    let payload = serde_json::json!({
        "message": message.unwrap_or_else(|| "Application is in maintenance mode.".to_string()),
        "retry": retry,
        "time": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });

    let content = serde_json::to_string_pretty(&payload).unwrap();
    std::fs::write(down_file, content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Application is now in maintenance mode at '{}'.",
            down_file.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Use 'up' to bring the application back online.".dimmed()
    );
}

pub fn maintenance_up() {
    let down_file = std::path::Path::new("storage/framework/down");

    if !down_file.exists() {
        println!("{}", "Application is not in maintenance mode.".yellow());
        return;
    }

    match std::fs::remove_file(down_file) {
        Ok(_) => {
            println!(
                "{}",
                format!(
                    "✓ Application is now live. Maintenance mode cleared from '{}'.",
                    down_file.display()
                )
                .green()
                .bold()
            );
        }
        Err(e) => {
            eprintln!(
                "{}",
                format!("Error clearing maintenance mode: {}", e).red()
            );
        }
    }
}
