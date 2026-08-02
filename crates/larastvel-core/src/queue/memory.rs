use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::{JobBox, JobError, Queue};

#[derive(Debug)]
struct Entry {
    job: JobBox,
    attempts: u64,
    available_at: Instant,
}

#[derive(Debug, Clone)]
pub struct InMemoryQueue {
    name: String,
    jobs: Arc<Mutex<VecDeque<Entry>>>,
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
        jobs.push_back(Entry {
            job,
            attempts: 0,
            available_at: Instant::now(),
        });
        Ok(())
    }

    async fn push_delayed(&self, job: JobBox, delay_seconds: u64) -> Result<(), JobError> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.push_back(Entry {
            job,
            attempts: 0,
            available_at: Instant::now() + Duration::from_secs(delay_seconds),
        });
        Ok(())
    }

    async fn pop(&self) -> Option<(JobBox, u64)> {
        let now = Instant::now();
        let mut jobs = self.jobs.lock().unwrap();
        let entry = jobs.pop_front()?;
        if entry.available_at > now {
            jobs.push_back(entry);
            return None;
        }
        self.processed.fetch_add(1, Ordering::SeqCst);
        Some((entry.job, entry.attempts + 1))
    }

    async fn release(
        &self,
        job: JobBox,
        attempts: u64,
        delay_seconds: u64,
    ) -> Result<(), JobError> {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.push_back(Entry {
            job,
            attempts,
            available_at: Instant::now() + Duration::from_secs(delay_seconds),
        });
        Ok(())
    }

    async fn count(&self) -> usize {
        let jobs = self.jobs.lock().unwrap();
        jobs.len()
    }

    fn name(&self) -> &str {
        &self.name
    }
}
