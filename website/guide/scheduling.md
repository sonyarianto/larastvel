# Task Scheduling

Larastvel provides cron-based task scheduling.

## Defining Schedules

```rust
use larastvel_core::scheduling::{Schedule, ScheduleManager, CronExpression};

let mut schedule = Schedule::new();
schedule.call("emails:send", || {
    Box::pin(async {
        // send emails
    })
}).every_minute();

// Or use cron expressions
schedule.call("report:generate", || {
    Box::pin(async {
        // generate report
    })
}).cron("0 0 * * *");  // daily at midnight
```

## Schedule Manager

```rust
let mut manager = ScheduleManager::new();
manager.register(schedule);

// Run due events
manager.run_due().await?;
```

## Console Integration

Define scheduled tasks in `routes/console.rs`:

```rust
use larastvel_core::console::{ConsoleKernel, Command};

pub fn schedule(schedule: &mut Schedule) {
    schedule.call("logs:cleanup", || {
        Box::pin(async {
            // cleanup old logs
        })
    }).daily();
}
```

## CLI

```bash
larastvel schedule:run
```
