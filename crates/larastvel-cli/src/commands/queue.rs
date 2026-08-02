use colored::*;

use larastvel_core::config::Config;
use larastvel_core::database::DatabaseManager;
use larastvel_core::queue::FailedJobStore;

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

/// Connect to the application database (from `.env` / `config/*.toml` in the
/// current directory) and return a ready-to-use failed-job store.
async fn connect_failed_store() -> Result<FailedJobStore, String> {
    let config = Config::load(std::path::Path::new("."));
    let db = DatabaseManager::new(&config);
    let conn = db
        .connection()
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    let store = FailedJobStore::new(conn);
    store
        .ensure_table_exists()
        .await
        .map_err(|e| format!("Failed to prepare failed_jobs table: {}", e))?;
    Ok(store)
}

pub async fn queue_failed_list() {
    match connect_failed_store().await {
        Ok(store) => match store.all().await {
            Ok(jobs) if jobs.is_empty() => {
                println!("{}", "No failed jobs found.".yellow());
            }
            Ok(jobs) => {
                println!("{}", "Failed Jobs".cyan().bold());
                println!("{}", format!("{} total", jobs.len()).dimmed());
                println!();
                for job in &jobs {
                    println!(
                        "{}  {}  {}",
                        format!("[{}]", job.id).dimmed(),
                        job.class.white(),
                        format!("queue: {}", job.queue).dimmed()
                    );
                    let ts = job.failed_at.to_string();
                    println!("     {} {}", "failed at:".dimmed(), ts.dimmed());
                    println!("     {} {}", "exception:".dimmed(), job.exception.red());
                    println!();
                }
            }
            Err(e) => eprintln!("{}", format!("Error listing failed jobs: {}", e).red()),
        },
        Err(e) => eprintln!("{}", e.red()),
    }
}

pub async fn queue_retry(ids: Vec<String>) {
    let Ok(store) = connect_failed_store().await else {
        eprintln!("{}", "Failed to connect to database.".red());
        return;
    };
    let jobs = match store.all().await {
        Ok(jobs) => jobs,
        Err(e) => {
            eprintln!("{}", format!("Error reading failed jobs: {}", e).red());
            return;
        }
    };

    let retry_all = ids.iter().any(|id| id == "all");
    let mut selected = Vec::new();
    for id in &ids {
        if id == "all" {
            selected.extend(jobs.iter().cloned());
        } else if let Ok(num) = id.parse::<i64>() {
            if let Some(job) = jobs.iter().find(|j| j.id == num) {
                selected.push(job.clone());
            }
        }
    }
    if retry_all {
        selected = jobs.clone();
    }

    if selected.is_empty() {
        println!("{}", "No retryable failed jobs found.".yellow());
        return;
    }

    for job in &selected {
        match store.requeue("jobs", job).await {
            Ok(()) => {
                println!(
                    "{}",
                    format!("✓ Requeued failed job [{}] {}", job.id, job.class).green()
                );
            }
            Err(e) => eprintln!(
                "{}",
                format!("Failed to requeue job [{}]: {}", job.id, e).red()
            ),
        }
    }
}

pub async fn queue_forget(id: String) {
    let Ok(store) = connect_failed_store().await else {
        eprintln!("{}", "Failed to connect to database.".red());
        return;
    };
    let Ok(num) = id.parse::<i64>() else {
        eprintln!("{}", format!("Invalid job id: {}", id).red());
        return;
    };
    match store.forget(num).await {
        Ok(true) => println!("{}", format!("✓ Forgotten failed job [{}].", num).green()),
        Ok(false) => println!("{}", format!("No failed job with id [{}].", num).yellow()),
        Err(e) => eprintln!("{}", format!("Error forgetting failed job: {}", e).red()),
    }
}

pub async fn queue_flush() {
    let Ok(store) = connect_failed_store().await else {
        eprintln!("{}", "Failed to connect to database.".red());
        return;
    };
    match store.flush().await {
        Ok(count) => println!("{}", format!("✓ Flushed {} failed job(s).", count).green()),
        Err(e) => eprintln!("{}", format!("Error flushing failed jobs: {}", e).red()),
    }
}
