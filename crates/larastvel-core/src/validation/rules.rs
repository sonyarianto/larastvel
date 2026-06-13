use regex::Regex;

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
    Regex(Regex),
    Between(usize, usize),
    Size(usize),
    Present,
    Prohibited,
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

pub(crate) fn check_rule(
    rule: &Rule,
    field: &str,
    value: Option<&serde_json::Value>,
    all_data: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<String> {
    match rule {
        Rule::Required => {
            match value {
                None | Some(serde_json::Value::Null) => {
                    Some(format!("The {} field is required.", field))
                }
                Some(serde_json::Value::String(s)) if s.is_empty() => {
                    Some(format!("The {} field is required.", field))
                }
                _ => None,
            }
        }
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
                    return Some(format!(
                        "The {} must be at least {} characters.",
                        field, n
                    ));
                }
            }
            None
        }
        Rule::Max(n) => {
            if let Some(s) = value.and_then(|v| v.as_str()) {
                if s.len() > *n {
                    return Some(format!(
                        "The {} must not exceed {} characters.",
                        field, n
                    ));
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
                    serde_json::Value::String(s) if s == "true" || s == "false" || s == "1" || s == "0" => {}
                    serde_json::Value::Number(n) if n.as_f64() == Some(1.0) || n.as_f64() == Some(0.0) => {}
                    _ => return Some(format!("The {} field must be true or false.", field)),
                }
            }
            None
        }
        Rule::Confirmed => {
            let confirmation = format!("{}_confirmation", field);
            let val = value.and_then(|v| v.as_str());
            let conf = all_data
                .get(&confirmation)
                .and_then(|v| v.as_str());
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
                    return Some(format!("The {} must contain only letters and numbers.", field));
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
                let ip_re = Regex::new(
                    r"^(\d{1,3}\.){3}\d{1,3}$|^([0-9a-fA-F:]+)$",
                )
                .unwrap();
                if !ip_re.is_match(s) {
                    return Some(format!("The {} must be a valid IP address.", field));
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
                    return Some(format!(
                        "The {} must be exactly {} characters.",
                        field, n
                    ));
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
    }
}
