pub mod batches;
pub mod database;
pub mod failed;
pub mod manager;
pub mod memory;
pub mod sync;
pub mod worker;

pub use batches::{batch, JobBatch, JobBatchStore, PendingBatch};
pub use database::{DatabaseQueue, JobResolver};
pub use failed::{FailedJob, FailedJobStore};
pub use manager::QueueManager;
pub use memory::InMemoryQueue;
pub use sync::SyncQueue;
pub use worker::QueueWorker;

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Job failed: {0}")]
    Failed(String),
    #[error("Queue error: {0}")]
    Queue(String),
}

#[async_trait]
pub trait ShouldQueue: Send + Sync + std::fmt::Debug {
    async fn handle(&self) -> Result<(), JobError>;
    fn name(&self) -> &str;
    /// Maximum attempts before the job is considered permanently failed.
    /// `None` falls back to the worker default of 3 attempts.
    fn max_attempts(&self) -> Option<u64> {
        None
    }
    /// Seconds to wait before releasing the job for another attempt.
    fn backoff_seconds(&self) -> Option<u64> {
        None
    }
    /// Maximum seconds a single attempt may run before it is aborted.
    /// `None` disables the timeout.
    fn timeout_seconds(&self) -> Option<u64> {
        None
    }
    /// Treat a timeout as a permanent failure instead of a retryable one.
    fn fail_on_timeout(&self) -> bool {
        false
    }
    /// Seconds to wait before the job becomes available, mirroring Laravel's
    /// `#[Delay]` attribute. `None` (the default) dispatches immediately.
    fn delay_seconds(&self) -> Option<u64> {
        None
    }
}

pub type JobBox = Box<dyn ShouldQueue>;

#[async_trait]
pub trait Queue: Send + Sync + std::fmt::Debug {
    async fn push(&self, job: JobBox) -> Result<(), JobError>;
    /// Push a job that must not be processed before `delay_seconds` have
    /// elapsed. The default implementation pushes immediately; backends
    /// that support delayed delivery override this.
    async fn push_delayed(&self, job: JobBox, delay_seconds: u64) -> Result<(), JobError> {
        let _ = delay_seconds;
        self.push(job).await
    }
    /// Pop the next available job together with the number of attempts it has
    /// already consumed (including this one).
    async fn pop(&self) -> Option<(JobBox, u64)>;
    /// Requeue a job after a failed attempt, optionally delaying it by
    /// `delay_seconds`. The default implementation re-pushes immediately.
    async fn release(
        &self,
        job: JobBox,
        attempts: u64,
        delay_seconds: u64,
    ) -> Result<(), JobError> {
        let _ = (attempts, delay_seconds);
        self.push(job).await
    }
    /// Mark a job as permanently failed. `exception` carries the failure
    /// reason (mirrors Laravel's `queue.failer` recording). The default
    /// implementation drops the job.
    async fn fail(&self, job: JobBox, exception: String) -> Result<(), JobError> {
        let _ = (job, exception);
        Ok(())
    }
    /// Called by the worker after a job finishes successfully. Queues that
    /// track batch progress (e.g. [`DatabaseQueue`]) override this.
    async fn job_succeeded(&self, _job: JobBox) -> Result<(), JobError> {
        Ok(())
    }
    async fn count(&self) -> usize;
    fn name(&self) -> &str;
}

pub async fn dispatch<J: ShouldQueue + 'static>(job: J) -> Result<(), JobError> {
    let sync_queue = SyncQueue::new("sync");
    let boxed: JobBox = Box::new(job);
    sync_queue.push(boxed).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct TestJob {
        name: String,
        handled: Arc<AtomicBool>,
    }

    #[async_trait]
    impl ShouldQueue for TestJob {
        async fn handle(&self) -> Result<(), JobError> {
            self.handled.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[allow(dead_code)]
    #[derive(Debug)]
    struct CountingJob {
        counter: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl ShouldQueue for CountingJob {
        async fn handle(&self) -> Result<(), JobError> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn name(&self) -> &str {
            "counting"
        }
    }

    #[derive(Debug)]
    struct FailingJob;

    #[async_trait]
    impl ShouldQueue for FailingJob {
        async fn handle(&self) -> Result<(), JobError> {
            Err(JobError::Failed("intentional failure".to_string()))
        }
        fn name(&self) -> &str {
            "failing"
        }
    }

    #[tokio::test]
    async fn test_sync_queue_executes_immediately() {
        let handled = Arc::new(AtomicBool::new(false));
        let job = TestJob {
            name: "test".to_string(),
            handled: handled.clone(),
        };
        let queue = SyncQueue::new("sync");
        queue.push(Box::new(job)).await.unwrap();
        assert!(handled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_sync_queue_pop_returns_none() {
        let queue = SyncQueue::new("sync");
        assert!(queue.pop().await.is_none());
        assert_eq!(queue.count().await, 0);
    }

    #[tokio::test]
    async fn test_in_memory_queue_push_and_pop() {
        let queue = InMemoryQueue::new("memory");
        let handled = Arc::new(AtomicBool::new(false));
        let job = TestJob {
            name: "mem".to_string(),
            handled: handled.clone(),
        };
        queue.push(Box::new(job)).await.unwrap();
        assert_eq!(queue.count().await, 1);

        let popped = queue.pop().await;
        assert!(popped.is_some());
        assert_eq!(queue.count().await, 0);

        popped.unwrap().0.handle().await.unwrap();
        assert!(handled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_in_memory_queue_fifo_order() {
        let queue = InMemoryQueue::new("fifo");
        let counter = Arc::new(AtomicUsize::new(0));
        let _c1 = counter.clone();
        let _c2 = counter.clone();

        #[derive(Debug)]
        struct OrderedJob {
            id: usize,
            results: Arc<Mutex<Vec<usize>>>,
        }
        #[async_trait]
        impl ShouldQueue for OrderedJob {
            async fn handle(&self) -> Result<(), JobError> {
                let mut results = self.results.lock().unwrap();
                results.push(self.id);
                Ok(())
            }
            fn name(&self) -> &str {
                "ordered"
            }
        }

        let results = Arc::new(Mutex::new(Vec::new()));
        queue
            .push(Box::new(OrderedJob {
                id: 1,
                results: results.clone(),
            }))
            .await
            .unwrap();
        queue
            .push(Box::new(OrderedJob {
                id: 2,
                results: results.clone(),
            }))
            .await
            .unwrap();

        let job1 = queue.pop().await.unwrap();
        job1.0.handle().await.unwrap();
        let job2 = queue.pop().await.unwrap();
        job2.0.handle().await.unwrap();

        let r = results.lock().unwrap();
        assert_eq!(*r, vec![1, 2]);
    }

    #[tokio::test]
    async fn test_in_memory_queue_empty_pop() {
        let queue = InMemoryQueue::new("empty");
        assert!(queue.pop().await.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_queue_count() {
        let queue = InMemoryQueue::new("count");
        assert_eq!(queue.count().await, 0);

        let handled = Arc::new(AtomicBool::new(false));
        queue
            .push(Box::new(TestJob {
                name: "j1".to_string(),
                handled: handled.clone(),
            }))
            .await
            .unwrap();
        queue
            .push(Box::new(TestJob {
                name: "j2".to_string(),
                handled: handled.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(queue.count().await, 2);

        queue.pop().await;
        assert_eq!(queue.count().await, 1);
    }

    #[tokio::test]
    async fn test_queue_worker_process() {
        let queue = Arc::new(InMemoryQueue::new("worker"));
        let handled = Arc::new(AtomicBool::new(false));
        queue
            .push(Box::new(TestJob {
                name: "w".to_string(),
                handled: handled.clone(),
            }))
            .await
            .unwrap();

        let worker = QueueWorker::new(queue);
        worker.process_next_job().await;

        assert!(handled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_queue_worker_work_once() {
        let queue = Arc::new(InMemoryQueue::new("once"));
        let handled = Arc::new(AtomicBool::new(false));
        queue
            .push(Box::new(TestJob {
                name: "once".to_string(),
                handled: handled.clone(),
            }))
            .await
            .unwrap();

        let worker = QueueWorker::new(queue);
        worker.work_once().await.unwrap();

        assert!(handled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_queue_worker_work_once_empty() {
        let queue = Arc::new(InMemoryQueue::new("empty-once"));
        let worker = QueueWorker::new(queue);
        let result = worker.work_once().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_manager() {
        let mut manager = QueueManager::new("default");
        manager.register("default", SyncQueue::new("default"));
        manager.register("memory", InMemoryQueue::new("memory"));

        let default = manager.default_queue().unwrap();
        assert_eq!(default.name(), "default");

        let q = manager.queue("memory").unwrap();
        assert_eq!(q.name(), "memory");
    }

    #[tokio::test]
    async fn test_queue_manager_missing() {
        let manager = QueueManager::new("default");
        let result = manager.queue("nonexistent");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_manager_names() {
        let mut manager = QueueManager::new("default");
        manager.register("default", SyncQueue::new("default"));
        manager.register("redis", SyncQueue::new("redis"));
        let mut names = manager.queue_names();
        names.sort();
        assert_eq!(names, vec!["default", "redis"]);
    }

    #[tokio::test]
    async fn test_queue_manager_set_default() {
        let mut manager = QueueManager::new("first");
        manager.register("first", SyncQueue::new("first"));
        manager.register("second", SyncQueue::new("second"));
        manager.set_default("second");
        assert_eq!(manager.default_name(), "second");
    }

    #[tokio::test]
    async fn test_queue_manager_route_resolution() {
        let mut manager = QueueManager::new("default");
        manager.register("default", SyncQueue::new("default"));
        manager.register("redis", InMemoryQueue::new("redis"));

        manager.route("ProcessPodcast", "redis");
        let routed = manager.routed_queue("ProcessPodcast").unwrap();
        assert_eq!(routed.name(), "redis");

        let unrouted = manager.routed_queue("SomeOtherJob").unwrap();
        assert_eq!(unrouted.name(), "default");
    }

    #[tokio::test]
    async fn test_queue_manager_route_to_missing_queue_errors() {
        let mut manager = QueueManager::new("default");
        manager.register("default", SyncQueue::new("default"));
        manager.route("OrphanJob", "nonexistent");

        let result = manager.routed_queue("OrphanJob");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_manager_unroute() {
        let mut manager = QueueManager::new("default");
        manager.register("default", SyncQueue::new("default"));
        manager.register("redis", InMemoryQueue::new("redis"));

        manager.route("JobA", "redis");
        manager.unroute("JobA");
        assert_eq!(manager.routed_queue("JobA").unwrap().name(), "default");
    }

    #[tokio::test]
    async fn test_queue_manager_dispatch_respects_route() {
        let mut manager = QueueManager::new("default");
        manager.register("default", InMemoryQueue::new("default"));
        manager.register("redis", InMemoryQueue::new("redis"));

        let handled = Arc::new(AtomicBool::new(false));
        manager.route("routed_job", "redis");

        manager
            .dispatch(TestJob {
                name: "routed_job".to_string(),
                handled: handled.clone(),
            })
            .await
            .unwrap();

        let redis_queue = manager.queue("redis").unwrap();
        assert_eq!(redis_queue.count().await, 1);
        assert_eq!(manager.queue("default").unwrap().count().await, 0);

        let popped = redis_queue.pop().await.unwrap();
        popped.0.handle().await.unwrap();
        assert!(handled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_queue_manager_dispatch_default_when_unrouted() {
        let mut manager = QueueManager::new("default");
        manager.register("default", InMemoryQueue::new("default"));
        manager.register("redis", InMemoryQueue::new("redis"));

        let handled = Arc::new(AtomicBool::new(false));
        manager
            .dispatch(TestJob {
                name: "plain_job".to_string(),
                handled: handled.clone(),
            })
            .await
            .unwrap();

        assert_eq!(manager.queue("default").unwrap().count().await, 1);
        assert_eq!(manager.queue("redis").unwrap().count().await, 0);
    }

    #[tokio::test]
    async fn test_dispatch_function() {
        let handled = Arc::new(AtomicBool::new(false));
        let job = TestJob {
            name: "dispatch".to_string(),
            handled: handled.clone(),
        };
        dispatch(job).await.unwrap();
        assert!(handled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_sync_queue_failing_job() {
        let queue = SyncQueue::new("fail");
        let result = queue.push(Box::new(FailingJob)).await;
        assert!(result.is_err());
        match result {
            Err(JobError::Failed(msg)) => assert_eq!(msg, "intentional failure"),
            _ => panic!("Expected JobError::Failed"),
        }
    }

    #[tokio::test]
    async fn test_in_memory_queue_name() {
        let queue = InMemoryQueue::new("my-queue");
        assert_eq!(queue.name(), "my-queue");
    }

    #[tokio::test]
    async fn test_sync_queue_name() {
        let queue = SyncQueue::new("my-sync");
        assert_eq!(queue.name(), "my-sync");
    }

    #[tokio::test]
    async fn test_queue_worker_stop() {
        let queue = Arc::new(InMemoryQueue::new("stop"));
        let worker = QueueWorker::new(queue);
        assert!(worker.is_running());
        worker.stop();
        assert!(!worker.is_running());
    }

    async fn setup_db_queue(name: &str) -> (DatabaseQueue, Arc<AtomicBool>) {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let handled = Arc::new(AtomicBool::new(false));
        let h = handled.clone();

        let resolver: JobResolver = Arc::new(move |class, _payload| {
            if class == "test_db_job" {
                Some(Box::new(TestJob {
                    name: "test_db_job".to_string(),
                    handled: h.clone(),
                }) as JobBox)
            } else {
                None
            }
        });

        let queue = DatabaseQueue::new(name, db.clone(), resolver);
        queue.ensure_table_exists().await.unwrap();
        (queue, handled)
    }

    async fn setup_db_queue_simple(name: &str) -> DatabaseQueue {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let resolver: JobResolver = Arc::new(|class, _payload| {
            if class == "failing" {
                Some(Box::new(FailingJob) as JobBox)
            } else {
                None
            }
        });

        let queue = DatabaseQueue::new(name, db.clone(), resolver);
        queue.ensure_table_exists().await.unwrap();
        queue
    }

    #[tokio::test]
    async fn test_db_queue_push_and_pop() {
        let (queue, handled) = setup_db_queue("db-test").await;
        let job = TestJob {
            name: "test_db_job".to_string(),
            handled: handled.clone(),
        };
        queue.push(Box::new(job)).await.unwrap();
        assert_eq!(queue.count().await, 1);

        let popped = queue.pop().await;
        assert!(popped.is_some());

        popped.unwrap().0.handle().await.unwrap();
        assert!(handled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_db_queue_empty_pop() {
        let queue = setup_db_queue_simple("db-empty").await;
        assert!(queue.pop().await.is_none());
    }

    #[tokio::test]
    async fn test_db_queue_name() {
        let queue = setup_db_queue_simple("db-name").await;
        assert_eq!(queue.name(), "db-name");
    }

    #[tokio::test]
    async fn test_db_queue_count_multiple() {
        let (queue, handled) = setup_db_queue("db-count").await;
        for i in 0..3 {
            let job = TestJob {
                name: format!("test_db_job_{}", i),
                handled: handled.clone(),
            };
            queue.push(Box::new(job)).await.unwrap();
        }
        assert_eq!(queue.count().await, 3);
    }

    #[tokio::test]
    async fn test_db_queue_push_count_multiple() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let resolver: JobResolver = Arc::new(|class, _payload| {
            if class == "test_job" {
                let handled = Arc::new(AtomicBool::new(false));
                Some(Box::new(TestJob {
                    name: "test_job".to_string(),
                    handled,
                }) as JobBox)
            } else {
                None
            }
        });

        let queue = DatabaseQueue::new("db-fifo", db.clone(), resolver);
        queue.ensure_table_exists().await.unwrap();

        use sea_orm::ConnectionTrait;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        for i in 0..3 {
            let payload = serde_json::json!({"i": i}).to_string();
            db.execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO jobs (queue, payload, class, attempts, available_at, created_at) VALUES (?1, ?2, ?3, 0, ?4, ?4)",
                ["db-fifo".into(), payload.into(), "test_job".into(), now.into()],
            ))
            .await
            .unwrap();
        }

        assert_eq!(queue.count().await, 3);
    }

    #[tokio::test]
    async fn test_db_queue_resolver_returns_none() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let resolver: JobResolver = Arc::new(|_class, _payload| None);
        let queue = DatabaseQueue::new("db-none", db.clone(), resolver);
        queue.ensure_table_exists().await.unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        use sea_orm::ConnectionTrait;
        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO jobs (queue, payload, class, attempts, available_at, created_at) VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            ["db-none".into(), "{}".into(), "unknown".into(), now.into()],
        ))
        .await
        .unwrap();

        let popped = queue.pop().await;
        assert!(popped.is_none());
    }

    #[tokio::test]
    async fn test_db_queue_with_table_name() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let resolver: JobResolver = Arc::new(|_, _| None);
        let queue = DatabaseQueue::new("custom", db.clone(), resolver).with_table("custom_jobs");
        queue.ensure_table_exists().await.unwrap();

        use sea_orm::ConnectionTrait;
        let result = db
            .execute(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type='table' AND name='custom_jobs'"
                    .to_string(),
            ))
            .await;
        assert!(result.is_ok());
    }

    // --- #[job] attribute macro tests ---

    use crate::job;

    #[tokio::test]
    async fn test_job_macro_no_params() {
        #[job]
        async fn simple_job() -> Result<(), JobError> {
            Ok(())
        }

        let result = SimpleJob::new().dispatch().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_job_macro_with_params() {
        static JOB_DATA: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

        #[job]
        async fn process_data(key: String, value: i32) -> Result<(), JobError> {
            let mut data = JOB_DATA.lock().unwrap();
            *data = Some(format!("{}_{}", key, value));
            Ok(())
        }

        ProcessDataJob::new("hello".to_string(), 42)
            .dispatch()
            .await
            .unwrap();

        let data = JOB_DATA.lock().unwrap();
        assert_eq!(data.as_deref(), Some("hello_42"));
    }

    #[tokio::test]
    async fn test_job_macro_name() {
        #[job]
        async fn my_named_job() -> Result<(), JobError> {
            Ok(())
        }

        let job = MyNamedJob::new();
        assert_eq!(<MyNamedJob as ShouldQueue>::name(&job), "my_named_job");
    }

    #[tokio::test]
    async fn test_job_macro_struct_is_debug() {
        #[job]
        async fn debug_job(count: u64) -> Result<(), JobError> {
            let _ = count;
            Ok(())
        }

        let job = DebugJob::new(7);
        let debug_str = format!("{:?}", job);
        assert!(debug_str.contains("count: 7"));
    }

    #[tokio::test]
    async fn test_job_macro_failing_job() {
        #[job]
        async fn failing_job() -> Result<(), JobError> {
            Err(JobError::Failed("oops".to_string()))
        }

        let result = FailingJob::new().dispatch().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), JobError::Failed(_)));
    }

    // --- #[job] attribute macro tests (tries/backoff/timeout/fail_on_timeout) ---

    #[tokio::test]
    async fn test_job_macro_attributes() {
        #[job(tries = 5, backoff = 10, timeout = 30, fail_on_timeout)]
        async fn attr_job() -> Result<(), JobError> {
            Ok(())
        }

        let job = AttrJob::new();
        assert_eq!(<AttrJob as ShouldQueue>::max_attempts(&job), Some(5));
        assert_eq!(<AttrJob as ShouldQueue>::backoff_seconds(&job), Some(10));
        assert_eq!(<AttrJob as ShouldQueue>::timeout_seconds(&job), Some(30));
        assert!(<AttrJob as ShouldQueue>::fail_on_timeout(&job));
    }

    #[tokio::test]
    async fn test_job_macro_attribute_defaults() {
        #[job]
        async fn plain_job() -> Result<(), JobError> {
            Ok(())
        }

        let job = PlainJob::new();
        assert_eq!(<PlainJob as ShouldQueue>::max_attempts(&job), None);
        assert_eq!(<PlainJob as ShouldQueue>::backoff_seconds(&job), None);
        assert_eq!(<PlainJob as ShouldQueue>::timeout_seconds(&job), None);
        assert!(!<PlainJob as ShouldQueue>::fail_on_timeout(&job));
        assert_eq!(<PlainJob as ShouldQueue>::delay_seconds(&job), None);
    }

    #[tokio::test]
    async fn test_job_macro_delay_attribute() {
        #[job(delay = 60)]
        async fn delayed_job() -> Result<(), JobError> {
            Ok(())
        }

        let job = DelayedJob::new();
        assert_eq!(<DelayedJob as ShouldQueue>::delay_seconds(&job), Some(60));
    }

    #[tokio::test]
    async fn test_queue_manager_dispatch_honors_delay() {
        let mut manager = QueueManager::new("default");
        let queue = InMemoryQueue::new("default");
        manager.register("default", queue.clone());
        manager.set_default("default");

        #[job(delay = 3600)]
        async fn later_job() -> Result<(), JobError> {
            Ok(())
        }

        manager.dispatch(LaterJob::new()).await.unwrap();
        // Not available yet — pop must return None and keep the job queued.
        assert!(queue.pop().await.is_none());
        assert_eq!(queue.count().await, 1);
    }

    #[tokio::test]
    async fn test_job_macro_partial_attributes() {
        #[job(tries = 1)]
        async fn limited_job() -> Result<(), JobError> {
            Ok(())
        }

        let job = LimitedJob::new();
        assert_eq!(<LimitedJob as ShouldQueue>::max_attempts(&job), Some(1));
        assert_eq!(<LimitedJob as ShouldQueue>::backoff_seconds(&job), None);
    }

    #[tokio::test]
    async fn test_worker_retries_then_succeeds() {
        static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

        #[job(tries = 3, backoff = 0)]
        async fn flaky_job() -> Result<(), JobError> {
            let attempt = ATTEMPTS.fetch_add(1, Ordering::SeqCst);
            if attempt < 2 {
                Err(JobError::Failed("flaky".to_string()))
            } else {
                Ok(())
            }
        }

        let queue = Arc::new(InMemoryQueue::new("retry"));
        queue.push(Box::new(FlakyJob::new())).await.unwrap();

        let worker = QueueWorker::new(queue.clone());
        assert!(worker.process_next_job().await.unwrap().is_ok());
        assert!(worker.process_next_job().await.unwrap().is_ok());
        assert!(worker.process_next_job().await.unwrap().is_ok());
        assert_eq!(ATTEMPTS.load(Ordering::SeqCst), 3);
        assert_eq!(queue.count().await, 0);
    }

    #[tokio::test]
    async fn test_worker_permanent_failure_after_tries() {
        static FAILED_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

        #[job(tries = 2)]
        async fn always_fails() -> Result<(), JobError> {
            FAILED_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
            Err(JobError::Failed("always".to_string()))
        }

        let queue = Arc::new(InMemoryQueue::new("give-up"));
        queue.push(Box::new(AlwaysFailsJob::new())).await.unwrap();

        let worker = QueueWorker::new(queue.clone());
        let first = worker.process_next_job().await.unwrap();
        assert!(first.is_ok(), "attempt 1 should be released for retry");
        let second = worker.process_next_job().await.unwrap();
        assert!(second.is_err(), "attempt 2 should fail permanently");

        assert_eq!(FAILED_ATTEMPTS.load(Ordering::SeqCst), 2);
        assert_eq!(queue.count().await, 0);
    }

    #[tokio::test]
    async fn test_worker_backoff_delays_retry() {
        #[job(tries = 3, backoff = 30)]
        async fn delayed_job() -> Result<(), JobError> {
            Err(JobError::Failed("backoff".to_string()))
        }

        let queue = Arc::new(InMemoryQueue::new("backoff"));
        queue.push(Box::new(DelayedJob::new())).await.unwrap();

        let worker = QueueWorker::new(queue.clone());
        let result = worker.process_next_job().await.unwrap();
        assert!(result.is_ok(), "failed attempt should be released");

        assert_eq!(queue.count().await, 1, "job should be pending");
        assert!(
            queue.pop().await.is_none(),
            "job must not be available before the backoff elapses"
        );
    }

    #[tokio::test]
    async fn test_worker_timeout_releases_job() {
        #[job(timeout = 0)]
        async fn slow_job() -> Result<(), JobError> {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            Ok(())
        }

        let queue = Arc::new(InMemoryQueue::new("timeout"));
        queue.push(Box::new(SlowJob::new())).await.unwrap();

        let worker = QueueWorker::new(queue.clone());
        let result = worker.process_next_job().await.unwrap();
        assert!(
            result.is_ok(),
            "timeout without fail_on_timeout should release"
        );
        assert_eq!(queue.count().await, 1);
    }

    #[tokio::test]
    async fn test_worker_timeout_fail_on_timeout() {
        #[job(timeout = 0, fail_on_timeout)]
        async fn slow_fatal_job() -> Result<(), JobError> {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            Ok(())
        }

        let queue = Arc::new(InMemoryQueue::new("timeout-fatal"));
        queue.push(Box::new(SlowFatalJob::new())).await.unwrap();

        let worker = QueueWorker::new(queue.clone());
        let result = worker.process_next_job().await.unwrap();
        assert!(result.is_err(), "fail_on_timeout should fail permanently");
        assert_eq!(queue.count().await, 0);
    }

    #[tokio::test]
    async fn test_db_queue_tries_limit() {
        #[job(tries = 1)]
        async fn one_shot() -> Result<(), JobError> {
            Ok(())
        }

        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let resolver: JobResolver = Arc::new(|class, _payload| {
            if class == "one_shot" {
                Some(Box::new(OneShotJob::new()) as JobBox)
            } else {
                None
            }
        });

        let queue = DatabaseQueue::new("db-tries", db.clone(), resolver);
        queue.ensure_table_exists().await.unwrap();

        queue.push(Box::new(OneShotJob::new())).await.unwrap();
        let (_, attempts) = queue.pop().await.unwrap();
        assert_eq!(attempts, 1);
        assert!(
            queue.pop().await.is_none(),
            "job with tries=1 must not be popped again"
        );
        assert_eq!(queue.count().await, 0);
    }

    #[tokio::test]
    async fn test_db_queue_failed_job_recorded_in_failed_jobs_table() {
        #[job(tries = 1)]
        async fn doomed() -> Result<(), JobError> {
            Err(JobError::Failed("irrecoverable".to_string()))
        }

        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let resolver: JobResolver = Arc::new(|class, _payload| {
            if class == "doomed" {
                Some(Box::new(DoomedJob::new()) as JobBox)
            } else {
                None
            }
        });

        let queue = Arc::new(DatabaseQueue::new("db-fail", db.clone(), resolver));
        queue.ensure_table_exists().await.unwrap();
        queue.push(Box::new(DoomedJob::new())).await.unwrap();

        let worker = QueueWorker::new(queue.clone());
        let result = worker.process_next_job().await.unwrap();
        assert!(result.is_err(), "tries=1 must fail permanently");

        let store = FailedJobStore::new(db.clone());
        assert_eq!(store.count().await, 1, "failure must be recorded");
        let failed = store.all().await.unwrap();
        assert_eq!(failed[0].class, "doomed");
        assert!(
            failed[0].exception.contains("irrecoverable"),
            "exception message must be stored: {}",
            failed[0].exception
        );
        assert_eq!(queue.count().await, 0, "failed job row must be removed");

        store.requeue("jobs", &failed[0]).await.unwrap();
        assert_eq!(store.count().await, 0, "requeue must forget the failure");
        assert_eq!(queue.count().await, 1, "requeued job must be pending");
    }
}
