use colored::*;

pub fn run_seed_command() {
    println!("{}", "Running database seeders...".green().bold());
    let status = std::process::Command::new("cargo")
        .args(["run", "--", "--seed"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("{}", "✓ Database seeding completed.".green());
        }
        _ => {
            eprintln!(
                "{}",
                "Seeding failed. Make sure you're in the project root directory.".red()
            );
            eprintln!("{}", "You can also run: cargo run -- --seed".dimmed());
        }
    }
}
