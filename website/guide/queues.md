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

This generates a `SendWelcomeEmailJob` struct (the function name converted to PascalCase plus `Job`) with `new()`, `dispatch()`, and `name()` methods.

The job can be dispatched manually:

```rust
SendWelcomeEmailJob::new(42).dispatch().await?;
```

### Job Attributes

Matching Laravel's `#[Tries]`, `#[Backoff]`, `#[Timeout]`, and `#[FailOnTimeout]`, the `#[job]` macro accepts tuning attributes:

```rust
#[job(tries = 3, backoff = 5, timeout = 30, fail_on_timeout)]
async fn send_welcome_email(user_id: i32) -> Result<(), JobError> {
    // ...
}
```

| Attribute | Default | Description |
|-----------|---------|-------------|
| `tries` | none (worker uses 3) | Max attempts before the job is permanently failed; the worker falls back to `DEFAULT_MAX_ATTEMPTS` (3) when unset |
| `backoff` | none (0) | Seconds to wait before retrying after a failure; 0 when unset |
| `timeout` | none | Job runs longer than this (seconds) are killed and retried (or failed with `fail_on_timeout`); no timeout when unset |
| `fail_on_timeout` | off | Treat a timeout as a permanent failure instead of a retry |

The worker enforces these: timed-out jobs stop executing, failed jobs with attempts remaining are re-released after the backoff delay, and jobs past `tries` are marked permanently failed.

## Dispatching

```rust
use larastvel_core::queue::dispatch;

// Dispatch a job (goes to the default queue)
dispatch(SendWelcomeEmailJob::new(42)).await?;

// Or use QueueManager for explicit queue control
let mut manager = QueueManager::new("default");
manager.register("default", InMemoryQueue::new("default"));
manager.register("sync", SyncQueue::new("sync"));

let queue = manager.default_queue()?;
queue.push(Box::new(SendWelcomeEmailJob::new(42))).await?;
```

### Queue Routing

Like Laravel's `Queue::route()`, jobs can be routed to specific queues centrally, without touching every dispatch site:

```rust
manager.route("send_welcome_email", "emails"); // job name -> queue name
manager.route("send_sms", "sms");

let queue = manager.routed_queue("send_welcome_email")?; // resolves by route, else default
manager.dispatch(SendWelcomeEmailJob::new(42)).await?; // goes to the "emails" queue
```

`routed_queue()` falls back to the default queue when no route matches; `unroute()` removes a rule.

## Queue Worker

```rust
use larastvel_core::queue::QueueWorker;

let worker = QueueWorker::new(Arc::new(queue));

worker.work_once().await?;          // process one job (Err if queue empty)
while let Some(result) = worker.process_next_job().await {
    // process next available job (None when queue is empty)
}

worker.is_running();                // check running state
worker.stop();                      // stop the worker
```

`work_once()` returns `Result<(), JobError>` and errors with `JobError::Queue("No jobs in queue")` when the queue is empty. `process_next_job()` returns `Option<Result<(), JobError>>` — `None` when there is nothing to process.

## Database Queue

```rust
use larastvel_core::queue::{DatabaseQueue, JobBox, JobResolver};

let resolver: JobResolver = Arc::new(|class, payload| {
    match class {
        // payload is a JSON string, e.g. {"name":"send_welcome_email"}
        "send_welcome_email" => Some(Box::new(SendWelcomeEmailJob::new(0)) as JobBox),
        _ => None,
    }
});

let queue = DatabaseQueue::new("default", db, resolver)
    .with_table("jobs");
queue.ensure_table_exists().await?;

// Run the worker via CLI
// larastvel queue:work
```

The `JobResolver` receives the job class name and the raw payload string, and returns the reconstructed job (or `None` if the class is unknown). Note that `DatabaseQueue::push` currently serializes only the job name — job arguments are not persisted, so the resolver must reconstruct the job with the values it needs.

## Failed Jobs

When a job exhausts its attempts (or times out with `fail_on_timeout`), the worker records it in a `failed_jobs` table instead of dropping it silently.

```rust
use larastvel_core::queue::{DatabaseQueue, FailedJobStore};

// Enable failed-job recording on the database queue
let queue = DatabaseQueue::new("default", db, resolver)
    .with_table("jobs")
    .with_failed_table("failed_jobs");
queue.ensure_table_exists().await?;

// Inspect and manage failures programmatically
let store = FailedJobStore::new(db.clone());
store.ensure_table_exists().await?;

let failed = store.all().await?;        // all failed jobs
store.find(1).await?;                   // one failed job by id
store.forget(1).await?;                 // drop one record
store.flush().await?;                   // drop all records
store.count().await;                    // number of failed jobs
```

The CLI manages failed jobs as well:

```bash
larastvel queue:failed        # list failed jobs with their ids
larastvel queue:retry 1 2 all # re-queue failed jobs by id (or "all")
larastvel queue:forget 1      # forget a single failed job
larastvel queue:flush         # forget all failed jobs
```

`queue:retry` re-inserts the job into the `jobs` table with its attempts reset, then removes the `failed_jobs` record.
