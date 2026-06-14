use colored::*;

pub async fn schedule_list() {
    println!("{}", "Scheduled Tasks".cyan().bold());

    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--schedule:list"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            // Output is handled by the user's app
        }
        _ => {
            eprintln!(
                "{}",
                "Failed to list scheduled tasks. Make sure you're in the project root directory."
                    .red()
            );
            eprintln!(
                "{}",
                "In your application's main.rs, add a --schedule:list argument handler:".dimmed()
            );
            eprintln!(
                "{}",
                "  let events = schedule.events();".to_string().dimmed()
            );
            eprintln!("{}", "  for event in events {".to_string().dimmed());
            eprintln!(
                "{}",
                "    println!(\"  {}  {}\", event.description(), \"schedule expression\");"
                    .to_string()
                    .dimmed()
            );
            eprintln!("{}", "  }".to_string().dimmed());
        }
    }
}

pub async fn run_schedule_command() {
    println!("{}", "⚡ Running scheduled tasks...".green().bold());
    println!("{}", "  Press Ctrl+C to stop.".dimmed());

    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--schedule:run"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("{}", "✓ Scheduled tasks completed.".green());
        }
        _ => {
            eprintln!(
                "{}",
                "Failed to run scheduled tasks. Make sure you're in the project root directory."
                    .red()
            );
            eprintln!(
                "{}",
                "In your application's main.rs, add a --schedule:run argument handler:".dimmed()
            );
            eprintln!("{}", "  let schedule = Schedule::new();".dimmed());
            eprintln!(
                "{}",
                "  schedule.call(\"* * * * *\", \"log stats\", || async { Ok(()) });".dimmed()
            );
            eprintln!(
                "{}",
                "  let manager = ScheduleManager::new(schedule);".dimmed()
            );
            eprintln!("{}", "  manager.run_due_async().await;".dimmed());
        }
    }
}
