use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "larastvel",
    about = "Larastvel Framework CLI",
    version = "0.2.0"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the development server
    Serve {
        #[arg(short, long, default_value = "8080")]
        port: u16,
        #[arg(short, long)]
        host: Option<String>,
    },
    /// List all registered routes
    #[command(name = "route:list")]
    RouteList,
    /// Display framework version
    Version,
    /// Run scheduled tasks
    #[command(name = "schedule:run")]
    ScheduleRun,
    /// Create a new Larastvel application
    New { name: String },
    /// Generate a new application key
    #[command(name = "key:generate")]
    KeyGenerate,
    /// Run database migrations
    Migrate,
    /// Drop all tables and re-run all migrations
    #[command(name = "migrate:fresh")]
    MigrateFresh,
    /// Rollback the last migration (or N steps)
    #[command(name = "migrate:rollback")]
    MigrateRollback {
        #[arg(short, long)]
        steps: Option<u32>,
    },
    /// Run database seeders
    #[command(name = "db:seed")]
    DbSeed,
    /// Create a symbolic link from public/storage to storage/app/public
    #[command(name = "storage:link")]
    StorageLink,
    /// Create a migration for the notifications table
    #[command(name = "notifications:table")]
    NotificationsTable,
    /// Cache config values into a single file for faster loading
    #[command(name = "config:cache")]
    ConfigCache,
    /// Clear the cached config file
    #[command(name = "config:clear")]
    ConfigClear,
    /// Cache all registered routes into a single file
    #[command(name = "route:cache")]
    RouteCache,
    /// Clear the cached routes file
    #[command(name = "route:clear")]
    RouteClear,
    /// Display the current environment variables (.env + config)
    Env,
    /// Put the application into maintenance mode
    Down {
        /// The message to display to visitors
        #[arg(short, long)]
        message: Option<String>,
        /// The number of seconds after which a request may be retried
        #[arg(short, long)]
        retry: Option<u64>,
        /// Allow maintenance mode even if the down file already exists
        #[arg(long)]
        force: bool,
    },
    /// Bring the application out of maintenance mode
    Up,
    /// List all registered scheduled tasks
    #[command(name = "schedule:list")]
    ScheduleList,
    /// Start processing jobs on the queue
    #[command(name = "queue:work")]
    QueueWork {
        /// Process only a single job from the queue
        #[arg(short, long)]
        once: bool,
        /// The name of the queue to work on
        #[arg(short, long, default_value = "default")]
        queue: String,
        /// Sleep duration (in seconds) when no job is available
        #[arg(long, default_value = "3")]
        sleep: u64,
    },
    /// Display help for any command
    Make {
        #[command(subcommand)]
        target: Option<MakeTarget>,
    },
}

#[derive(Subcommand)]
pub enum MakeTarget {
    /// Create a new model
    Model { name: String },
    /// Create a new controller
    Controller { name: String },
    /// Create a new migration
    Migration { name: String },
    /// Create a new seeder
    Seeder { name: String },
    /// Create a new policy
    Policy { name: String },
    /// Create a new test
    Test { name: String },
    /// Create a new job
    Job { name: String },
    /// Create a new event
    Event { name: String },
    /// Create a new listener
    Listener { name: String },
    /// Create a new notification
    Notification { name: String },
    /// Create a new validation rule
    Rule { name: String },
    /// Create a new console command
    Command { name: String },
    /// Create a new factory
    Factory { name: String },
    /// Create a new mail class
    Mail { name: String },
    /// Create a new query scope
    Scope { name: String },
    /// Create a new model observer
    Observer { name: String },
}
