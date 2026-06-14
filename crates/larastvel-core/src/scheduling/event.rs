use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::cron::CronExpression;
use crate::queue::JobError;

type JobCallback =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), JobError>> + Send>> + Send + Sync>;

pub struct ScheduledEvent {
    pub cron: CronExpression,
    pub description: String,
    pub timezone: Option<String>,
    pub(super) callback: Option<JobCallback>,
    even_in_maintenance: bool,
    on_one_server: bool,
    run_in_background: bool,
}

impl Clone for ScheduledEvent {
    fn clone(&self) -> Self {
        Self {
            cron: self.cron.clone(),
            description: self.description.clone(),
            timezone: self.timezone.clone(),
            callback: self.callback.clone(),
            even_in_maintenance: self.even_in_maintenance,
            on_one_server: self.on_one_server,
            run_in_background: self.run_in_background,
        }
    }
}

impl std::fmt::Debug for ScheduledEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScheduledEvent")
            .field("cron", &self.cron)
            .field("description", &self.description)
            .field("timezone", &self.timezone)
            .field("even_in_maintenance", &self.even_in_maintenance)
            .field("on_one_server", &self.on_one_server)
            .field("run_in_background", &self.run_in_background)
            .finish()
    }
}

impl ScheduledEvent {
    pub fn new(cron: CronExpression, description: &str) -> Self {
        Self {
            cron,
            description: description.to_string(),
            timezone: None,
            callback: None,
            even_in_maintenance: false,
            on_one_server: false,
            run_in_background: false,
        }
    }

    pub fn is_due(&self, dt: &chrono::DateTime<chrono::Local>) -> bool {
        self.cron.is_due(dt)
    }

    pub async fn run(&self) -> Result<(), JobError> {
        if let Some(cb) = &self.callback {
            cb().await
        } else {
            Err(JobError::Queue("No callback or job registered".to_string()))
        }
    }

    pub fn even_in_maintenance(mut self) -> Self {
        self.even_in_maintenance = true;
        self
    }

    pub fn on_one_server(mut self) -> Self {
        self.on_one_server = true;
        self
    }

    pub fn run_in_background(mut self) -> Self {
        self.run_in_background = true;
        self
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}
