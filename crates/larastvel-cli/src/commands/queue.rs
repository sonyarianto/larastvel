use colored::*;

pub async fn queue_work(once: bool, queue: &str, sleep: u64) {
    println!(
        "{}",
        format!(
            "⚡ Queue worker starting [queue: {}, sleep: {}s]...",
            queue, sleep
        )
        .green()
        .bold()
    );
    println!("{}", "  Press Ctrl+C to stop.".dimmed());

    let status = std::process::Command::new("cargo")
        .args([
            "run",
            "--",
            &format!("--queue:work={}", queue),
            &format!("--queue-sleep={}", sleep),
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            if once {
                println!("{}", "✓ Queue worker completed.".green());
            }
        }
        _ => {
            eprintln!(
                "{}",
                "Failed to start queue worker. Make sure you're in the project root directory."
                    .red()
            );
            eprintln!(
                "{}",
                "In your application's main.rs, add a --queue:work argument handler:".dimmed()
            );
            eprintln!(
                "{}",
                "  let mut db = DatabaseManager::new(&app.config());"
                    .to_string()
                    .dimmed()
            );
            eprintln!(
                "{}",
                "  let conn = db.connect().await?;".to_string().dimmed()
            );
            eprintln!(
                "{}",
                "  let queue = DatabaseQueue::new(\"default\", conn, resolver);"
                    .to_string()
                    .dimmed()
            );
            eprintln!(
                "{}",
                "  let worker = QueueWorker::new(Arc::new(queue));"
                    .to_string()
                    .dimmed()
            );
            eprintln!("{}", "  worker.work().await;".dimmed());
        }
    }

    if once {
        println!(
            "{}",
            "  Pass --once to process a single job, or omit it to keep the worker running."
                .dimmed()
        );
    } else {
        println!(
            "{}",
            "  Use --once to process a single job, or omit it to keep the worker running.".dimmed()
        );
    }
}
