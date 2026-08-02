use colored::*;

use super::{config_cache, config_clear, route_cache, route_clear};

pub async fn optimize_all() {
    config_cache();
    route_cache().await;
    println!();
    println!(
        "{}",
        "✓ Application optimized — config and routes cached."
            .green()
            .bold()
    );
}

pub fn optimize_clear() {
    config_clear();
    route_clear();
    println!();
    println!("{}", "✓ Optimization caches cleared.".green().bold());
}
