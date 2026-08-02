use regex::Regex;
use std::sync::Arc;

/// An error returned by a custom validation rule.
#[derive(Debug, Clone)]
pub struct ValidationError(pub String);

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ValidationError {}

/// Trait for custom validation rules.
pub trait ValidationRule: Send + Sync + std::fmt::Debug {
    /// The rule name (used in error messages).
    fn name(&self) -> &str;
    /// Validate a value. Return `Ok(())` if valid, `Err(ValidationError)` if not.
    fn validate(&self, field: &str, value: &str) -> Result<(), ValidationError>;
}

#[derive(Clone)]
pub enum Rule {
    Required,
    Email,
    Min(usize),
    Max(usize),
    MinValue(f64),
    MaxValue(f64),
    String,
    Numeric,
    Boolean,
    Confirmed,
    Same(String),
    Different(String),
    Alpha,
    AlphaNumeric,
    Url,
    Ip,
    Base64,
    Regex(Regex),
    Between(usize, usize),
    Size(usize),
    Present,
    Prohibited,
    Custom(Arc<dyn ValidationRule>),
    /// The field value must not already exist in a database table column
    /// (validated asynchronously via [`crate::validation::validate_async`]).
    Unique(UniqueRule),
    /// The field value must exist in a database table column
    /// (validated asynchronously via [`crate::validation::validate_async`]).
    Exists(ExistsRule),
}

/// Parameters for the `unique` rule, mirroring Laravel's
/// `unique:table,column,except,idColumn`:
///
/// ```rust,ignore
/// validate_async(&data, vec![(
///     "email",
///     vec![unique_except("users", Some("email"), "42")],
/// )]).await?;
/// ```
#[derive(Clone, Debug)]
pub struct UniqueRule {
    pub table: String,
    /// Column to check; `None` falls back to the validated field name.
    pub column: Option<String>,
    /// Optional primary-key value to exclude (useful when updating a record).
    pub ignore_id: Option<String>,
}

/// Parameters for the `exists` rule, mirroring Laravel's `exists:table,column`.
#[derive(Clone, Debug)]
pub struct ExistsRule {
    pub table: String,
    /// Column to check; `None` falls back to the validated field name.
    pub column: Option<String>,
}

impl std::fmt::Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Required => write!(f, "Required"),
            Self::Email => write!(f, "Email"),
            Self::Min(n) => write!(f, "Min({})", n),
            Self::Max(n) => write!(f, "Max({})", n),
            Self::MinValue(n) => write!(f, "MinValue({})", n),
            Self::MaxValue(n) => write!(f, "MaxValue({})", n),
            Self::String => write!(f, "String"),
            Self::Numeric => write!(f, "Numeric"),
            Self::Boolean => write!(f, "Boolean"),
            Self::Confirmed => write!(f, "Confirmed"),
            Self::Same(s) => write!(f, "Same({})", s),
            Self::Different(s) => write!(f, "Different({})", s),
            Self::Alpha => write!(f, "Alpha"),
            Self::AlphaNumeric => write!(f, "AlphaNumeric"),
            Self::Url => write!(f, "Url"),
            Self::Ip => write!(f, "Ip"),
            Self::Base64 => write!(f, "Base64"),
            Self::Regex(_) => write!(f, "Regex"),
            Self::Between(a, b) => write!(f, "Between({}, {})", a, b),
            Self::Size(n) => write!(f, "Size({})", n),
            Self::Present => write!(f, "Present"),
            Self::Prohibited => write!(f, "Prohibited"),
            Self::Unique(r) => write!(
                f,
                "Unique({}.{})",
                r.table,
                r.column.as_deref().unwrap_or("*")
            ),
            Self::Exists(r) => write!(
                f,
                "Exists({}.{})",
                r.table,
                r.column.as_deref().unwrap_or("*")
            ),
            Self::Custom(rule) => write!(f, "Custom({})", rule.name()),
        }
    }
}

pub fn required() -> Rule {
    Rule::Required
}
pub fn email() -> Rule {
    Rule::Email
}
pub fn min(n: usize) -> Rule {
    Rule::Min(n)
}
pub fn max(n: usize) -> Rule {
    Rule::Max(n)
}
pub fn min_value(n: f64) -> Rule {
    Rule::MinValue(n)
}
pub fn max_value(n: f64) -> Rule {
    Rule::MaxValue(n)
}
pub fn string() -> Rule {
    Rule::String
}
pub fn numeric() -> Rule {
    Rule::Numeric
}
pub fn boolean() -> Rule {
    Rule::Boolean
}
pub fn confirmed() -> Rule {
    Rule::Confirmed
}
pub fn same(field: &str) -> Rule {
    Rule::Same(field.to_string())
}
pub fn different(field: &str) -> Rule {
    Rule::Different(field.to_string())
}
pub fn alpha() -> Rule {
    Rule::Alpha
}
pub fn alpha_numeric() -> Rule {
    Rule::AlphaNumeric
}
pub fn url() -> Rule {
    Rule::Url
}
pub fn ip() -> Rule {
    Rule::Ip
}

/// The field value must be valid base64 (standard alphabet, canonical
/// encoding with padding — mirrors Laravel 13's `validateBase64`).
pub fn base64() -> Rule {
    Rule::Base64
}
pub fn regex(pattern: &str) -> Result<Rule, regex::Error> {
    Regex::new(pattern).map(Rule::Regex)
}
pub fn between(min: usize, max: usize) -> Rule {
    Rule::Between(min, max)
}
pub fn size(n: usize) -> Rule {
    Rule::Size(n)
}
pub fn present() -> Rule {
    Rule::Present
}
pub fn prohibited() -> Rule {
    Rule::Prohibited
}

/// The field value must not already exist in the given database table.
///
/// `column` defaults to the validated field name when `None`.
///
/// ```rust,ignore
/// validate_async(&data, vec![("email", vec![unique("users", None)])]).await?;
/// ```
pub fn unique(table: &str, column: Option<&str>) -> Rule {
    Rule::Unique(UniqueRule {
        table: table.to_string(),
        column: column.map(|c| c.to_string()),
        ignore_id: None,
    })
}

/// Like [`unique`], but excludes a row by primary key — used when updating
/// a record so its own value does not trip the rule.
pub fn unique_except(table: &str, column: Option<&str>, ignore_id: &str) -> Rule {
    Rule::Unique(UniqueRule {
        table: table.to_string(),
        column: column.map(|c| c.to_string()),
        ignore_id: Some(ignore_id.to_string()),
    })
}

/// The field value must already exist in the given database table.
///
/// `column` defaults to the validated field name when `None`.
pub fn exists(table: &str, column: Option<&str>) -> Rule {
    Rule::Exists(ExistsRule {
        table: table.to_string(),
        column: column.map(|c| c.to_string()),
    })
}

pub fn custom(rule: Arc<dyn ValidationRule>) -> Rule {
    Rule::Custom(rule)
}

pub(crate) fn check_rule(
    rule: &Rule,
    field: &str,
    value: Option<&serde_json::Value>,
    all_data: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<String> {
    match rule {
        Rule::Required => match value {
            None | Some(serde_json::Value::Null) => {
                Some(format!("The {} field is required.", field))
            }
            Some(serde_json::Value::String(s)) if s.is_empty() => {
                Some(format!("The {} field is required.", field))
            }
            _ => None,
        },
        Rule::Email => {
            let s = value.and_then(|v| v.as_str())?;
            let email_regex =
                Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
            if !email_regex.is_match(s) {
                return Some(format!("The {} must be a valid email address.", field));
            }
            None
        }
        Rule::Min(n) => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if s.len() < *n {
                    return Some(format!("The {} must be at least {} characters.", field, n));
                }
            }
            None
        }
        Rule::Max(n) => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if s.len() > *n {
                    return Some(format!("The {} must not exceed {} characters.", field, n));
                }
            }
            None
        }
        Rule::MinValue(n) => {
            if let Some(num) = value.and_then(|v| v.as_f64()) {
                if num < *n {
                    return Some(format!("The {} must be at least {}.", field, n));
                }
            }
            None
        }
        Rule::MaxValue(n) => {
            if let Some(num) = value.and_then(|v| v.as_f64()) {
                if num > *n {
                    return Some(format!("The {} must not exceed {}.", field, n));
                }
            }
            None
        }
        Rule::String => {
            if let Some(v) = value {
                if !v.is_string() {
                    return Some(format!("The {} must be a string.", field));
                }
            }
            None
        }
        Rule::Numeric => {
            if let Some(v) = value {
                if !v.is_number() {
                    return Some(format!("The {} must be a number.", field));
                }
            }
            None
        }
        Rule::Boolean => {
            if let Some(v) = value {
                match v {
                    serde_json::Value::Bool(_) => {}
                    serde_json::Value::String(s)
                        if s == "true" || s == "false" || s == "1" || s == "0" => {}
                    serde_json::Value::Number(n)
                        if n.as_f64() == Some(1.0) || n.as_f64() == Some(0.0) => {}
                    _ => return Some(format!("The {} field must be true or false.", field)),
                }
            }
            None
        }
        Rule::Confirmed => {
            let confirmation = format!("{}_confirmation", field);
            let val = value.and_then(|v| v.as_str());
            let conf = all_data.get(&confirmation).and_then(|v| v.as_str());
            match (val, conf) {
                (Some(v), Some(c)) if v == c => None,
                _ => Some(format!("The {} confirmation does not match.", field)),
            }
        }
        Rule::Same(other) => {
            let val = value.and_then(|v| v.as_str());
            let other_val = all_data.get(other).and_then(|v| v.as_str());
            match (val, other_val) {
                (Some(v), Some(o)) if v == o => None,
                _ => Some(format!("The {} and {} must match.", field, other)),
            }
        }
        Rule::Different(other) => {
            let val = value.and_then(|v| v.as_str());
            let other_val = all_data.get(other).and_then(|v| v.as_str());
            match (val, other_val) {
                (Some(v), Some(o)) if v != o => None,
                _ => Some(format!("The {} and {} must be different.", field, other)),
            }
        }
        Rule::Alpha => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if !s.chars().all(|c| c.is_ascii_alphabetic()) {
                    return Some(format!("The {} must contain only letters.", field));
                }
            }
            None
        }
        Rule::AlphaNumeric => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Some(format!(
                        "The {} must contain only letters and numbers.",
                        field
                    ));
                }
            }
            None
        }
        Rule::Url => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if !s.starts_with("http://") && !s.starts_with("https://") {
                    return Some(format!("The {} must be a valid URL.", field));
                }
            }
            None
        }
        Rule::Ip => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                let ip_re = Regex::new(r"^(\d{1,3}\.){3}\d{1,3}$|^([0-9a-fA-F:]+)$").unwrap();
                if !ip_re.is_match(s) {
                    return Some(format!("The {} must be a valid IP address.", field));
                }
            }
            None
        }
        Rule::Base64 => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                use base64::Engine as _;
                let valid = base64::engine::general_purpose::STANDARD
                    .decode(s.as_bytes())
                    .ok()
                    .map(|decoded| base64::engine::general_purpose::STANDARD.encode(decoded) == s)
                    .unwrap_or(false);
                if !valid {
                    return Some(format!("The {} must be a valid base64 string.", field));
                }
            }
            None
        }
        Rule::Regex(re) => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if !re.is_match(s) {
                    return Some(format!("The {} format is invalid.", field));
                }
            }
            None
        }
        Rule::Between(min, max) => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if s.len() < *min || s.len() > *max {
                    return Some(format!(
                        "The {} must be between {} and {} characters.",
                        field, min, max
                    ));
                }
            }
            None
        }
        Rule::Size(n) => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if s.len() != *n {
                    return Some(format!("The {} must be exactly {} characters.", field, n));
                }
            }
            None
        }
        Rule::Present => {
            if value.is_none() {
                return Some(format!("The {} field must be present.", field));
            }
            None
        }
        Rule::Prohibited => {
            if value.is_some() && value != Some(&serde_json::Value::Null) {
                return Some(format!("The {} field is prohibited.", field));
            }
            None
        }
        Rule::Custom(rule) => {
            let val = value.and_then(|v| v.as_str()).unwrap_or("");
            rule.validate(field, val).err().map(|e| e.0)
        }
        Rule::Unique(_) | Rule::Exists(_) => Some(format!(
            "The {} field must be validated asynchronously (use validate_async).",
            field
        )),
    }
}

/// Async variant of [`check_rule`] that also resolves DB-backed rules
/// (`unique` / `exists`) against `db`.
pub(crate) async fn check_rule_async(
    rule: &Rule,
    field: &str,
    value: Option<&serde_json::Value>,
    all_data: &std::collections::HashMap<String, serde_json::Value>,
    db: &sea_orm::DatabaseConnection,
) -> Option<String> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    match rule {
        Rule::Unique(r) => {
            let raw = value?;
            let column = r.column.clone().unwrap_or_else(|| field.to_string());
            let value_str = match raw {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let sql = match &r.ignore_id {
                Some(_id) => format!(
                    "SELECT COUNT(*) FROM {} WHERE {} = ?1 AND id != ?2",
                    r.table, column
                ),
                None => format!("SELECT COUNT(*) FROM {} WHERE {} = ?1", r.table, column),
            };
            let mut values = vec![value_str.into()];
            if let Some(id) = &r.ignore_id {
                values.push(id.clone().into());
            }
            match db
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    &sql,
                    values,
                ))
                .await
            {
                Ok(Some(row)) => {
                    let count: i64 = row.try_get_by_index(0).unwrap_or(0);
                    if count > 0 {
                        Some(format!("The {} has already been taken.", field))
                    } else {
                        None
                    }
                }
                Ok(None) => None,
                Err(e) => Some(format!(
                    "The {} could not be validated against the database: {}",
                    field, e
                )),
            }
        }
        Rule::Exists(r) => {
            let raw = value?;
            let column = r.column.clone().unwrap_or_else(|| field.to_string());
            let value_str = match raw {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let sql = format!("SELECT COUNT(*) FROM {} WHERE {} = ?1", r.table, column);
            match db
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    &sql,
                    [value_str.into()],
                ))
                .await
            {
                Ok(Some(row)) => {
                    let count: i64 = row.try_get_by_index(0).unwrap_or(0);
                    if count == 0 {
                        Some(format!("The selected {} is invalid.", field))
                    } else {
                        None
                    }
                }
                Ok(None) => None,
                Err(e) => Some(format!(
                    "The {} could not be validated against the database: {}",
                    field, e
                )),
            }
        }
        _ => check_rule(rule, field, value, all_data),
    }
}
