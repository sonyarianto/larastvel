use colored::*;

/// Convert PascalCase to snake_case.
pub fn to_snake_case(name: &str) -> String {
    {
        let mut result = String::new();
        for (i, ch) in name.chars().enumerate() {
            {
                if ch.is_uppercase() {
                    {
                        if i > 0 {
                            {
                                result.push('_');
                            }
                        }
                        result.push(ch.to_ascii_lowercase());
                    }
                } else {
                    {
                        result.push(ch);
                    }
                }
            }
        }
        result
    }
}

pub fn make_test(name: &str) {
    let tests_dir = std::path::Path::new("tests");
    std::fs::create_dir_all(tests_dir).unwrap();

    let snake_name = to_snake_case(name);
    let file_name = if snake_name.ends_with("_test") {
        snake_name
    } else {
        format!("{}_test", snake_name)
    };

    let test_content = format!(
        r#"use larastvel_core::TestClient;

/// Test: {name}
#[cfg(test)]
mod tests {{
    use super::*;

    // #[tokio::test]
    // async fn test_example() {{
    //     let client = TestClient::new(app);
    //     let response = client.get("/").await;
    //     response.assert_ok();
    // }}
}}
"#,
        name = name,
    );

    let file_path = tests_dir.join(format!("{}.rs", file_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Test '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, test_content).unwrap();

    println!(
        "{}",
        format!("✓ Test [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Add your test assertions inside the test module.".dimmed()
    );
}

pub fn make_job(name: &str) {
    let jobs_dir = std::path::Path::new("src/jobs");
    std::fs::create_dir_all(jobs_dir).unwrap();

    let snake_name = to_snake_case(name);
    let job_name = if snake_name.ends_with("_job") {
        name.to_string()
    } else {
        format!("{}Job", name)
    };

    let job_content = format!(
        r#"use larastvel_core::queue::{{JobError, ShouldQueue}};
use async_trait::async_trait;

#[derive(Debug)]
pub struct {name};

#[async_trait]
impl ShouldQueue for {name} {{
    fn name(&self) -> &str {{
        "{snake}"
    }}

    async fn handle(&self) -> Result<(), JobError> {{
        // TODO: Implement job logic
        tracing::info!("Job executed: {name}");
        Ok(())
    }}
}}
"#,
        name = job_name,
        snake = snake_name,
    );

    let file_path = jobs_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Job '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, job_content).unwrap();

    // Update mod.rs
    let mod_path = jobs_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Job [{}] created at '{}'.", job_name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Dispatch with: larastvel_core::queue::dispatch(MyJob).await;".dimmed()
    );
}

pub fn make_event(name: &str) {
    let events_dir = std::path::Path::new("src/events");
    std::fs::create_dir_all(events_dir).unwrap();

    let snake_name = to_snake_case(name);

    let event_content = format!(
        r#"use larastvel_core::events::Listener;
use async_trait::async_trait;

/// Event payload for {name}
#[derive(Debug, Clone)]
pub struct {name}Event {{
    // TODO: Add event data fields
}}

/// Listener for {name}
#[derive(Debug)]
pub struct {name}Listener;

#[async_trait]
impl Listener<{name}Event> for {name}Listener {{
    async fn handle(&self, _event: &{name}Event) {{
        // TODO: Handle the event
        tracing::info!("Event handled: {name}");
    }}
}}
"#,
        name = name,
    );

    let file_path = events_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Event '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, event_content).unwrap();

    // Update mod.rs
    let mod_path = events_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Event [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Register with: EventService::listen::<MyEvent, MyListener>();".dimmed()
    );
    println!(
        "{}",
        "  Dispatch with: larastvel_core::events::dispatch(MyEvent).await;".dimmed()
    );
}

pub fn make_notification(name: &str) {
    let notifications_dir = std::path::Path::new("src/notifications");
    std::fs::create_dir_all(notifications_dir).unwrap();

    let snake_name = to_snake_case(name);
    let struct_name = if name.ends_with("Notification") {
        name.to_string()
    } else {
        format!("{}Notification", name)
    };

    let notification_content = format!(
        r#"use larastvel_core::mail::{{Mailable, Mailer}};

/// Notification: {name}
#[derive(Debug)]
pub struct {struct_name} {{
    // TODO: Add notification data fields
}}

impl {struct_name} {{
    pub async fn send(&self, mailer: &dyn Mailer, to: &str) -> Result<(), Box<dyn std::error::Error>> {{
        let mailable = Mailable::html(
            vec![to.to_string()],
            "Notification: {name}",
            "<h1>{name}</h1><p>Your notification content here.</p>",
        )
        .from("noreply@example.com");

        mailer.send(mailable).await?;
        Ok(())
    }}
}}
"#,
        struct_name = struct_name,
        name = name,
    );

    let file_path = notifications_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Notification '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, notification_content).unwrap();

    // Update mod.rs
    let mod_path = notifications_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Notification [{}] created at '{}'.",
            struct_name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Customize the email template and data fields.".dimmed()
    );
}

pub fn make_rule(name: &str) {
    let rules_dir = std::path::Path::new("src/rules");
    std::fs::create_dir_all(rules_dir).unwrap();

    let snake_name = to_snake_case(name);
    let struct_name = if name.ends_with("Rule") {
        name.to_string()
    } else {
        format!("{}Rule", name)
    };

    let rule_content = format!(
        r#"use larastvel_core::validation::{{ValidationRule, ValidationError}};

/// Validation rule: {name}
#[derive(Debug, Clone)]
pub struct {struct_name} {{
    // TODO: Add rule parameters
}}

impl {struct_name} {{
    pub fn new() -> Self {{
        Self {{
            // TODO: Initialize rule parameters
        }}
    }}
}}

impl ValidationRule for {struct_name} {{
    fn name(&self) -> &str {{
        "{snake}"
    }}

    fn validate(&self, _field: &str, _value: &str) -> Result<(), ValidationError> {{
        // TODO: Implement validation logic
        // Return Ok(()) for valid, Err(ValidationError::new("message")) for invalid
        Ok(())
    }}
}}
"#,
        struct_name = struct_name,
        name = name,
        snake = snake_name,
    );

    let file_path = rules_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Rule '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, rule_content).unwrap();

    // Update mod.rs
    let mod_path = rules_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Rule [{}] created at '{}'.",
            struct_name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Add validation logic in the `validate` method.".dimmed()
    );
    println!(
        "{}",
        "  Register with: Validator::extend(MyRule::new());".dimmed()
    );
}

pub fn make_command(name: &str) {
    let commands_dir = std::path::Path::new("src/commands");
    std::fs::create_dir_all(commands_dir).unwrap();

    let snake_name = to_snake_case(name);
    let struct_name = if name.ends_with("Command") {
        name.to_string()
    } else {
        format!("{}Command", name)
    };

    let command_content = format!(
        r#"use clap::Parser;

/// {name}
#[derive(Debug, Parser)]
pub struct {struct_name} {{
    // TODO: Add command arguments
    // #[arg(short, long)]
    // pub name: Option<String>,
}}

impl {struct_name} {{
    pub async fn execute(&self) -> Result<(), Box<dyn std::error::Error>> {{
        // TODO: Implement command logic
        println!("Command executed: {name}");
        Ok(())
    }}
}}
"#,
        struct_name = struct_name,
        name = name,
    );

    let file_path = commands_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Command '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, command_content).unwrap();

    // Update mod.rs
    let mod_path = commands_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Command [{}] created at '{}'.",
            struct_name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Register the command in your console kernel.".dimmed()
    );
}

pub fn make_policy(name: &str) {
    let policies_dir = std::path::Path::new("src/policies");
    std::fs::create_dir_all(policies_dir).unwrap();

    let snake_name = to_snake_case(name);

    let resource_name = snake_name.strip_suffix("_policy").unwrap_or(&snake_name);

    // Split the model name from the policy name
    // e.g. "PostPolicy" -> resource "post", policy "PostPolicy"
    let policy_name = name;

    let policy_content = format!(
        r#"use larastvel_core::auth::{{AuthenticatedUser, GateCheck, Policy}};

#[derive(Debug)]
pub struct {name};

impl {name} {{
    /// Register this policy with the given gate.
    ///
    /// Call this in your application's service provider:
    /// ```ignore
    /// gate.register_policy("{resource}", std::sync::Arc::new({name}));
    /// ```
    pub fn register(gate: &larastvel_core::auth::Gate) {{
        gate.register_policy("{resource}", std::sync::Arc::new({name}));
    }}
}}

impl Policy for {name} {{
    fn resource(&self) -> &str {{
        "{resource}"
    }}

    fn check(
        &self,
        _user: &AuthenticatedUser,
        ability: &str,
        _args: &[String],
    ) -> Option<GateCheck> {{
        match ability {{
            "view-{resource}" | "create-{resource}" | "update-{resource}" | "delete-{resource}" => {{
                Some(GateCheck::Allowed)
            }}
            _ => None,
        }}
    }}
}}
"#,
        name = policy_name,
        resource = resource_name,
    );

    let file_path = policies_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Policy '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, policy_content).unwrap();

    // Update mod.rs
    let mod_path = policies_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Policy [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Implement check logic in the `check` method for each ability.".dimmed()
    );
    println!(
        "{}",
        "  Register the policy in your AuthServiceProvider: PostPolicy::register(&gate);".dimmed()
    );
}

pub fn make_model(name: &str) {
    let models_dir = std::path::Path::new("src/models");
    std::fs::create_dir_all(models_dir).unwrap();

    let snake_name = to_snake_case(name);

    let model_content = format!(
        r#"use larastvel_core::sea_orm;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "{table}")]
pub struct Model {{
    #[sea_orm(primary_key)]
    pub id: i32,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {{}}

impl ActiveModelBehavior for ActiveModel {{}}

pub struct {name};

impl larastvel_core::models::DbModel for {name} {{
        type Entity = Entity;
    }}
"#,
        name = name,
        table = snake_name
    );

    let file_path = models_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Model '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, model_content).unwrap();

    // Update mod.rs
    let mod_path = models_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Model [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
}

pub fn make_controller(name: &str) {
    let controllers_dir = std::path::Path::new("src/controllers");
    std::fs::create_dir_all(controllers_dir).unwrap();

    let snake_name = to_snake_case(name);

    let controller_content = format!(
        r#"use larastvel_core::axum::response::{{IntoResponse, Json, Response}};
use larastvel_core::Resource;
use serde_json::json;

#[derive(Resource)]
pub struct {name};

impl larastvel_core::routing::ResourceController for {name} {{
    const RESOURCE_NAME: &'static str = "{resource_name}";
}}
"#,
        name = name,
        resource_name = snake_name
    );

    let file_path = controllers_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Controller '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, controller_content).unwrap();

    // Update mod.rs
    let mod_path = controllers_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Controller [{}] created at '{}'.",
            name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Override resource methods in the `impl ResourceController` block to add custom logic."
            .dimmed()
    );
    println!(
        "{}",
        "  Register routes: MyController::register_routes(&registrar);".dimmed()
    );
}

pub fn make_seeder(name: &str) {
    let seeders_dir = std::path::Path::new("src/database/seeders");
    std::fs::create_dir_all(seeders_dir).unwrap();

    let snake_name = to_snake_case(name);

    let seeder_content = format!(
        r#"use larastvel_core::database::DatabaseSeeder;
use larastvel_core::sea_orm::DbConn;

pub struct {name};

impl {name} {{
    pub async fn run(conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {{
        // TODO: Insert seed data here
        // Example:
        // use sea_orm::{{ActiveModelTrait, Set}};
        // use crate::models::user::ActiveModel as UserActiveModel;
        //
        // let user = UserActiveModel {{
        //     name: Set("Admin".to_string()),
        //     email: Set("admin@example.com".to_string()),
        //     password: Set(larastvel_core::hash::make("password")?),
        //     ..Default::default()
        // }};
        // user.insert(conn).await?;

        tracing::info!("Seeded: {name}");
        Ok(())
    }}
}}
"#,
        name = name,
    );

    let file_path = seeders_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Seeder '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, seeder_content).unwrap();

    // Update mod.rs
    let mod_path = seeders_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!("✓ Seeder [{}] created at '{}'.", name, file_path.display())
            .green()
            .bold()
    );
    println!(
        "{}",
        "  Implement the `run` method to insert seed data.".dimmed()
    );
}

pub fn make_migration(name: &str) {
    let migrations_dir = std::path::Path::new("src/database/migrations");
    std::fs::create_dir_all(migrations_dir).unwrap();

    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();
    let version = format!("m{}", secs);
    let snake_name = to_snake_case(name);
    let file_name = format!("{}_{}", version, snake_name);

    let migration_content = r#"use larastvel_core::sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        todo!("Implement up migration");
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        todo!("Implement down migration");
    }
}
"#
    .to_string();

    let file_path = migrations_dir.join(format!("{}.rs", file_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Migration '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, migration_content).unwrap();

    // Register in mod.rs
    let mod_path = migrations_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", file_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Migration [{}] created at '{}'.",
            name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Don't forget to register the migration in src/database/migrator.rs".dimmed()
    );
}

pub fn make_mail(name: &str) {
    let mails_dir = std::path::Path::new("src/mails");
    std::fs::create_dir_all(mails_dir).unwrap();

    let snake_name = to_snake_case(name);
    let struct_name = if name.ends_with("Mail") {
        name.to_string()
    } else {
        format!("{}Mail", name)
    };

    let mail_content = format!(
        r#"use larastvel_core::mail::{{Mailable, Mailer}};

/// Mailable: {name}
#[derive(Debug)]
pub struct {struct_name} {{
    // TODO: Add email data fields
}}

impl {struct_name} {{
    pub fn new() -> Self {{
        Self {{
            // TODO: Initialize with data
        }}
    }}

    pub async fn send(&self, mailer: &dyn Mailer, to: &str) -> Result<(), Box<dyn std::error::Error>> {{
        let mailable = Mailable::html(
            vec![to.to_string()],
            "{name}",
            "<h1>{name}</h1><p>Your message here.</p>",
        )
        .from("noreply@example.com");

        mailer.send(mailable).await?;
        Ok(())
    }}
}}
"#,
        struct_name = struct_name,
        name = struct_name,
    );

    let file_path = mails_dir.join(format!("{}.rs", snake_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Mail '{}' already exists at '{}'.",
                name,
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, mail_content).unwrap();

    // Update mod.rs
    let mod_path = mails_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", snake_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Mail [{}] created at '{}'.",
            struct_name,
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Customize the email template and data fields.".dimmed()
    );
    println!(
        "{}",
        "  Send with: my_mail.send(&mailer, \"user@example.com\").await?;".dimmed()
    );
}
