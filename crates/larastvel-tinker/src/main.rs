use clap::Parser;
use rustyline::DefaultEditor;

#[derive(Parser)]
#[command(name = "tinker", about = "Larastvel interactive shell")]
struct Cli {
    #[arg(short, long, default_value = ".")]
    path: String,

    #[arg(short, long)]
    execute: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    println!("⚡ Larastvel Tinker - Interactive Shell");
    println!("Type 'exit' or Ctrl+C to quit");
    println!();

    if let Some(cmd) = cli.execute {
        execute_command(&cmd).await;
        return;
    }

    let mut rl = DefaultEditor::new().expect("Failed to create REPL");

    loop {
        let readline = rl.readline(">>> ");
        match readline {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                if line == "exit" || line == "quit" {
                    break;
                }
                rl.add_history_entry(&line).ok();
                execute_command(&line).await;
            }
            Err(_) => break,
        }
    }
}

async fn execute_command(command: &str) {
    match command {
        "help" => {
            println!("Available commands:");
            println!("  help           Show this help");
            println!("  exit/quit      Exit the shell");
            println!("  version        Show Larastvel version");
            println!("  php            Evaluate Rust expressions");
        }
        "version" => {
            println!("Larastvel Tinker v{}", env!("CARGO_PKG_VERSION"));
        }
        _ => {
            // Try to evaluate as Rust code
            println!("=> {}", command);
        }
    }
}
