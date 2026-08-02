# Task Scheduling

Larastvel provides cron-based task scheduling.

## Defining Schedules

```rust
use larastvel_core::scheduling::{Schedule, ScheduleManager, SchedulingError};

let schedule = Schedule::new();

// Every minute — the callback returns Result<(), JobError>
schedule.call("* * * * *", "emails:send", || async {
    // send emails
    Ok(())
})?;

// Or use the cron builder with a description
schedule
    .cron("0 0 * * *") // daily at midnight
    .description("report:generate")
    .call(|| async {
        // generate report
        Ok(())
    })?;
```

## Schedule Manager

```rust
let manager = ScheduleManager::new(schedule);

// Run due events (synchronous — returns per-event results)
let results = manager.run_due();

// Or inspect what's due
let due = manager.due_events();
```

`run_due()` is synchronous and returns a `Vec<Result<(), JobError>>` — one entry per due event. An async variant `run_due_async()` is also available.

## Console Integration

Handle the `--schedule:run` argument in `main.rs` to run scheduled tasks:

```rust
let schedule = Schedule::new();
schedule.call("* * * * *", "logs:cleanup", || async {
    // cleanup old logs
    Ok(())
})?;

let manager = ScheduleManager::new(schedule);

// Check for the --schedule:run argument
if args.contains("--schedule:run") {
    manager.run_due_async().await;
}
```

`larastvel schedule:run` runs the application with the `--schedule:run` argument; a `--schedule:list` argument can be handled similarly to print `schedule.events()`.

## CLI

```bash
larastvel schedule:run
```
