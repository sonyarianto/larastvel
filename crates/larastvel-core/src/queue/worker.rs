use std::sync::{Arc, Mutex};

use super::{JobError, Queue};

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
