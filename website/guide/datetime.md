# Date & Time (Dt)

Larastvel's `Dt` provides a fluent wrapper around `chrono::DateTime<Utc>` inspired by Laravel's Carbon API for date and time manipulation.

## Creating Instances

```rust
use larastvel_core::{Dt, now, today};

// Current time
let now = Dt::now();
let now = now();  // free function

// Start of today
let today = Dt::today();
let today = today();  // free function

// Relative days
let tomorrow = Dt::tomorrow();
let yesterday = Dt::yesterday();

// From specific dates
let dt = Dt::from_ymd(2025, 6, 15);
let dt = Dt::from_ymd_hms(2025, 6, 15, 10, 30, 0);
```

### Parsing

```rust
// ISO 8601
let dt = Dt::parse("2025-06-15T10:30:00Z").unwrap();

// Space-separated
let dt = Dt::parse("2025-06-15 10:30:00").unwrap();

// Custom format
let dt = Dt::from_format("15/06/2025 10:30", "%d/%m/%Y %H:%M").unwrap();
```

## Formatting

```rust
let dt = Dt::from_ymd_hms(2025, 6, 15, 10, 30, 0);

dt.to_date_string();        // "2025-06-15"
dt.to_time_string();        // "10:30:00"
dt.to_datetime_string();    // "2025-06-15 10:30:00"
dt.to_rfc3339();            // "2025-06-15T10:30:00+00:00"
dt.to_rfc2822();            // "Sun, 15 Jun 2025 10:30:00 +0000"
dt.to_iso_string();         // RFC 3339 format
dt.format("%Y-%m-%d");      // custom format string
```

## Available Methods

| Method | Description |
|--------|-------------|
| `now()` | Current UTC time |
| `today()` | Start of current day |
| `tomorrow()` | Start of next day |
| `yesterday()` | Start of previous day |
| `parse(s)` | Parse ISO 8601 or space-separated datetime |
| `from_format(s, fmt)` | Parse with custom format string |
| `year()`, `month()`, `day()` | Date component accessors |
| `hour()`, `minute()`, `second()` | Time component accessors |
| `timestamp()` | Unix timestamp in seconds |
| `format(fmt)` | Custom format output |
| `to_date_string()` | Format as `YYYY-MM-DD` |
| `to_time_string()` | Format as `HH:MM:SS` |
| `to_datetime_string()` | Format as `YYYY-MM-DD HH:MM:SS` |
| `to_rfc3339()` / `to_rfc2822()` | RFC format output |
| `add_days(n)` / `sub_days(n)` | Add/subtract days |
| `add_hours(n)` / `sub_hours(n)` | Add/subtract hours |
| `add_minutes(n)` / `sub_minutes(n)` | Add/subtract minutes |
| `add_seconds(n)` / `sub_seconds(n)` | Add/subtract seconds |
| `add_weeks(n)` / `sub_weeks(n)` | Add/subtract weeks |
| `start_of_day()` / `end_of_day()` | Start/end of current day |
| `start_of_month()` / `end_of_month()` | Start/end of current month |
| `start_of_year()` / `end_of_year()` | Start/end of current year |
| `start_of_week()` / `end_of_week()` | Start (Mon) / end (Sun) of week |
| `diff_in_days(other)` / `diff_in_hours(other)` | Difference in various units |
| `diff_in_minutes(other)` / `diff_in_seconds(other)` | |
| `diff_in_weeks(other)` | |
| `gt(other)` / `gte(other)` / `lt(other)` / `lte(other)` | Comparison methods |
| `is_future()` / `is_past()` | Relative time checks |
| `is_today()` / `is_weekend()` / `is_weekday()` | Calendar checks |
| `average(a, b)` | Midpoint between two instants |
| `copy()` | Clone the instance |
| `inner()` / `into_inner()` | Borrow/consume inner `DateTime<Utc>` |

## Arithmetic

`Dt` supports `Add<Duration>` and `Sub<Duration>`:

```rust
use chrono::Duration;

let dt = Dt::from_ymd(2025, 1, 1) + Duration::days(10);
// day == 11

let dt = Dt::from_ymd(2025, 1, 15) - Duration::days(5);
// day == 10
```

## Comparison

`Dt` implements `Ord`, `PartialOrd`, `Eq`, `PartialEq`:

```rust
let a = Dt::from_ymd(2025, 1, 1);
let b = Dt::from_ymd(2025, 1, 2);

assert!(a < b);
assert!(b > a);
assert!(a <= b);
assert!(b >= a);
```

## Conversions

```rust
use chrono::{DateTime, NaiveDateTime, Utc};

// From chrono types
let dt = Dt::from(Utc::now());
let dt = Dt::from(naive_datetime);

// Into chrono types
let dt: DateTime<Utc> = Dt::now().into();

// Borrow or consume the inner value
let inner: &DateTime<Utc> = dt.inner();
let consumed: DateTime<Utc> = dt.into_inner();

// Display
println!("{}", Dt::now());  // "2025-06-15 10:30:00"
```

## Free Functions

```rust
use larastvel_core::{now, today};

let dt = now();   // same as Dt::now()
let dt = today(); // same as Dt::today()
```
