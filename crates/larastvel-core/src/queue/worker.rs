use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{JobBox, JobError, Queue};

/// Default maximum attempts for jobs that do not declare `#[tries]`.
pub const DEFAULT_MAX_ATTEMPTS: u64 = 3;

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
        if let Some((job, attempts)) = self.queue.pop().await {
            Some(self.execute(job, attempts).await)
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
            if let Some(result) = self.process_next_job().await {
                if let Err(e) = result {
                    tracing::error!("[QueueWorker] Job failed permanently: {}", e);
                }
            } else {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    pub async fn work_once(&self) -> Result<(), JobError> {
        if let Some((job, attempts)) = self.queue.pop().await {
            self.execute(job, attempts).await
        } else {
            Err(JobError::Queue("No jobs in queue".to_string()))
        }
    }

    /// Run a single job attempt, enforcing the job's declared `#[timeout]`,
    /// `#[tries]`, `#[backoff]`, and `#[fail_on_timeout]` attributes. Failed
    /// attempts are released back to the queue; permanent failures are marked
    /// via `Queue::fail`.
    async fn execute(&self, job: JobBox, attempts: u64) -> Result<(), JobError> {
        let name = job.name().to_string();
        let max = job.max_attempts().unwrap_or(DEFAULT_MAX_ATTEMPTS);
        let backoff = job.backoff_seconds().unwrap_or(0);
        let fail_on_timeout = job.fail_on_timeout();

        let outcome = match job.timeout_seconds() {
            Some(secs) => tokio::time::timeout(Duration::from_secs(secs), job.handle()).await,
            None => Ok(job.handle().await),
        };

        match outcome {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                tracing::warn!(
                    "[QueueWorker] Job {} failed on attempt {}/{}: {}",
                    name,
                    attempts,
                    max,
                    e
                );
                if attempts >= max {
                    self.queue.fail(job, e.to_string()).await?;
                    tracing::error!(
                        "[QueueWorker] Job {} failed permanently after {} attempts",
                        name,
                        attempts
                    );
                    Err(e)
                } else {
                    self.queue.release(job, attempts, backoff).await?;
                    tracing::warn!(
                        "[QueueWorker] Job {} released for retry (attempt {}/{})",
                        name,
                        attempts,
                        max
                    );
                    Ok(())
                }
            }
            Err(_) => {
                tracing::warn!(
                    "[QueueWorker] Job {} timed out on attempt {}/{}",
                    name,
                    attempts,
                    max
                );
                if fail_on_timeout || attempts >= max {
                    self.queue
                        .fail(job, format!("Job {} timed out", name))
                        .await?;
                    tracing::error!(
                        "[QueueWorker] Job {} timed out and failed permanently",
                        name
                    );
                    Err(JobError::Failed(format!("Job {} timed out", name)))
                } else {
                    self.queue.release(job, attempts, backoff).await?;
                    tracing::warn!(
                        "[QueueWorker] Job {} released after timeout (attempt {}/{})",
                        name,
                        attempts,
                        max
                    );
                    Ok(())
                }
            }
        }
    }
}
