use colored::*;
use larastvel_core::routing::{find_route_conflicts, RouteDefinition};

pub async fn route_cache() {
    println!("{}", "⚡ Caching routes...".green().bold());

    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--route:cache"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!(
                "{}",
                format!("✓ Routes cached to '{}'.", "bootstrap/cache/routes.json")
                    .green()
                    .bold()
            );
            println!(
                "{}",
                "  Use route:clear to remove the cached routes file.".dimmed()
            );
        }
        _ => {
            eprintln!(
                "{}",
                "Failed to cache routes. Make sure you're in the project root directory.".red()
            );
            eprintln!(
                "{}",
                "In your application's main.rs, add a --route:cache argument handler:".dimmed()
            );
            eprintln!(
                "{}",
                "  let registrar = Registrar::new(Arc::new(Mutex::new(AxumRouter::new())), Arc::new(Mutex::new(vec![])));".to_string()
                .dimmed()
            );
            eprintln!("{}", "  routes::web(&registrar);".dimmed());
            eprintln!("{}", "  routes::api(&registrar);".dimmed());
            eprintln!(
                "{}",
                "  let routes_json = serde_json::to_string_pretty(&registrar.list_routes()).unwrap();".to_string()
                .dimmed()
            );
            eprintln!(
                "{}",
                "  std::fs::create_dir_all(\"bootstrap/cache\").unwrap();".dimmed()
            );
            eprintln!(
                "{}",
                "  std::fs::write(\"bootstrap/cache/routes.json\", routes_json).unwrap();".dimmed()
            );
        }
    }
}

pub fn route_clear() {
    let cache_path = std::path::Path::new("bootstrap/cache/routes.json");
    if cache_path.exists() {
        match std::fs::remove_file(cache_path) {
            Ok(_) => {
                println!(
                    "{}",
                    format!("✓ Cached routes cleared from '{}'.", cache_path.display())
                        .green()
                        .bold()
                );
            }
            Err(e) => {
                eprintln!("{}", format!("Error clearing routes cache: {}", e).red());
            }
        }
    } else {
        println!(
            "{}",
            "No cached routes found. Run 'route:cache' first.".yellow()
        );
    }
}

pub fn route_conflicts_list() {
    let cache_path = std::path::Path::new("bootstrap/cache/routes.json");
    if !cache_path.exists() {
        println!(
            "{}",
            "No cached routes found. Run 'route:cache' first.".yellow()
        );
        return;
    }

    let content = match std::fs::read_to_string(cache_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", format!("Error reading routes cache: {}", e).red());
            return;
        }
    };
    let routes: Vec<RouteDefinition> = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", format!("Error parsing routes cache: {}", e).red());
            return;
        }
    };

    let conflicts = find_route_conflicts(&routes);
    if conflicts.is_empty() {
        println!(
            "{}",
            format!(
                "✓ No route conflicts found ({} routes checked).",
                routes.len()
            )
            .green()
            .bold()
        );
        return;
    }

    println!(
        "{}",
        format!("Found {} route conflict(s):", conflicts.len())
            .red()
            .bold()
    );
    for c in &conflicts {
        println!(
            "  {}",
            format!(
                "{} {}  vs  {} {}",
                c.method, c.first_uri, c.method, c.second_uri
            )
            .yellow()
        );
        println!(
            "    {}",
            format!(
                "{} ({}) and {} ({})",
                c.first_uri, c.first_handler, c.second_uri, c.second_handler
            )
            .dimmed()
        );
    }
}
