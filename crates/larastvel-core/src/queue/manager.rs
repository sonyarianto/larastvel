use std::collections::HashMap;
use std::sync::Arc;

use super::{JobError, Queue};

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
