use clap::Parser;
use colored::*;

mod cli;
mod commands;

use cli::{Cli, Commands, MakeTarget};
use commands::*;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve { port, host }) => {
            let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
            println!(
                "{}",
                format!(
                    "⚡ Larastvel development server starting on http://{}:{}",
                    host, port
                )
                .green()
                .bold()
            );
            println!("{}", "  Press Ctrl+C to stop.".dimmed());
            start_server(&host, port).await;
        }
        Some(Commands::RouteList) => {
            println!("{}", "Route List".cyan().bold());
            println!("{}", "  No routes defined yet.".dimmed());
        }
        Some(Commands::Version) => {
            println!("Larastvel Framework v{}", env!("CARGO_PKG_VERSION"));
        }
        Some(Commands::ScheduleRun) => {
            run_schedule_command().await;
        }
        Some(Commands::New { name }) => {
            println!(
                "{}",
                format!("Creating new Larastvel application: {}", name)
                    .green()
                    .bold()
            );
            create_project(&name).await;
        }
        Some(Commands::KeyGenerate) => {
            let key = uuid::Uuid::new_v4().to_string();
            println!("{}", "Application key generated:".green());
            println!("  APP_KEY={}", key.cyan());
        }
        Some(Commands::Migrate) => {
            run_migrate_command("migrate");
        }
        Some(Commands::MigrateFresh) => {
            run_migrate_command("migrate:fresh");
        }
        Some(Commands::MigrateRollback { steps }) => {
            let cmd = match steps {
                Some(n) => format!("migrate:rollback --steps {}", n),
                None => "migrate:rollback".to_string(),
            };
            run_migrate_command(&cmd);
        }
        Some(Commands::DbSeed) => {
            run_seed_command();
        }
        Some(Commands::StorageLink) => {
            storage_link();
        }
        Some(Commands::NotificationsTable) => {
            create_notifications_table();
        }
        Some(Commands::ConfigCache) => {
            config_cache();
        }
        Some(Commands::ConfigClear) => {
            config_clear();
        }
        Some(Commands::RouteCache) => {
            route_cache().await;
        }
        Some(Commands::RouteClear) => {
            route_clear();
        }
        Some(Commands::Env) => {
            env_display();
        }
        Some(Commands::Down {
            message,
            retry,
            force,
        }) => {
            maintenance_down(message, retry, force);
        }
        Some(Commands::Up) => {
            maintenance_up();
        }
        Some(Commands::ScheduleList) => {
            schedule_list().await;
        }
        Some(Commands::QueueWork { once, queue, sleep }) => {
            queue_work(once, &queue, sleep).await;
        }
        Some(Commands::Make { target }) => match target {
            Some(MakeTarget::Model { name }) => {
                make_model(&name);
            }
            Some(MakeTarget::Controller { name }) => {
                make_controller(&name);
            }
            Some(MakeTarget::Migration { name }) => {
                make_migration(&name);
            }
            Some(MakeTarget::Seeder { name }) => {
                make_seeder(&name);
            }
            Some(MakeTarget::Policy { name }) => {
                make_policy(&name);
            }
            Some(MakeTarget::Test { name }) => {
                make_test(&name);
            }
            Some(MakeTarget::Job { name }) => {
                make_job(&name);
            }
            Some(MakeTarget::Event { name }) => {
                make_event(&name);
            }
            Some(MakeTarget::Listener { name }) => {
                make_listener(&name);
            }
            Some(MakeTarget::Notification { name }) => {
                make_notification(&name);
            }
            Some(MakeTarget::Rule { name }) => {
                make_rule(&name);
            }
            Some(MakeTarget::Command { name }) => {
                make_command(&name);
            }
            Some(MakeTarget::Mail { name }) => {
                make_mail(&name);
            }
            Some(MakeTarget::Scope { name }) => {
                make_scope(&name);
            }
            Some(MakeTarget::Factory { name }) => {
                make_factory(&name);
            }
            Some(MakeTarget::Observer { name }) => {
                make_observer(&name);
            }
            None => {
                println!("{}", "Available make targets:".cyan());
                println!("  make:model       Create a new model");
                println!("  make:controller  Create a new controller");
                println!("  make:migration   Create a new migration");
                println!("  make:seeder      Create a new seeder");
                println!("  make:policy      Create a new policy");
                println!("  make:test        Create a new test");
                println!("  make:job         Create a new job");
                println!("  make:event       Create a new event");
                println!("  make:listener    Create a new event listener");
                println!("  make:observer    Create a new model observer");
                println!("  make:scope       Create a new query scope");
                println!("  make:notification Create a new notification");
                println!("  make:rule        Create a new validation rule");
                println!("  make:command     Create a new console command");
                println!("  make:mail        Create a new mail class");
                println!("  make:factory     Create a new model factory");
            }
        },
        None => {
            println!("{}", "Larastvel Framework CLI".cyan().bold());
            println!("{}", "Usage: larastvel <command>".dimmed());
            println!();
            println!("Available commands:");
            println!("  serve            Start the development server");
            println!("  route:list       List all registered routes");
            println!("  new              Create a new Larastvel application");
            println!("  key:generate     Generate a new application key");
            println!("  migrate          Run database migrations");
            println!("  migrate:fresh    Drop all tables and re-run migrations");
            println!("  migrate:rollback Rollback the last migration");
            println!("  make:model       Create a new model");
            println!("  make:controller  Create a new controller");
            println!("  make:migration   Create a new migration");
            println!("  make:seeder      Create a new seeder");
            println!("  make:policy      Create a new policy");
            println!("  make:test        Create a new test");
            println!("  make:job         Create a new job");
            println!("  make:event       Create a new event");
            println!("  make:listener    Create a new event listener");
            println!("  make:observer    Create a new model observer");
            println!("  make:scope       Create a new query scope");
            println!("  make:notification Create a new notification");
            println!("  make:rule        Create a new validation rule");
            println!("  make:command     Create a new console command");
            println!("  make:factory     Create a new model factory");
            println!("  schedule:run     Run scheduled tasks");
            println!("  db:seed          Run database seeders");
            println!("  storage:link     Create a symbolic link from public/storage to storage/app/public");
            println!("  notifications:table  Create a migration for the notifications table");
            println!("  config:cache     Cache config values into a single file");
            println!("  config:clear     Clear the cached config file");
            println!("  route:cache      Cache all registered routes into a single file");
            println!("  route:clear      Clear the cached routes file");
            println!(
                "  env              Display the current environment variables (.env + config)"
            );
            println!("  down             Put the application into maintenance mode");
            println!("  up               Bring the application out of maintenance mode");
            println!("  schedule:list    List all registered scheduled tasks");
            println!("  queue:work       Start processing jobs on the queue");
            println!("  version          Display framework version");
        }
    }
}
