use async_trait::async_trait;

use super::{JobBox, JobError, Queue};

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

    async fn pop(&self) -> Option<(JobBox, u64)> {
        None
    }

    /// Sync jobs run exactly once per push — a failed attempt is not retried.
    async fn release(
        &self,
        job: JobBox,
        attempts: u64,
        delay_seconds: u64,
    ) -> Result<(), JobError> {
        let _ = (job, attempts, delay_seconds);
        Ok(())
    }

    async fn count(&self) -> usize {
        0
    }

    fn name(&self) -> &str {
        &self.name
    }
}
