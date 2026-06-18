# Queues

Larastvel provides queue drivers for deferring time-consuming tasks.

## Drivers

| Driver | Description |
|--------|-------------|
| **Sync** | Executes jobs immediately (synchronous) |
| **In-Memory** | In-process queue (non-persistent) |
| **Database** | Persistent queue backed by SQL |

## Defining Jobs

```rust
use larastvel_core::queue::{ShouldQueue, JobError, dispatch};

#[derive(Debug)]
struct SendWelcomeEmail {
    user_id: i32,
}

#[async_trait]
impl ShouldQueue for SendWelcomeEmail {
    async fn handle(&self) -> Result<(), JobError> {
        // send email logic
        Ok(())
    }

    fn name(&self) -> &str {
        "send-welcome-email"
    }
}
```

## Dispatching

```rust
// Dispatch using the default sync queue
dispatch(SendWelcomeEmail { user_id: 42 }).await?;

// Or use QueueManager for explicit queue control
let mut manager = QueueManager::new("default");
manager.register("default", InMemoryQueue::new("default"));
manager.register("sync", SyncQueue::new("sync"));

let queue = manager.default_queue()?;
queue.push(Box::new(SendWelcomeEmail { user_id: 42 })).await?;
```

## Queue Worker

```rust
use larastvel_core::queue::QueueWorker;

let worker = QueueWorker::new(Arc::new(queue));
worker.work_once().await?;    // process one job
worker.process_next_job().await; // process next available
```

## Database Queue

```rust
use larastvel_core::queue::DatabaseQueue;

let resolver: JobResolver = Arc::new(|class, payload| {
    match class {
        "send-welcome-email" => Some(Box::new(SendWelcomeEmail::from_payload(payload))),
        _ => None,
    }
});

let queue = DatabaseQueue::new("default", db, resolver)
    .with_table("jobs");
queue.ensure_table_exists().await?;

// Run the worker via CLI
// larastvel queue:work
```
