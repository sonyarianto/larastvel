use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use super::{JobBox, JobError};

/// A group of jobs dispatched together and tracked as a unit, mirroring
/// Laravel's `Illuminate\Bus\Batch`.
#[derive(Debug, Clone)]
pub struct JobBatch {
    pub id: String,
    pub name: String,
    pub total_jobs: i64,
    pub pending_jobs: i64,
    pub failed_jobs: i64,
    pub cancelled: bool,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub cancelled_at: Option<i64>,
}

impl JobBatch {
    /// Fraction (0.0 - 1.0) of the batch's jobs that have finished.
    pub fn progress(&self) -> f64 {
        if self.total_jobs == 0 {
            return 1.0;
        }
        (self.total_jobs - self.pending_jobs) as f64 / self.total_jobs as f64
    }

    pub fn pending_jobs(&self) -> i64 {
        self.pending_jobs
    }

    pub fn failed_jobs(&self) -> i64 {
        self.failed_jobs
    }

    pub fn total_jobs(&self) -> i64 {
        self.total_jobs
    }

    /// Whether every job in the batch has finished (successfully or not).
    pub fn finished(&self) -> bool {
        self.pending_jobs == 0
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    /// Re-read the batch's current state from the database.
    pub async fn refresh(&self, db: &sea_orm::DatabaseConnection) -> Result<JobBatch, JobError> {
        JobBatchStore::new(db.clone())
            .find(&self.id)
            .await?
            .ok_or_else(|| JobError::Queue(format!("Batch {} not found", self.id)))
    }

    /// Cancel the batch. Pending jobs that have not been reserved yet will be
    /// skipped by the queue worker.
    pub async fn cancel(&self, db: &sea_orm::DatabaseConnection) -> Result<(), JobError> {
        JobBatchStore::new(db.clone()).cancel(&self.id).await
    }
}

/// A batch that has not been dispatched yet. Created with [`batch`].
#[derive(Debug)]
pub struct PendingBatch {
    name: String,
    jobs: Vec<JobBox>,
}

impl PendingBatch {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn add_job(mut self, job: JobBox) -> Self {
        self.jobs.push(job);
        self
    }

    pub fn add_many(mut self, jobs: Vec<JobBox>) -> Self {
        self.jobs.extend(jobs);
        self
    }

    pub(crate) fn name_of(&self) -> &str {
        &self.name
    }

    pub(crate) fn jobs(&self) -> &[JobBox] {
        &self.jobs
    }
}

/// Start a new pending job batch (Laravel's `Bus::batch([...])`). Dispatch it
/// with [`super::DatabaseQueue::dispatch_batch`].
pub fn batch(jobs: Vec<JobBox>) -> PendingBatch {
    PendingBatch {
        name: String::new(),
        jobs,
    }
}

/// SQLite-backed persistence for [`JobBatch`] records.
#[derive(Debug, Clone)]
pub struct JobBatchStore {
    db: sea_orm::DatabaseConnection,
    table_name: String,
}

impl JobBatchStore {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            db,
            table_name: "job_batches".to_string(),
        }
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table_name = table.to_string();
        self
    }

    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    pub async fn ensure_table_exists(&self) -> Result<(), JobError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL DEFAULT '',
                total_jobs INTEGER NOT NULL DEFAULT 0,
                pending_jobs INTEGER NOT NULL DEFAULT 0,
                failed_jobs INTEGER NOT NULL DEFAULT 0,
                cancelled INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                finished_at INTEGER,
                cancelled_at INTEGER
            )",
            self.table_name
        );
        self.db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to create job batches table: {}", e)))?;
        Ok(())
    }

    pub async fn insert(&self, batch: &JobBatch) -> Result<(), JobError> {
        let sql = format!(
            "INSERT INTO {} (id, name, total_jobs, pending_jobs, failed_jobs, cancelled, created_at, finished_at, cancelled_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            self.table_name
        );
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [
                    batch.id.clone().into(),
                    batch.name.clone().into(),
                    batch.total_jobs.into(),
                    batch.pending_jobs.into(),
                    batch.failed_jobs.into(),
                    (batch.cancelled as i64).into(),
                    batch.created_at.into(),
                    batch
                        .finished_at
                        .map(|v| sea_orm::Value::BigInt(Some(v)))
                        .unwrap_or(sea_orm::Value::BigInt(None)),
                    batch
                        .cancelled_at
                        .map(|v| sea_orm::Value::BigInt(Some(v)))
                        .unwrap_or(sea_orm::Value::BigInt(None)),
                ],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to insert batch: {}", e)))?;
        Ok(())
    }

    pub async fn find(&self, id: &str) -> Result<Option<JobBatch>, JobError> {
        let sql = format!(
            "SELECT id, name, total_jobs, pending_jobs, failed_jobs, cancelled, created_at, finished_at, cancelled_at
             FROM {} WHERE id = ?1",
            self.table_name
        );
        let result = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to find batch: {}", e)))?;

        let Some(row) = result else {
            return Ok(None);
        };
        let get = |idx: usize| row.try_get_by_index::<i64>(idx).ok();

        Ok(Some(JobBatch {
            id: row
                .try_get_by_index::<String>(0)
                .unwrap_or_else(|_| id.to_string()),
            name: row.try_get_by_index::<String>(1).unwrap_or_default(),
            total_jobs: get(2).unwrap_or(0),
            pending_jobs: get(3).unwrap_or(0),
            failed_jobs: get(4).unwrap_or(0),
            cancelled: get(5).unwrap_or(0) != 0,
            created_at: get(6).unwrap_or(0),
            finished_at: get(7),
            cancelled_at: get(8),
        }))
    }

    pub async fn decrement_pending(&self, id: &str) -> Result<(), JobError> {
        let sql = format!(
            "UPDATE {} SET pending_jobs = MAX(pending_jobs - 1, 0) WHERE id = ?1",
            self.table_name
        );
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to decrement batch pending: {}", e)))?;
        Ok(())
    }

    pub async fn increment_failed(&self, id: &str) -> Result<(), JobError> {
        let sql = format!(
            "UPDATE {} SET failed_jobs = failed_jobs + 1 WHERE id = ?1",
            self.table_name
        );
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to increment batch failed: {}", e)))?;
        Ok(())
    }

    /// Stamp `finished_at` if every job in the batch has been processed.
    pub async fn mark_finished_if_done(&self, id: &str) -> Result<(), JobError> {
        let now = now_unix();
        let sql = format!(
            "UPDATE {} SET finished_at = ?1 WHERE id = ?2 AND pending_jobs = 0 AND finished_at IS NULL",
            self.table_name
        );
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [now.into(), id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to finish batch: {}", e)))?;
        Ok(())
    }

    pub async fn cancel(&self, id: &str) -> Result<(), JobError> {
        let now = now_unix();
        let sql = format!(
            "UPDATE {} SET cancelled = 1, cancelled_at = ?1 WHERE id = ?2",
            self.table_name
        );
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [now.into(), id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to cancel batch: {}", e)))?;
        Ok(())
    }

    /// Whether the batch has been cancelled (missing rows count as active).
    pub async fn is_cancelled(&self, id: &str) -> Result<bool, JobError> {
        let sql = format!("SELECT cancelled FROM {} WHERE id = ?1", self.table_name);
        let result = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to check batch: {}", e)))?;

        match result {
            Some(row) => Ok(row.try_get_by_index::<i64>(0).unwrap_or(0) != 0),
            None => Ok(false),
        }
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{DatabaseQueue, JobResolver, Queue, QueueWorker, ShouldQueue};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestJob {
        name: String,
        counter: Arc<AtomicUsize>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl ShouldQueue for TestJob {
        async fn handle(&self) -> Result<(), JobError> {
            if self.fail {
                return Err(JobError::Failed("boom".into()));
            }
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn name(&self) -> &str {
            &self.name
        }
        fn max_attempts(&self) -> Option<u64> {
            Some(1)
        }
    }

    async fn setup(name: &str) -> (DatabaseQueue, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let ok_counter = Arc::new(AtomicUsize::new(0));
        let fail_counter = Arc::new(AtomicUsize::new(0));
        let ok = ok_counter.clone();
        let fail = fail_counter.clone();
        let resolver: JobResolver = Arc::new(move |class, _payload| -> Option<super::JobBox> {
            match class {
                "ok" => Some(Box::new(TestJob {
                    name: "ok".into(),
                    counter: ok.clone(),
                    fail: false,
                })),
                "bad" => Some(Box::new(TestJob {
                    name: "bad".into(),
                    counter: fail.clone(),
                    fail: true,
                })),
                _ => None,
            }
        });
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        (
            DatabaseQueue::new(name, db, resolver),
            ok_counter,
            fail_counter,
        )
    }

    #[tokio::test]
    async fn batch_tracks_progress_to_finished() {
        let (queue, counter, _) = setup("default").await;
        queue.ensure_table_exists().await.unwrap();

        let b = batch(vec![Box::new(TestJob {
            name: "ok".into(),
            counter: counter.clone(),
            fail: false,
        })])
        .add_job(Box::new(TestJob {
            name: "ok".into(),
            counter: counter.clone(),
            fail: false,
        }));

        let batch = queue.dispatch_batch(&b.name("import")).await.unwrap();
        assert_eq!(batch.total_jobs(), 2);
        assert_eq!(batch.pending_jobs(), 2);
        assert!(!batch.finished());
        assert_eq!(batch.progress(), 0.0);

        let worker = QueueWorker::new(Arc::new(queue.clone()));
        assert!(worker.process_next_job().await.unwrap().is_ok());
        let found = queue.batch(&batch.id).await.unwrap().unwrap();
        assert_eq!(found.pending_jobs(), 1);
        assert_eq!(found.progress(), 0.5);

        assert!(worker.process_next_job().await.unwrap().is_ok());
        let found = queue.batch(&batch.id).await.unwrap().unwrap();
        assert_eq!(found.pending_jobs(), 0);
        assert!(found.finished());
        assert!(found.finished_at.is_some());
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn batch_records_failed_jobs() {
        let (queue, _, fail_counter) = setup("default").await;
        queue.ensure_table_exists().await.unwrap();
        queue.ensure_failed_table_exists().await.unwrap();

        let batch = queue
            .dispatch_batch(&batch(vec![
                Box::new(TestJob {
                    name: "bad".into(),
                    counter: fail_counter.clone(),
                    fail: true,
                }),
                Box::new(TestJob {
                    name: "ok".into(),
                    counter: fail_counter.clone(),
                    fail: false,
                }),
            ]))
            .await
            .unwrap();

        let worker = QueueWorker::new(Arc::new(queue.clone()));
        let _ = worker.process_next_job().await.unwrap();
        let _ = worker.process_next_job().await.unwrap();

        let found = queue.batch(&batch.id).await.unwrap().unwrap();
        assert_eq!(found.failed_jobs(), 1);
        assert_eq!(found.pending_jobs(), 0);
        assert!(found.finished());
        assert!(found.finished_at.is_some());
    }

    #[tokio::test]
    async fn cancelled_batch_jobs_are_skipped() {
        let (queue, counter, _) = setup("default").await;
        queue.ensure_table_exists().await.unwrap();

        let batch = queue
            .dispatch_batch(&batch(vec![Box::new(TestJob {
                name: "ok".into(),
                counter: counter.clone(),
                fail: false,
            })]))
            .await
            .unwrap();

        queue.cancel_batch(&batch.id).await.unwrap();
        let found = queue.batch(&batch.id).await.unwrap().unwrap();
        assert!(found.cancelled());
        assert!(found.cancelled_at.is_some());

        assert!(queue.pop().await.is_none());
        assert_eq!(queue.count().await, 0);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn batch_refresh_and_cancel_via_instance() {
        let (queue, counter, _) = setup("default").await;
        queue.ensure_table_exists().await.unwrap();

        let batch = queue
            .dispatch_batch(&batch(vec![Box::new(TestJob {
                name: "ok".into(),
                counter: counter.clone(),
                fail: false,
            })]))
            .await
            .unwrap();

        queue.cancel_batch(&batch.id).await.unwrap();
        let refreshed = queue.batch(&batch.id).await.unwrap().unwrap();
        assert!(refreshed.cancelled());
    }
}
