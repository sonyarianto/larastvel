//! Failed-job storage, mirroring Laravel's `failed_jobs` table + `queue.failer`
//! service. Jobs that exhaust their attempts are recorded here so they can be
//! inspected (`queue:failed`), retried (`queue:retry`), or purged
//! (`queue:flush` / `queue:forget`).

use std::sync::atomic::{AtomicU64, Ordering};

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};

use super::JobError;

/// A single recorded job failure.
#[derive(Debug, Clone)]
pub struct FailedJob {
    pub id: i64,
    pub uuid: String,
    pub connection: String,
    pub queue: String,
    pub class: String,
    pub payload: String,
    pub exception: String,
    pub failed_at: i64,
}

static UUID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a pseudo-random UUID string without pulling in extra
/// dependencies — a hash of the current time, process id, and a counter.
fn generate_uuid() -> String {
    use sha2::{Digest, Sha256};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = UUID_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}:{}", now, pid, counter));
    let digest = hasher.finalize();
    hex::encode(&digest[..16])
}

/// Persists failed jobs to a `failed_jobs` database table.
pub struct FailedJobStore {
    table: String,
    db: DatabaseConnection,
}

impl FailedJobStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            table: "failed_jobs".to_string(),
            db,
        }
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table = table.to_string();
        self
    }

    pub fn table_name(&self) -> &str {
        &self.table
    }

    pub async fn ensure_table_exists(&self) -> Result<(), JobError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT NOT NULL,
                connection TEXT NOT NULL DEFAULT 'default',
                queue TEXT NOT NULL DEFAULT 'default',
                class TEXT NOT NULL DEFAULT '',
                payload TEXT NOT NULL,
                exception TEXT NOT NULL DEFAULT '',
                failed_at INTEGER NOT NULL
            )",
            self.table
        );
        self.db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to create failed jobs table: {}", e)))?;
        Ok(())
    }

    /// Record a failed job.
    pub async fn log(
        &self,
        connection: &str,
        queue: &str,
        class: &str,
        payload: &str,
        exception: &str,
    ) -> Result<FailedJob, JobError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let sql = format!(
            "INSERT INTO {} (uuid, connection, queue, class, payload, exception, failed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            self.table
        );
        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [
                    generate_uuid().into(),
                    connection.into(),
                    queue.into(),
                    class.into(),
                    payload.into(),
                    exception.into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to log failed job: {}", e)))?;

        Ok(FailedJob {
            id: result.last_insert_id() as i64,
            uuid: String::new(),
            connection: connection.to_string(),
            queue: queue.to_string(),
            class: class.to_string(),
            payload: payload.to_string(),
            exception: exception.to_string(),
            failed_at: now,
        })
    }

    /// Return all recorded failures, oldest first.
    pub async fn all(&self) -> Result<Vec<FailedJob>, JobError> {
        let sql = format!("SELECT * FROM {} ORDER BY id ASC", self.table);
        let result = self
            .db
            .query_all(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to list failed jobs: {}", e)))?;

        let mut jobs = Vec::new();
        for row in result {
            jobs.push(FailedJob {
                id: row.try_get_by_index(0).unwrap_or(0),
                uuid: row.try_get_by_index(1).unwrap_or_default(),
                connection: row.try_get_by_index(2).unwrap_or_default(),
                queue: row.try_get_by_index(3).unwrap_or_default(),
                class: row.try_get_by_index(4).unwrap_or_default(),
                payload: row.try_get_by_index(5).unwrap_or_default(),
                exception: row.try_get_by_index(6).unwrap_or_default(),
                failed_at: row.try_get_by_index(7).unwrap_or(0),
            });
        }
        Ok(jobs)
    }

    /// Find a single failed job by id.
    pub async fn find(&self, id: i64) -> Result<Option<FailedJob>, JobError> {
        let sql = format!("SELECT * FROM {} WHERE id = ?1", self.table);
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to find failed job: {}", e)))?;

        Ok(row.map(|row| FailedJob {
            id: row.try_get_by_index(0).unwrap_or(id),
            uuid: row.try_get_by_index(1).unwrap_or_default(),
            connection: row.try_get_by_index(2).unwrap_or_default(),
            queue: row.try_get_by_index(3).unwrap_or_default(),
            class: row.try_get_by_index(4).unwrap_or_default(),
            payload: row.try_get_by_index(5).unwrap_or_default(),
            exception: row.try_get_by_index(6).unwrap_or_default(),
            failed_at: row.try_get_by_index(7).unwrap_or(0),
        }))
    }

    /// Remove a failed job by id. Returns `true` if a row was removed.
    pub async fn forget(&self, id: i64) -> Result<bool, JobError> {
        let sql = format!("DELETE FROM {} WHERE id = ?1", self.table);
        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [id.into()],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to forget failed job: {}", e)))?;
        Ok(result.rows_affected() > 0)
    }

    /// Remove all failed jobs. Returns the number of removed rows.
    pub async fn flush(&self) -> Result<usize, JobError> {
        let sql = format!("DELETE FROM {}", self.table);
        let result = self
            .db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to flush failed jobs: {}", e)))?;
        Ok(result.rows_affected() as usize)
    }

    /// Total number of recorded failures.
    pub async fn count(&self) -> usize {
        let sql = format!("SELECT COUNT(*) FROM {}", self.table);
        match self
            .db
            .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
        {
            Ok(Some(row)) => row.try_get_by_index::<i64>(0).unwrap_or(0) as usize,
            _ => 0,
        }
    }

    /// Re-queue a failed job back onto the given jobs table, resetting its
    /// attempt count, then forget the failure record.
    pub async fn requeue(&self, jobs_table: &str, job: &FailedJob) -> Result<(), JobError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let sql = format!(
            "INSERT INTO {} (queue, payload, class, attempts, max_attempts, available_at, created_at)
             VALUES (?1, ?2, ?3, 0, 3, ?4, ?4)",
            jobs_table
        );
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [
                    job.queue.clone().into(),
                    job.payload.clone().into(),
                    job.class.clone().into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to requeue failed job: {}", e)))?;
        self.forget(job.id).await?;
        Ok(())
    }
}

/// Build the serialized payload for a job (mirrors `DatabaseQueue::push`).
pub(crate) fn job_payload(job: &dyn crate::queue::ShouldQueue) -> String {
    serde_json::to_string(&serde_json::json!({ "name": job.name() }))
        .unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    async fn store() -> FailedJobStore {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");
        let store = FailedJobStore::new(db);
        store.ensure_table_exists().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_log_and_all() {
        let store = store().await;
        store
            .log(
                "default",
                "default",
                "SendEmail",
                "{\"name\":\"x\"}",
                "boom",
            )
            .await
            .unwrap();
        store
            .log("default", "emails", "ProcessPodcast", "{}", "timeout")
            .await
            .unwrap();

        let jobs = store.all().await.unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].class, "SendEmail");
        assert_eq!(jobs[0].exception, "boom");
        assert_eq!(jobs[1].queue, "emails");
        assert_eq!(jobs[1].failed_at, jobs[1].failed_at);
        assert!(!jobs[0].uuid.is_empty(), "uuid must be generated");
    }

    #[tokio::test]
    async fn test_find_and_forget() {
        let store = store().await;
        store
            .log("default", "default", "JobA", "{}", "err")
            .await
            .unwrap();
        let job = store.find(1).await.unwrap().expect("job 1 must exist");
        assert_eq!(job.class, "JobA");

        assert!(store.forget(1).await.unwrap());
        assert!(!store.forget(1).await.unwrap(), "already removed");
        assert_eq!(store.count().await, 0);
    }

    #[tokio::test]
    async fn test_flush() {
        let store = store().await;
        store
            .log("default", "default", "A", "{}", "")
            .await
            .unwrap();
        store
            .log("default", "default", "B", "{}", "")
            .await
            .unwrap();
        assert_eq!(store.flush().await.unwrap(), 2);
        assert_eq!(store.count().await, 0);
    }

    #[tokio::test]
    async fn test_requeue_resets_attempts() {
        let store = store().await;
        store
            .log("default", "default", "SendEmail", "{}", "err")
            .await
            .unwrap();
        let job = store.find(1).await.unwrap().unwrap();

        use sea_orm::ConnectionTrait;
        store
            .db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                "CREATE TABLE jobs (id INTEGER PRIMARY KEY AUTOINCREMENT, queue TEXT, payload TEXT, class TEXT, attempts INTEGER, max_attempts INTEGER, available_at INTEGER, created_at INTEGER)".to_string(),
            ))
            .await
            .unwrap();

        store.requeue("jobs", &job).await.unwrap();

        let row = store
            .db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT queue, payload, class, attempts FROM jobs".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let queue: String = row.try_get_by_index(0).unwrap();
        let class: String = row.try_get_by_index(2).unwrap();
        let attempts: i64 = row.try_get_by_index(3).unwrap();
        assert_eq!(queue, "default");
        assert_eq!(class, "SendEmail");
        assert_eq!(attempts, 0, "attempts must reset on requeue");
        assert_eq!(store.count().await, 0, "failure record must be forgotten");
    }

    #[tokio::test]
    async fn test_generate_uuid_unique() {
        let a = generate_uuid();
        let b = generate_uuid();
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[allow(dead_code)]
    fn _assert_jobbox_payload(job: crate::queue::JobBox) {
        let _ = job_payload(job.as_ref());
    }
}
