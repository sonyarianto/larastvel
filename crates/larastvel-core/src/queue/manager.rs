use std::collections::HashMap;
use std::sync::Arc;

use super::{JobBox, JobError, Queue, ShouldQueue};

#[derive(Debug, Clone)]
pub struct QueueManager {
    queues: HashMap<String, Arc<dyn Queue>>,
    routes: HashMap<String, String>,
    default: String,
}

impl QueueManager {
    pub fn new(default: &str) -> Self {
        Self {
            queues: HashMap::new(),
            routes: HashMap::new(),
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

    /// Register a routing rule mapping a job name to a queue connection,
    /// mirroring Laravel's `Queue::route(Job::class, connection: ...)`.
    pub fn route(&mut self, job: &str, queue: &str) {
        self.routes.insert(job.to_string(), queue.to_string());
    }

    /// Remove a routing rule for the given job name.
    pub fn unroute(&mut self, job: &str) {
        self.routes.remove(job);
    }

    /// Resolve the queue a job should be dispatched to, falling back to the
    /// default queue when no route matches.
    pub fn routed_queue(&self, job: &str) -> Result<Arc<dyn Queue>, JobError> {
        match self.routes.get(job) {
            Some(queue_name) => self.queue(queue_name),
            None => self.default_queue(),
        }
    }

    /// Dispatch a job to its routed queue (or the default queue), honoring
    /// the job's `delay_seconds()` (Laravel's `#[Delay]` attribute) when set.
    pub async fn dispatch<J: ShouldQueue + 'static>(&self, job: J) -> Result<(), JobError> {
        let name = job.name().to_string();
        let delay = job.delay_seconds();
        let queue = self.routed_queue(&name)?;
        let boxed: JobBox = Box::new(job);
        match delay {
            Some(seconds) if seconds > 0 => queue.push_delayed(boxed, seconds).await,
            _ => queue.push(boxed).await,
        }
    }
}
