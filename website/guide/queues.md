# Queues

Larastvel provides queue drivers for deferring time-consuming tasks.

## Drivers

| Driver | Description |
|--------|-------------|
| **Sync** | Executes jobs immediately (synchronous) |
| **In-Memory** | In-process queue (non-persistent) |
| **Database** | Persistent queue backed by SQL |

## Defining Jobs

Use the `#[job]` attribute macro to turn an async function into a queued job:

```rust
use larastvel_core::job;
use larastvel_core::queue::JobError;

#[job]
async fn send_welcome_email(user_id: i32) -> Result<(), JobError> {
    // send email logic
    Ok(())
}
```

This generates a `SendWelcomeEmailJob` struct with `new()`, `dispatch()`, and `name()` methods.

The job can be dispatched manually:

```rust
SendWelcomeEmailJob::new(42).dispatch().await?;
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
