use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
}

pub type JobBox = Box<dyn ShouldQueue>;

#[async_trait]
pub trait Queue: Send + Sync + std::fmt::Debug {
    async fn push(&self, job: JobBox) -> Result<(), JobError>;
    async fn pop(&self) -> Option<JobBox>;
    async fn count(&self) -> usize;
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct SyncQueue {
    name: String,
}

impl SyncQueue {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl Queue for SyncQueue {
    async fn push(&self, job: JobBox) -> Result<(), JobError> {
        tracing::debug!("[SyncQueue] Executing job: {}", job.name());
        job.handle().await
    }

    async fn pop(&self) -> Option<JobBox> {
        None
    }

    async fn count(&self) -> usize {
        0
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct InMemoryQueue {
    name: String,
    jobs: Arc<Mutex<VecDeque<JobBox>>>,
    processed: Arc<AtomicUsize>,
}

impl InMemoryQueue {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            jobs: Arc::new(Mutex::new(VecDeque::new())),
            processed: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn processed_count(&self) -> usize {
        self.processed.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Queue for InMemoryQueue {
    async fn push(&self, job: JobBox) -> Result<(), JobError> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.push_back(job);
        Ok(())
    }

    async fn pop(&self) -> Option<JobBox> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.pop_front()
    }

    async fn count(&self) -> usize {
        let jobs = self.jobs.lock().unwrap();
        jobs.len()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct QueueWorker {
    queue: Arc<dyn Queue>,
    running: Arc<Mutex<bool>>,
}

impl QueueWorker {
    pub fn new(queue: Arc<dyn Queue>) -> Self {
        Self {
            queue,
            running: Arc::new(Mutex::new(true)),
        }
    }

    pub fn queue(&self) -> &Arc<dyn Queue> {
        &self.queue
    }

    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    pub fn is_running(&self) -> bool {
        let running = self.running.lock().unwrap();
        *running
    }

    pub async fn process_next_job(&self) -> Option<Result<(), JobError>> {
        if let Some(job) = self.queue.pop().await {
            Some(job.handle().await)
        } else {
            None
        }
    }

    pub async fn work(&self) {
        loop {
            let should_run = {
                let running = self.running.lock().unwrap();
                *running
            };
            if !should_run {
                break;
            }
            if let Some(job) = self.queue.pop().await {
                if let Err(e) = job.handle().await {
                    tracing::error!("[QueueWorker] Job failed: {} - {}", job.name(), e);
                }
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    pub async fn work_once(&self) -> Result<(), JobError> {
        if let Some(job) = self.queue.pop().await {
            job.handle().await
        } else {
            Err(JobError::Queue("No jobs in queue".to_string()))
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueManager {
    queues: HashMap<String, Arc<dyn Queue>>,
    default: String,
}

impl QueueManager {
    pub fn new(default: &str) -> Self {
        Self {
            queues: HashMap::new(),
            default: default.to_string(),
        }
    }

    pub fn register<Q: Queue + 'static>(&mut self, name: &str, queue: Q) {
        self.queues.insert(name.to_string(), Arc::new(queue));
    }

    pub fn queue(&self, name: &str) -> Result<Arc<dyn Queue>, JobError> {
        self.queues
            .get(name)
            .cloned()
            .ok_or_else(|| JobError::Queue(format!("Queue [{}] not configured", name)))
    }

    pub fn default_queue(&self) -> Result<Arc<dyn Queue>, JobError> {
        self.queue(&self.default)
    }

    pub fn set_default(&mut self, name: &str) {
        self.default = name.to_string();
    }

    pub fn default_name(&self) -> &str {
        &self.default
    }

    pub fn queue_names(&self) -> Vec<String> {
        self.queues.keys().cloned().collect()
    }
}

pub async fn dispatch<J: ShouldQueue + 'static>(job: J) -> Result<(), JobError> {
    let sync_queue = SyncQueue::new("sync");
    let boxed: JobBox = Box::new(job);
    sync_queue.push(boxed).await
}

pub type JobResolver = Arc<dyn Fn(&str, &str) -> Option<JobBox> + Send + Sync>;

pub struct DatabaseQueue {
    name: String,
    table_name: String,
    db: sea_orm::DatabaseConnection,
    resolver: JobResolver,
}

impl std::fmt::Debug for DatabaseQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseQueue")
            .field("name", &self.name)
            .field("table_name", &self.table_name)
            .field("db", &self.db)
            .field("resolver", &"<closure>")
            .finish()
    }
}

impl Clone for DatabaseQueue {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            table_name: self.table_name.clone(),
            db: self.db.clone(),
            resolver: self.resolver.clone(),
        }
    }
}

impl DatabaseQueue {
    pub fn new(name: &str, db: sea_orm::DatabaseConnection, resolver: JobResolver) -> Self {
        Self {
            name: name.to_string(),
            table_name: "jobs".to_string(),
            db,
            resolver,
        }
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table_name = table.to_string();
        self
    }

    pub async fn ensure_table_exists(&self) -> Result<(), JobError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue TEXT NOT NULL DEFAULT 'default',
                payload TEXT NOT NULL,
                class TEXT NOT NULL DEFAULT '',
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 3,
                reserved_at INTEGER,
                available_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            self.table_name
        );
        use sea_orm::ConnectionTrait;
        self.db
            .execute(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                sql,
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to create jobs table: {}", e)))?;
        Ok(())
    }
}

#[async_trait]
impl Queue for DatabaseQueue {
    async fn push(&self, job: JobBox) -> Result<(), JobError> {
        let payload = serde_json::to_string(&serde_json::json!({
            "name": job.name(),
        }))
        .map_err(|e| JobError::Queue(format!("Serialization error: {}", e)))?;

        let class = job.name().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sql = format!(
            "INSERT INTO {} (queue, payload, class, attempts, available_at, created_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            self.table_name
        );
        use sea_orm::ConnectionTrait;
        self.db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                &sql,
                [
                    self.name.clone().into(),
                    payload.into(),
                    class.into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to push job: {}", e)))?;
        Ok(())
    }

    async fn pop(&self) -> Option<JobBox> {
        let sql = format!(
            "SELECT id, payload, class FROM {} 
             WHERE queue = ?1 AND (reserved_at IS NULL OR reserved_at < ?2)
             AND attempts < max_attempts
             AND available_at <= ?2
             ORDER BY id ASC LIMIT 1",
            self.table_name
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        use sea_orm::{ConnectionTrait, QueryResult};
        let result: Vec<QueryResult> = self
            .db
            .query_all(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                &sql,
                [self.name.clone().into(), now.into(), now.into()],
            ))
            .await
            .ok()?;

        let row = result.into_iter().next()?;

        let class: String = row.try_get_by_index::<String>(2).ok()?;
        let payload: String = row.try_get_by_index::<String>(1).ok()?;

        let resolver = self.resolver.clone();
        let job = resolver(&class, &payload)?;

        let id: Option<i64> = row.try_get_by_index::<i64>(0).ok();
        if let Some(job_id) = id {
            let update_sql = format!(
                "UPDATE {} SET reserved_at = ?1, attempts = attempts + 1 WHERE id = ?2",
                self.table_name
            );
            let _ = self
                .db
                .execute(sea_orm::Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Sqlite,
                    &update_sql,
                    [now.into(), job_id.into()],
                ))
                .await;
        }

        Some(job)
    }

    async fn count(&self) -> usize {
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM {} WHERE queue = ?1 AND attempts < max_attempts",
            self.table_name
        );
        use sea_orm::ConnectionTrait;
        let result = self
            .db
            .query_one(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                &sql,
                [self.name.clone().into()],
            ))
            .await;

        match result {
            Ok(Some(row)) => {
                let cnt: Option<i64> = row.try_get_by_index(0).ok();
                cnt.unwrap_or(0) as usize
            }
            _ => 0,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

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

        popped.unwrap().handle().await.unwrap();
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
        queue.push(Box::new(OrderedJob { id: 1, results: results.clone() })).await.unwrap();
        queue.push(Box::new(OrderedJob { id: 2, results: results.clone() })).await.unwrap();

        let job1 = queue.pop().await.unwrap();
        job1.handle().await.unwrap();
        let job2 = queue.pop().await.unwrap();
        job2.handle().await.unwrap();

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
        queue.push(Box::new(TestJob { name: "j1".to_string(), handled: handled.clone() })).await.unwrap();
        queue.push(Box::new(TestJob { name: "j2".to_string(), handled: handled.clone() })).await.unwrap();
        assert_eq!(queue.count().await, 2);

        queue.pop().await;
        assert_eq!(queue.count().await, 1);
    }

    #[tokio::test]
    async fn test_queue_worker_process() {
        let queue = Arc::new(InMemoryQueue::new("worker"));
        let handled = Arc::new(AtomicBool::new(false));
        queue.push(Box::new(TestJob { name: "w".to_string(), handled: handled.clone() })).await.unwrap();

        let worker = QueueWorker::new(queue);
        worker.process_next_job().await;

        assert!(handled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_queue_worker_work_once() {
        let queue = Arc::new(InMemoryQueue::new("once"));
        let handled = Arc::new(AtomicBool::new(false));
        queue.push(Box::new(TestJob { name: "once".to_string(), handled: handled.clone() })).await.unwrap();

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

        popped.unwrap().handle().await.unwrap();
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
        let queue = DatabaseQueue::new("custom", db.clone(), resolver)
            .with_table("custom_jobs");
        queue.ensure_table_exists().await.unwrap();

        use sea_orm::ConnectionTrait;
        let result = db
            .execute(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type='table' AND name='custom_jobs'".to_string(),
            ))
            .await;
        assert!(result.is_ok());
    }
}
