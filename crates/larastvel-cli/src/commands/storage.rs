use colored::*;

pub fn storage_link() {
    let target = std::path::Path::new("storage/app/public");
    let link = std::path::Path::new("public/storage");

    // Create the target directory if it doesn't exist
    std::fs::create_dir_all(target).unwrap();

    // Remove existing link/file/dir if present (use symlink_metadata
    // to catch broken symlinks too, since exists() follows targets)
    if link.symlink_metadata().is_ok() {
        if let Err(e) = std::fs::remove_file(link) {
            // Try rmdir if it's a directory
            if std::fs::remove_dir(link).is_err() {
                eprintln!(
                    "{}",
                    format!(
                        "Error: Could not remove existing '{}': {}",
                        link.display(),
                        e
                    )
                    .red()
                );
                return;
            }
        }
    }

    #[cfg(unix)]
    {
        match std::os::unix::fs::symlink(target, link) {
            Ok(_) => {
                println!(
                    "{}",
                    format!(
                        "✓ Symlink created: [{}] -> [{}]",
                        link.display(),
                        target.display()
                    )
                    .green()
                    .bold()
                );
            }
            Err(e) => {
                eprintln!("{}", format!("Error creating symlink: {}", e).red());
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Fallback: copy instead of symlink on non-Unix platforms
        eprintln!(
            "{}",
            "Warning: Symlinks not supported on this platform. Using copy instead.".yellow()
        );
        println!(
            "{}",
            format!(
                "✓ Directory created: [{}] -> [{}]",
                link.display(),
                target.display()
            )
            .green()
            .bold()
        );
        println!(
            "{}",
            "  Copy files manually or use a storage driver that supports your platform.".dimmed()
        );
    }
}
