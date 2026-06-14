use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use chrono::{Datelike, Timelike};

use crate::queue::JobError;

#[derive(Debug, Clone)]
pub struct SchedulingError(pub String);

impl fmt::Display for SchedulingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SchedulingError {}

impl From<String> for SchedulingError {
    fn from(s: String) -> Self {
        SchedulingError(s)
    }
}

pub fn parse_cron(expr: &str) -> Result<CronExpression, SchedulingError> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(format!(
            "Invalid cron expression '{}': expected 5 fields, got {}",
            expr,
            parts.len()
        )
        .into());
    }

    Ok(CronExpression {
        minute: CronField::parse(parts[0], 0, 59)?,
        hour: CronField::parse(parts[1], 0, 23)?,
        day_of_month: CronField::parse(parts[2], 1, 31)?,
        month: CronField::parse(parts[3], 1, 12)?,
        day_of_week: CronField::parse(parts[4], 0, 6)?,
    })
}

#[derive(Debug, Clone)]
pub struct CronExpression {
    minute: CronField,
    hour: CronField,
    day_of_month: CronField,
    month: CronField,
    day_of_week: CronField,
}

impl CronExpression {
    pub fn is_due(&self, dt: &chrono::DateTime<chrono::Local>) -> bool {
        self.minute.matches(dt.minute() as i32)
            && self.hour.matches(dt.hour() as i32)
            && self.day_of_month.matches(dt.day() as i32)
            && self.month.matches(dt.month() as i32)
            && self
                .day_of_week
                .matches(dt.weekday().num_days_from_sunday() as i32)
    }
}

#[derive(Debug, Clone)]
enum CronField {
    All,
    Single(i32),
    List(Vec<i32>),
    Range(i32, i32),
    Step(i32, i32),
    StepRange(i32, i32, i32),
}

impl CronField {
    fn parse(field: &str, min: i32, max: i32) -> Result<Self, SchedulingError> {
        match field {
            "*" => Ok(CronField::All),
            _ if field.contains('/') => {
                let parts: Vec<&str> = field.split('/').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid step expression: {}", field).into());
                }
                let step: i32 = parts[1]
                    .parse()
                    .map_err(|_| format!("Invalid step: {}", parts[1]))?;
                if parts[0] == "*" {
                    Ok(CronField::Step(min, step))
                } else if parts[0].contains('-') {
                    let range_parts: Vec<&str> = parts[0].split('-').collect();
                    if range_parts.len() != 2 {
                        return Err(format!("Invalid step range: {}", field).into());
                    }
                    let start: i32 = range_parts[0]
                        .parse()
                        .map_err(|_| "Invalid range start".to_string())?;
                    let end: i32 = range_parts[1]
                        .parse()
                        .map_err(|_| "Invalid range end".to_string())?;
                    Ok(CronField::StepRange(start, end, step))
                } else {
                    let start: i32 = parts[0]
                        .parse()
                        .map_err(|_| "Invalid step start".to_string())?;
                    Ok(CronField::StepRange(start, max, step))
                }
            }
            _ if field.contains(',') => {
                let values: Result<Vec<i32>, _> =
                    field.split(',').map(|s| s.trim().parse::<i32>()).collect();
                Ok(CronField::List(
                    values.map_err(|_| format!("Invalid list: {}", field))?,
                ))
            }
            _ if field.contains('-') => {
                let parts: Vec<&str> = field.split('-').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid range: {}", field).into());
                }
                let start: i32 = parts[0]
                    .parse()
                    .map_err(|_| "Invalid range start".to_string())?;
                let end: i32 = parts[1]
                    .parse()
                    .map_err(|_| "Invalid range end".to_string())?;
                Ok(CronField::Range(start, end))
            }
            _ => {
                let val: i32 = field
                    .parse()
                    .map_err(|_| format!("Invalid cron value: {}", field))?;
                Ok(CronField::Single(val))
            }
        }
    }

    fn matches(&self, value: i32) -> bool {
        match self {
            CronField::All => true,
            CronField::Single(v) => *v == value,
            CronField::List(values) => values.contains(&value),
            CronField::Range(start, end) => value >= *start && value <= *end,
            CronField::Step(start, step) => value >= *start && (value - start) % step == 0,
            CronField::StepRange(start, end, step) => {
                value >= *start && value <= *end && (value - start) % step == 0
            }
        }
    }
}

type JobCallback =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), JobError>> + Send>> + Send + Sync>;

pub struct ScheduledEvent {
    pub cron: CronExpression,
    pub description: String,
    pub timezone: Option<String>,
    callback: Option<JobCallback>,
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
        job: Box<dyn crate::queue::ShouldQueue>,
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

    pub fn job(self, job: Box<dyn crate::queue::ShouldQueue>) -> Result<(), SchedulingError> {
        self.schedule.job(&self.cron, &self.description, job)
    }
}

#[derive(Debug, Clone)]
pub struct ScheduleManager {
    schedule: Schedule,
}

impl ScheduleManager {
    pub fn new(schedule: Schedule) -> Self {
        Self { schedule }
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn run_due(&self) -> Vec<Result<(), JobError>> {
        let now = chrono::Local::now();
        let events = self.schedule.events();
        let mut results = Vec::new();

        for event in events {
            if event.is_due(&now) {
                let result = tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(event.run());
                results.push(result);
            }
        }

        results
    }

    pub async fn run_due_async(&self) -> Vec<Result<(), JobError>> {
        let now = chrono::Local::now();
        let events = self.schedule.events();
        let mut results = Vec::new();

        for event in events {
            if event.is_due(&now) {
                let result = event.run().await;
                results.push(result);
            }
        }

        results
    }

    pub fn due_events(&self) -> Vec<ScheduledEvent> {
        let now = chrono::Local::now();
        self.schedule
            .events()
            .into_iter()
            .filter(|e| e.is_due(&now))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShouldQueue;
    use chrono::TimeZone;

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32) -> chrono::DateTime<chrono::Local> {
        chrono::Local
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .single()
            .expect("invalid datetime")
    }

    #[test]
    fn test_cron_every_minute() {
        let cron = parse_cron("* * * * *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 0)));
        assert!(cron.is_due(&dt(2025, 6, 15, 12, 30)));
        assert!(cron.is_due(&dt(2025, 12, 31, 23, 59)));
    }

    #[test]
    fn test_cron_specific_minute() {
        let cron = parse_cron("30 * * * *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 30)));
        assert!(cron.is_due(&dt(2025, 1, 1, 5, 30)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 0, 0)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 0, 29)));
    }

    #[test]
    fn test_cron_specific_hour() {
        let cron = parse_cron("0 9 * * *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 1, 9, 0)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 8, 0)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 10, 0)));
    }

    #[test]
    fn test_cron_specific_day() {
        let cron = parse_cron("0 0 15 * *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 15, 0, 0)));
        assert!(!cron.is_due(&dt(2025, 1, 14, 0, 0)));
    }

    #[test]
    fn test_cron_specific_month() {
        let cron = parse_cron("0 0 1 1 *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 0)));
        assert!(!cron.is_due(&dt(2025, 2, 1, 0, 0)));
    }

    #[test]
    fn test_cron_range() {
        let cron = parse_cron("0 9-17 * * *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 1, 9, 0)));
        assert!(cron.is_due(&dt(2025, 1, 1, 12, 0)));
        assert!(cron.is_due(&dt(2025, 1, 1, 17, 0)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 8, 0)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 18, 0)));
    }

    #[test]
    fn test_cron_list() {
        let cron = parse_cron("0,30 * * * *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 0)));
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 30)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 0, 15)));
    }

    #[test]
    fn test_cron_step() {
        let cron = parse_cron("*/15 * * * *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 0)));
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 15)));
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 30)));
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 45)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 0, 10)));
    }

    #[test]
    fn test_cron_step_range() {
        let cron = parse_cron("0-30/15 * * * *").unwrap();
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 0)));
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 15)));
        assert!(cron.is_due(&dt(2025, 1, 1, 0, 30)));
        assert!(!cron.is_due(&dt(2025, 1, 1, 0, 45)));
    }

    #[test]
    fn test_cron_weekday() {
        let cron = parse_cron("0 9 * * 1-5").unwrap();
        // 2025-01-01 is Wednesday (weekday 3)
        assert!(cron.is_due(&dt(2025, 1, 1, 9, 0)));
        // 2025-01-04 is Saturday (weekday 6)
        assert!(!cron.is_due(&dt(2025, 1, 4, 9, 0)));
    }

    #[test]
    fn test_cron_invalid_expression() {
        assert!(parse_cron("invalid").is_err());
        assert!(parse_cron("* * * * * *").is_err());
    }

    #[test]
    fn test_schedule_call() {
        let schedule = Schedule::new();
        let ran = Arc::new(Mutex::new(false));
        let r = ran.clone();

        schedule
            .call("* * * * *", "test task", move || {
                let r = r.clone();
                async move {
                    *r.lock().unwrap() = true;
                    Ok(())
                }
            })
            .unwrap();

        assert_eq!(schedule.events().len(), 1);
        let event = schedule.events().into_iter().next().unwrap();
        assert!(event.is_due(&chrono::Local::now()));
        assert_eq!(event.description(), "test task");
    }

    #[test]
    fn test_schedule_cron_builder() {
        let schedule = Schedule::new();
        let ran = Arc::new(Mutex::new(false));
        let r = ran.clone();

        schedule
            .cron("0 0 * * *")
            .description("midnight task")
            .call(move || {
                let r = r.clone();
                async move {
                    *r.lock().unwrap() = true;
                    Ok(())
                }
            })
            .unwrap();

        assert_eq!(schedule.events().len(), 1);
        let event = schedule.events().into_iter().next().unwrap();
        assert!(!event.is_due(&chrono::Local::now()));
    }

    #[test]
    fn test_schedule_manager_run_due() {
        let schedule = Schedule::new();
        let ran = Arc::new(Mutex::new(false));
        let r = ran.clone();

        schedule
            .call("* * * * *", "every minute", move || {
                let r = r.clone();
                async move {
                    *r.lock().unwrap() = true;
                    Ok(())
                }
            })
            .unwrap();

        let manager = ScheduleManager::new(schedule);
        let results = manager.run_due();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert!(*ran.lock().unwrap());
    }

    #[test]
    fn test_schedule_clear() {
        let schedule = Schedule::new();
        schedule
            .call("* * * * *", "task", || async { Ok(()) })
            .unwrap();
        assert_eq!(schedule.events().len(), 1);
        schedule.clear();
        assert_eq!(schedule.events().len(), 0);
    }

    #[test]
    fn test_schedule_job() {
        use std::sync::atomic::{AtomicBool, Ordering};

        #[derive(Debug)]
        struct TestScheduledJob {
            flag: Arc<AtomicBool>,
        }

        #[async_trait::async_trait]
        impl ShouldQueue for TestScheduledJob {
            async fn handle(&self) -> Result<(), JobError> {
                self.flag.store(true, Ordering::SeqCst);
                Ok(())
            }
            fn name(&self) -> &str {
                "test_scheduled"
            }
        }

        let schedule = Schedule::new();
        let flag = Arc::new(AtomicBool::new(false));
        let job = TestScheduledJob { flag: flag.clone() };

        schedule
            .job("* * * * *", "test job", Box::new(job))
            .unwrap();
        assert_eq!(schedule.events().len(), 1);

        let event = schedule.events().into_iter().next().unwrap();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(event.run());
        assert!(result.is_ok());
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_event_due_and_run() {
        let schedule = Schedule::new();
        let ran = Arc::new(Mutex::new(false));
        let r = ran.clone();

        schedule
            .call("* * * * *", "test", move || {
                let r = r.clone();
                async move {
                    *r.lock().unwrap() = true;
                    Ok(())
                }
            })
            .unwrap();

        let manager = ScheduleManager::new(schedule);
        assert_eq!(manager.due_events().len(), 1);
    }

    #[test]
    fn test_event_is_not_due() {
        let schedule = Schedule::new();
        schedule
            .call("0 0 1 1 0", "yearly", || async { Ok(()) })
            .unwrap();

        let manager = ScheduleManager::new(schedule);
        assert!(manager.due_events().is_empty());
    }

    #[test]
    fn test_scheduled_event_even_in_maintenance() {
        let cron = parse_cron("* * * * *").unwrap();
        let _event = ScheduledEvent::new(cron, "test").even_in_maintenance();
        // just verifying no panic
    }

    #[test]
    fn test_scheduled_event_on_one_server() {
        let cron = parse_cron("* * * * *").unwrap();
        let _event = ScheduledEvent::new(cron, "test").on_one_server();
    }

    #[test]
    fn test_scheduled_event_run_in_background() {
        let cron = parse_cron("* * * * *").unwrap();
        let _event = ScheduledEvent::new(cron, "test").run_in_background();
    }

    #[test]
    fn test_event_without_callback_errors() {
        let cron = parse_cron("* * * * *").unwrap();
        let event = ScheduledEvent::new(cron, "empty");
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(event.run());
        assert!(result.is_err());
    }
}
