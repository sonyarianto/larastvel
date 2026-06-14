use std::fmt;

#[derive(Debug, Clone)]
pub struct SchedulingError(pub String);

impl fmt::Display for SchedulingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SchedulingError {}

impl From<String> for SchedulingError {
    fn from(s: String) -> Self {
        SchedulingError(s)
    }
}
