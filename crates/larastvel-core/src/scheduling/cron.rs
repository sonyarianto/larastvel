use chrono::{Datelike, Timelike};

use super::SchedulingError;

pub fn parse_cron(expr: &str) -> Result<CronExpression, SchedulingError> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(format!(
            "Invalid cron expression '{}': expected 5 fields, got {}",
            expr,
            parts.len()
        )
        .into());
    }

    Ok(CronExpression {
        minute: CronField::parse(parts[0], 0, 59)?,
        hour: CronField::parse(parts[1], 0, 23)?,
        day_of_month: CronField::parse(parts[2], 1, 31)?,
        month: CronField::parse(parts[3], 1, 12)?,
        day_of_week: CronField::parse(parts[4], 0, 6)?,
    })
}

#[derive(Debug, Clone)]
pub struct CronExpression {
    minute: CronField,
    hour: CronField,
    day_of_month: CronField,
    month: CronField,
    day_of_week: CronField,
}

impl CronExpression {
    pub fn is_due(&self, dt: &chrono::DateTime<chrono::Local>) -> bool {
        self.minute.matches(dt.minute() as i32)
            && self.hour.matches(dt.hour() as i32)
            && self.day_of_month.matches(dt.day() as i32)
            && self.month.matches(dt.month() as i32)
            && self
                .day_of_week
                .matches(dt.weekday().num_days_from_sunday() as i32)
    }
}

#[derive(Debug, Clone)]
enum CronField {
    All,
    Single(i32),
    List(Vec<i32>),
    Range(i32, i32),
    Step(i32, i32),
    StepRange(i32, i32, i32),
}

impl CronField {
    fn parse(field: &str, min: i32, max: i32) -> Result<Self, SchedulingError> {
        match field {
            "*" => Ok(CronField::All),
            _ if field.contains('/') => {
                let parts: Vec<&str> = field.split('/').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid step expression: {}", field).into());
                }
                let step: i32 = parts[1]
                    .parse()
                    .map_err(|_| format!("Invalid step: {}", parts[1]))?;
                if parts[0] == "*" {
                    Ok(CronField::Step(min, step))
                } else if parts[0].contains('-') {
                    let range_parts: Vec<&str> = parts[0].split('-').collect();
                    if range_parts.len() != 2 {
                        return Err(format!("Invalid step range: {}", field).into());
                    }
                    let start: i32 = range_parts[0]
                        .parse()
                        .map_err(|_| "Invalid range start".to_string())?;
                    let end: i32 = range_parts[1]
                        .parse()
                        .map_err(|_| "Invalid range end".to_string())?;
                    Ok(CronField::StepRange(start, end, step))
                } else {
                    let start: i32 = parts[0]
                        .parse()
                        .map_err(|_| "Invalid step start".to_string())?;
                    Ok(CronField::StepRange(start, max, step))
                }
            }
            _ if field.contains(',') => {
                let values: Result<Vec<i32>, _> =
                    field.split(',').map(|s| s.trim().parse::<i32>()).collect();
                Ok(CronField::List(
                    values.map_err(|_| format!("Invalid list: {}", field))?,
                ))
            }
            _ if field.contains('-') => {
                let parts: Vec<&str> = field.split('-').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid range: {}", field).into());
                }
                let start: i32 = parts[0]
                    .parse()
                    .map_err(|_| "Invalid range start".to_string())?;
                let end: i32 = parts[1]
                    .parse()
                    .map_err(|_| "Invalid range end".to_string())?;
                Ok(CronField::Range(start, end))
            }
            _ => {
                let val: i32 = field
                    .parse()
                    .map_err(|_| format!("Invalid cron value: {}", field))?;
                Ok(CronField::Single(val))
            }
        }
    }

    fn matches(&self, value: i32) -> bool {
        match self {
            CronField::All => true,
            CronField::Single(v) => *v == value,
            CronField::List(values) => values.contains(&value),
            CronField::Range(start, end) => value >= *start && value <= *end,
            CronField::Step(start, step) => value >= *start && (value - start) % step == 0,
            CronField::StepRange(start, end, step) => {
                value >= *start && value <= *end && (value - start) % step == 0
            }
        }
    }
}
