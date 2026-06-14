pub mod cron;
pub mod error;
pub mod event;
pub mod schedule;

pub use cron::{parse_cron, CronExpression};
pub use error::SchedulingError;
pub use event::ScheduledEvent;
pub use schedule::{EventBuilder, Schedule};

use crate::queue::JobError;

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
    use std::sync::{Arc, Mutex};

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
        assert!(cron.is_due(&dt(2025, 1, 1, 9, 0)));
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
