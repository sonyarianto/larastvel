use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::{
    batches::{JobBatch, PendingBatch},
    JobBox, JobError, Queue,
};

pub type JobResolver = Arc<dyn Fn(&str, &str) -> Option<JobBox> + Send + Sync>;

type Reserved = Option<(i64, Option<String>)>;

pub struct DatabaseQueue {
    name: String,
    table_name: String,
    db: sea_orm::DatabaseConnection,
    resolver: JobResolver,
    last_reserved: Arc<Mutex<Reserved>>,
    failed_table_name: String,
    batch_table_name: String,
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
            last_reserved: self.last_reserved.clone(),
            failed_table_name: self.failed_table_name.clone(),
            batch_table_name: self.batch_table_name.clone(),
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
            last_reserved: Arc::new(Mutex::new(None)),
            failed_table_name: "failed_jobs".to_string(),
            batch_table_name: "job_batches".to_string(),
        }
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table_name = table.to_string();
        self
    }

    /// Set the name of the `failed_jobs` table used to record permanent
    /// failures.
    pub fn with_failed_table(mut self, table: &str) -> Self {
        self.failed_table_name = table.to_string();
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

    /// Ensure the `failed_jobs` table exists (used by [`Queue::fail`]).
    pub async fn ensure_failed_table_exists(&self) -> Result<(), JobError> {
        super::FailedJobStore::new(self.db.clone())
            .with_table(&self.failed_table_name)
            .ensure_table_exists()
            .await
    }

    /// Set the name of the `job_batches` table used to track batches.
    pub fn with_batches_table(mut self, table: &str) -> Self {
        self.batch_table_name = table.to_string();
        self
    }

    /// Ensure the `job_batches` table exists.
    pub async fn ensure_batches_table_exists(&self) -> Result<(), JobError> {
        super::JobBatchStore::new(self.db.clone())
            .with_table(&self.batch_table_name)
            .ensure_table_exists()
            .await
    }

    /// Dispatch a pending batch (Laravel's `Bus::batch([...])->dispatch()`).
    pub async fn dispatch_batch(&self, pending: &PendingBatch) -> Result<JobBatch, JobError> {
        self.ensure_batches_table_exists().await?;

        let now = now_unix();
        let id = uuid::Uuid::new_v4().to_string();
        let total = pending.jobs().len() as i64;
        let batch = JobBatch {
            id,
            name: pending.name_of().to_string(),
            total_jobs: total,
            pending_jobs: total,
            failed_jobs: 0,
            cancelled: false,
            created_at: now,
            finished_at: None,
            cancelled_at: None,
        };

        super::JobBatchStore::new(self.db.clone())
            .with_table(&self.batch_table_name)
            .insert(&batch)
            .await?;

        for job in pending.jobs() {
            let payload = serde_json::to_string(&serde_json::json!({
                "name": job.name(),
                "batch_id": batch.id,
            }))
            .map_err(|e| JobError::Queue(format!("Serialization error: {}", e)))?;

            let max_attempts = job.max_attempts().unwrap_or(3);
            let sql = format!(
                "INSERT INTO {} (queue, payload, class, attempts, max_attempts, available_at, created_at)
                 VALUES (?1, ?2, ?3, 0, ?4, ?5, ?5)",
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
                        job.name().to_string().into(),
                        (max_attempts as i64).into(),
                        now.into(),
                    ],
                ))
                .await
                .map_err(|e| JobError::Queue(format!("Failed to push batch job: {}", e)))?;
        }

        Ok(batch)
    }

    /// Look up a batch by its id.
    pub async fn batch(&self, id: &str) -> Result<Option<JobBatch>, JobError> {
        super::JobBatchStore::new(self.db.clone())
            .with_table(&self.batch_table_name)
            .find(id)
            .await
    }

    /// Cancel a batch by its id. Reserved jobs finish; the rest are skipped
    /// by the worker.
    pub async fn cancel_batch(&self, id: &str) -> Result<(), JobError> {
        super::JobBatchStore::new(self.db.clone())
            .with_table(&self.batch_table_name)
            .cancel(id)
            .await
    }

    /// Record the last reserved job as failed in the `failed_jobs` table and
    /// remove it from the jobs table. If no job was reserved, falls back to
    /// exhausting its attempts.
    async fn record_failure(&self, job: JobBox, exception: String) -> Result<(), JobError> {
        let reserved = self.last_reserved.lock().unwrap().clone();
        let Some((job_id, batch_id)) = reserved else {
            return Ok(());
        };

        if let Some(batch_id) = &batch_id {
            let store =
                super::JobBatchStore::new(self.db.clone()).with_table(&self.batch_table_name);
            store.increment_failed(batch_id).await?;
            store.decrement_pending(batch_id).await?;
            store.mark_finished_if_done(batch_id).await?;
        }

        let store = super::FailedJobStore::new(self.db.clone()).with_table(&self.failed_table_name);
        store.ensure_table_exists().await?;

        let class = job.name().to_string();
        let payload = super::failed::job_payload(job.as_ref());

        store
            .log(&self.name, &self.name, &class, &payload, &exception)
            .await?;

        let sql = format!("DELETE FROM {} WHERE id = ?1", self.table_name);
        use sea_orm::ConnectionTrait;
        self.db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                &sql,
                [job_id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to remove failed job row: {}", e)))?;
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
        let max_attempts = job.max_attempts().unwrap_or(3);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sql = format!(
            "INSERT INTO {} (queue, payload, class, attempts, max_attempts, available_at, created_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?5, ?5)",
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
                    (max_attempts as i64).into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to push job: {}", e)))?;
        Ok(())
    }

    async fn pop(&self) -> Option<(JobBox, u64)> {
        let now = now_unix();
        let store = super::JobBatchStore::new(self.db.clone()).with_table(&self.batch_table_name);

        loop {
            let sql = format!(
                "SELECT id, payload, class, attempts FROM {}
                 WHERE queue = ?1 AND (reserved_at IS NULL OR reserved_at < ?2)
                 AND attempts < max_attempts
                 AND available_at <= ?2
                 ORDER BY id ASC LIMIT 1",
                self.table_name
            );
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
            let id: Option<i64> = row.try_get_by_index::<i64>(0).ok();
            let batch_id: Option<String> =
                serde_json::from_str(&payload)
                    .ok()
                    .and_then(|v: serde_json::Value| {
                        v.get("batch_id").and_then(|b| b.as_str()).map(String::from)
                    });

            if let Some(batch_id) = &batch_id {
                if store.is_cancelled(batch_id).await.unwrap_or(false) {
                    if let Some(job_id) = id {
                        let delete_sql = format!("DELETE FROM {} WHERE id = ?1", self.table_name);
                        let _ = self
                            .db
                            .execute(sea_orm::Statement::from_sql_and_values(
                                sea_orm::DatabaseBackend::Sqlite,
                                &delete_sql,
                                [job_id.into()],
                            ))
                            .await;
                    }
                    continue;
                }
            }

            let resolver = self.resolver.clone();
            let job = resolver(&class, &payload)?;

            let attempts: i64 = row.try_get_by_index::<i64>(3).unwrap_or(0);
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

                let mut last = self.last_reserved.lock().unwrap();
                *last = Some((job_id, batch_id));
            }

            return Some((job, (attempts + 1) as u64));
        }
    }

    /// Release the last reserved row back to the queue with a delay, keeping
    /// its attempt count. Assumes a single worker per queue instance.
    async fn release(
        &self,
        job: JobBox,
        attempts: u64,
        delay_seconds: u64,
    ) -> Result<(), JobError> {
        let _ = (job, attempts);
        let id = {
            let last = self.last_reserved.lock().unwrap();
            last.as_ref().map(|(job_id, _)| *job_id)
        };
        let Some(job_id) = id else {
            return Ok(());
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sql = format!(
            "UPDATE {} SET reserved_at = NULL, available_at = ?1 WHERE id = ?2",
            self.table_name
        );
        use sea_orm::ConnectionTrait;
        self.db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                &sql,
                [(now + delay_seconds as i64).into(), job_id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to release job: {}", e)))?;
        Ok(())
    }

    /// Permanently fail the last reserved row by recording it in the
    /// `failed_jobs` table and removing it from the jobs table.
    async fn fail(&self, job: JobBox, exception: String) -> Result<(), JobError> {
        self.record_failure(job, exception).await
    }

    /// Called by the worker after a job finishes successfully: decrements the
    /// pending count of the job's batch (if any) and marks it finished.
    async fn job_succeeded(&self, job: JobBox) -> Result<(), JobError> {
        let reserved = self.last_reserved.lock().unwrap().clone();
        let _ = job;

        let Some((_, Some(batch_id))) = reserved else {
            return Ok(());
        };

        let store = super::JobBatchStore::new(self.db.clone()).with_table(&self.batch_table_name);
        store.decrement_pending(&batch_id).await?;
        store.mark_finished_if_done(&batch_id).await?;
        Ok(())
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

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
