use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::{JobBox, JobError, Queue};

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
