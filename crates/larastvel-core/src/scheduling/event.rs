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
        if let Some(tz_name) = &self.timezone {
            if let Ok(tz) = tz_name.parse::<chrono_tz::Tz>() {
                let dt_in_tz = dt.with_timezone(&tz);
                return self.cron.is_due_in(&dt_in_tz);
            }
        }
        self.cron.is_due(dt)
    }

    /// The next time this event will run, evaluated against the current
    /// time. When a timezone is configured (see [`timezone`](Self::timezone)),
    /// the next run is computed in that timezone and returned in local time —
    /// this is what `schedule:list` resolves to display "Next Run".
    pub fn next_run(&self) -> Option<chrono::DateTime<chrono::Local>> {
        let now = chrono::Local::now();
        if let Some(tz_name) = &self.timezone {
            if let Ok(tz) = tz_name.parse::<chrono_tz::Tz>() {
                let now_in_tz = now.with_timezone(&tz);
                return self
                    .cron
                    .next_run_after(now_in_tz)
                    .map(|dt| dt.with_timezone(&chrono::Local));
            }
        }
        self.cron.next_run_after(now)
    }

    /// Set the timezone this event is evaluated in (IANA name, e.g.
    /// `"Asia/Jakarta"`).
    pub fn timezone(mut self, tz: impl Into<String>) -> Self {
        self.timezone = Some(tz.into());
        self
    }

    pub fn timezone_name(&self) -> Option<&str> {
        self.timezone.as_deref()
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
