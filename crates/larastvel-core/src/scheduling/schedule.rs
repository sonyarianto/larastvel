use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use super::cron::parse_cron;
use super::event::ScheduledEvent;
use super::SchedulingError;
use crate::queue::{JobError, ShouldQueue};

type JobCallback =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), JobError>> + Send>> + Send + Sync>;

#[derive(Debug, Clone)]
pub struct Schedule {
    events: Arc<Mutex<Vec<ScheduledEvent>>>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn call<F, Fut>(&self, cron: &str, description: &str, f: F) -> Result<(), SchedulingError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), JobError>> + Send + 'static,
    {
        let cron_expr = parse_cron(cron).map_err(|e| format!("Invalid cron expression: {}", e))?;
        let mut event = ScheduledEvent::new(cron_expr, description);
        let cb: JobCallback = Arc::new(move || Box::pin(f()));
        event.callback = Some(cb);
        self.events.lock().unwrap().push(event);
        Ok(())
    }

    pub fn job(
        &self,
        cron: &str,
        description: &str,
        job: Box<dyn ShouldQueue>,
    ) -> Result<(), SchedulingError> {
        let cron_expr = parse_cron(cron).map_err(|e| format!("Invalid cron expression: {}", e))?;
        let mut event = ScheduledEvent::new(cron_expr, description);
        let shared = Arc::new(job);
        let cb: JobCallback = Arc::new(move || {
            let shared = shared.clone();
            Box::pin(async move { shared.handle().await })
        });
        event.callback = Some(cb);
        self.events.lock().unwrap().push(event);
        Ok(())
    }

    pub fn push(&self, event: ScheduledEvent) {
        self.events.lock().unwrap().push(event);
    }

    pub fn cron(&self, cron: &str) -> EventBuilder {
        EventBuilder::new(self.clone(), cron)
    }

    pub fn events(&self) -> Vec<ScheduledEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventBuilder {
    schedule: Schedule,
    cron: String,
    description: String,
}

impl EventBuilder {
    fn new(schedule: Schedule, cron: &str) -> Self {
        Self {
            schedule,
            cron: cron.to_string(),
            description: String::new(),
        }
    }

    pub fn description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn call<F, Fut>(self, f: F) -> Result<(), SchedulingError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), JobError>> + Send + 'static,
    {
        self.schedule.call(&self.cron, &self.description, f)
    }

    pub fn job(self, job: Box<dyn ShouldQueue>) -> Result<(), SchedulingError> {
        self.schedule.job(&self.cron, &self.description, job)
    }
}
