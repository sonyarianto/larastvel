use colored::*;

pub fn run_migrate_command(subcommand: &str) {
    println!("{}", format!("Running '{}'...", subcommand).green().bold());
    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--migrate", subcommand])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("{}", "✓ Migration completed successfully.".green());
        }
        _ => {
            eprintln!(
                "{}",
                "Migration failed. Make sure you're in the project root directory.".red()
            );
            eprintln!(
                "{}",
                "You can also run: cargo run -- --migrate <command>".dimmed()
            );
        }
    }
}
