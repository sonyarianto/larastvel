pub mod rules;

use std::collections::HashMap;

use axum::{
    extract::{FromRequest, Json, Query, Request},
    http::StatusCode,
    response::{IntoResponse, Json as JsonResponse, Response},
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use self::rules::check_rule;
pub use self::rules::{custom, Rule, ValidationError, ValidationRule};

#[derive(Debug, Clone)]
pub struct ValidationErrors {
    pub errors: HashMap<String, Vec<String>>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self {
            errors: HashMap::new(),
        }
    }

    pub fn add(&mut self, field: &str, message: &str) {
        self.errors
            .entry(field.to_string())
            .or_default()
            .push(message.to_string());
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has(&self, field: &str) -> bool {
        self.errors.contains_key(field)
    }

    pub fn all(&self) -> &HashMap<String, Vec<String>> {
        &self.errors
    }

    pub fn first(&self, field: &str) -> Option<&String> {
        self.errors.get(field).and_then(|e| e.first())
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({"errors": self.errors, "message": "Validation failed."})
    }
}

impl Default for ValidationErrors {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoResponse for ValidationErrors {
    fn into_response(self) -> Response {
        let status = StatusCode::UNPROCESSABLE_ENTITY;
        (status, JsonResponse(self.to_json())).into_response()
    }
}

pub struct Validator<'a> {
    data: &'a HashMap<String, Value>,
    rules: Vec<(&'a str, Vec<Rule>)>,
    custom_messages: HashMap<String, String>,
}

impl<'a> Validator<'a> {
    pub fn new(data: &'a HashMap<String, Value>, rules: Vec<(&'a str, Vec<Rule>)>) -> Self {
        Self {
            data,
            rules,
            custom_messages: HashMap::new(),
        }
    }

    pub fn with_messages(mut self, messages: HashMap<String, String>) -> Self {
        self.custom_messages = messages;
        self
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        for (field, field_rules) in &self.rules {
            let value = self.data.get(*field);
            for rule in field_rules {
                if let Some(msg) = check_rule(rule, field, value, self.data) {
                    let msg = self.custom_messages.get(*field).cloned().unwrap_or(msg);
                    errors.add(field, &msg);
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn passes(&self) -> bool {
        self.validate().is_ok()
    }

    pub fn fails(&self) -> bool {
        !self.passes()
    }
}

pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + 'static,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = Json::<T>::from_request(req, state).await.map_err(|e| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                JsonResponse(serde_json::json!({
                    "errors": {},
                    "message": format!("Invalid JSON: {}", e)
                })),
            )
                .into_response()
        })?;
        Ok(ValidatedJson(data))
    }
}

pub struct ValidatedQuery<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + 'static,
    S: Send + Sync,
{
    type Rejection = (StatusCode, JsonResponse<Value>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Query(data) = Query::<T>::from_request(req, state).await.map_err(|e| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                JsonResponse(serde_json::json!({
                    "errors": {},
                    "message": format!("Invalid query parameters: {}", e)
                })),
            )
        })?;
        Ok(ValidatedQuery(data))
    }
}

pub fn validate(
    data: &HashMap<String, Value>,
    rules: Vec<(&str, Vec<Rule>)>,
) -> Result<(), ValidationErrors> {
    let validator = Validator::new(data, rules);
    validator.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn data(pairs: Vec<(&str, Value)>) -> HashMap<String, Value> {
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    mod required {
        use super::*;

        #[test]
        fn passes_when_present() {
            let d = data(vec![("name", json!("John"))]);
            assert!(validate(&d, vec![("name", vec![rules::required()])]).is_ok());
        }

        #[test]
        fn fails_when_missing() {
            let d = data(vec![]);
            let err = validate(&d, vec![("name", vec![rules::required()])]).unwrap_err();
            assert!(err.has("name"));
        }

        #[test]
        fn fails_when_empty_string() {
            let d = data(vec![("name", json!(""))]);
            let err = validate(&d, vec![("name", vec![rules::required()])]).unwrap_err();
            assert!(err.has("name"));
        }

        #[test]
        fn fails_when_null() {
            let d = data(vec![("name", Value::Null)]);
            let err = validate(&d, vec![("name", vec![rules::required()])]).unwrap_err();
            assert!(err.has("name"));
        }
    }

    mod email {
        use super::*;

        #[test]
        fn passes_valid_email() {
            let d = data(vec![("email", json!("user@example.com"))]);
            assert!(validate(&d, vec![("email", vec![rules::email()])]).is_ok());
        }

        #[test]
        fn fails_invalid_email() {
            let d = data(vec![("email", json!("not-an-email"))]);
            let err = validate(&d, vec![("email", vec![rules::email()])]).unwrap_err();
            assert!(err.has("email"));
        }
    }

    mod min_max {
        use super::*;

        #[test]
        fn passes_min() {
            let d = data(vec![("name", json!("John"))]);
            assert!(validate(&d, vec![("name", vec![rules::min(3)])]).is_ok());
        }

        #[test]
        fn fails_min() {
            let d = data(vec![("name", json!("Jo"))]);
            let err = validate(&d, vec![("name", vec![rules::min(3)])]).unwrap_err();
            assert!(err.has("name"));
        }

        #[test]
        fn passes_max() {
            let d = data(vec![("name", json!("John"))]);
            assert!(validate(&d, vec![("name", vec![rules::max(10)])]).is_ok());
        }

        #[test]
        fn fails_max() {
            let d = data(vec![("name", json!("John Doe"))]);
            let err = validate(&d, vec![("name", vec![rules::max(5)])]).unwrap_err();
            assert!(err.has("name"));
        }

        #[test]
        fn passes_between() {
            let d = data(vec![("name", json!("John"))]);
            assert!(validate(&d, vec![("name", vec![rules::between(2, 10)])]).is_ok());
        }

        #[test]
        fn fails_between() {
            let d = data(vec![("name", json!("A"))]);
            let err = validate(&d, vec![("name", vec![rules::between(2, 10)])]).unwrap_err();
            assert!(err.has("name"));
        }
    }

    mod string_numeric {
        use super::*;

        #[test]
        fn passes_string() {
            let d = data(vec![("val", json!("hello"))]);
            assert!(validate(&d, vec![("val", vec![rules::string()])]).is_ok());
        }

        #[test]
        fn fails_string_for_number() {
            let d = data(vec![("val", json!(42))]);
            let err = validate(&d, vec![("val", vec![rules::string()])]).unwrap_err();
            assert!(err.has("val"));
        }

        #[test]
        fn passes_numeric() {
            let d = data(vec![("val", json!(42))]);
            assert!(validate(&d, vec![("val", vec![rules::numeric()])]).is_ok());
        }

        #[test]
        fn fails_numeric_for_string() {
            let d = data(vec![("val", json!("abc"))]);
            let err = validate(&d, vec![("val", vec![rules::numeric()])]).unwrap_err();
            assert!(err.has("val"));
        }
    }

    mod boolean_tests {
        use super::*;

        #[test]
        fn passes_boolean() {
            let d = data(vec![("val", json!(true))]);
            assert!(validate(&d, vec![("val", vec![rules::boolean()])]).is_ok());
        }

        #[test]
        fn fails_boolean_for_string() {
            let d = data(vec![("val", json!("yes"))]);
            let err = validate(&d, vec![("val", vec![rules::boolean()])]).unwrap_err();
            assert!(err.has("val"));
        }
    }

    mod confirmed {
        use super::*;

        #[test]
        fn passes_confirmed() {
            let d = data(vec![
                ("password", json!("secret")),
                ("password_confirmation", json!("secret")),
            ]);
            assert!(validate(&d, vec![("password", vec![rules::confirmed()])]).is_ok());
        }

        #[test]
        fn fails_confirmed_mismatch() {
            let d = data(vec![
                ("password", json!("secret")),
                ("password_confirmation", json!("different")),
            ]);
            let err = validate(&d, vec![("password", vec![rules::confirmed()])]).unwrap_err();
            assert!(err.has("password"));
        }

        #[test]
        fn fails_confirmed_missing() {
            let d = data(vec![("password", json!("secret"))]);
            let err = validate(&d, vec![("password", vec![rules::confirmed()])]).unwrap_err();
            assert!(err.has("password"));
        }
    }

    mod alpha {
        use super::*;

        #[test]
        fn passes_alpha() {
            let d = data(vec![("val", json!("John"))]);
            assert!(validate(&d, vec![("val", vec![rules::alpha()])]).is_ok());
        }

        #[test]
        fn fails_alpha_with_numbers() {
            let d = data(vec![("val", json!("John123"))]);
            let err = validate(&d, vec![("val", vec![rules::alpha()])]).unwrap_err();
            assert!(err.has("val"));
        }
    }

    mod url_ip {
        use super::*;

        #[test]
        fn passes_url() {
            let d = data(vec![("val", json!("https://example.com"))]);
            assert!(validate(&d, vec![("val", vec![rules::url()])]).is_ok());
        }

        #[test]
        fn fails_url() {
            let d = data(vec![("val", json!("not-a-url"))]);
            let err = validate(&d, vec![("val", vec![rules::url()])]).unwrap_err();
            assert!(err.has("val"));
        }

        #[test]
        fn passes_ip() {
            let d = data(vec![("val", json!("192.168.1.1"))]);
            assert!(validate(&d, vec![("val", vec![rules::ip()])]).is_ok());
        }

        #[test]
        fn fails_ip() {
            let d = data(vec![("val", json!("not-an-ip"))]);
            let err = validate(&d, vec![("val", vec![rules::ip()])]).unwrap_err();
            assert!(err.has("val"));
        }
    }

    mod min_max_value {
        use super::*;

        #[test]
        fn passes_min_value() {
            let d = data(vec![("age", json!(18))]);
            assert!(validate(&d, vec![("age", vec![rules::min_value(18.0)])]).is_ok());
        }

        #[test]
        fn fails_min_value() {
            let d = data(vec![("age", json!(15))]);
            let err = validate(&d, vec![("age", vec![rules::min_value(18.0)])]).unwrap_err();
            assert!(err.has("age"));
        }

        #[test]
        fn passes_max_value() {
            let d = data(vec![("age", json!(25))]);
            assert!(validate(&d, vec![("age", vec![rules::max_value(100.0)])]).is_ok());
        }

        #[test]
        fn fails_max_value() {
            let d = data(vec![("age", json!(150))]);
            let err = validate(&d, vec![("age", vec![rules::max_value(100.0)])]).unwrap_err();
            assert!(err.has("age"));
        }
    }

    mod same_different {
        use super::*;

        #[test]
        fn passes_same() {
            let d = data(vec![
                ("password", json!("secret")),
                ("password_confirm", json!("secret")),
            ]);
            assert!(validate(
                &d,
                vec![("password", vec![rules::same("password_confirm")])]
            )
            .is_ok());
        }

        #[test]
        fn fails_same() {
            let d = data(vec![
                ("password", json!("secret")),
                ("password_confirm", json!("different")),
            ]);
            let err = validate(
                &d,
                vec![("password", vec![rules::same("password_confirm")])],
            )
            .unwrap_err();
            assert!(err.has("password"));
        }

        #[test]
        fn passes_different() {
            let d = data(vec![
                ("password", json!("secret")),
                ("old_password", json!("oldsecret")),
            ]);
            assert!(validate(
                &d,
                vec![("password", vec![rules::different("old_password")])]
            )
            .is_ok());
        }

        #[test]
        fn fails_different() {
            let d = data(vec![
                ("password", json!("secret")),
                ("old_password", json!("secret")),
            ]);
            let err = validate(
                &d,
                vec![("password", vec![rules::different("old_password")])],
            )
            .unwrap_err();
            assert!(err.has("password"));
        }
    }

    mod size_present_prohibited {
        use super::*;

        #[test]
        fn passes_size() {
            let d = data(vec![("code", json!("ABC"))]);
            assert!(validate(&d, vec![("code", vec![rules::size(3)])]).is_ok());
        }

        #[test]
        fn fails_size() {
            let d = data(vec![("code", json!("AB"))]);
            let err = validate(&d, vec![("code", vec![rules::size(3)])]).unwrap_err();
            assert!(err.has("code"));
        }

        #[test]
        fn passes_present() {
            let d = data(vec![("field", json!("value"))]);
            assert!(validate(&d, vec![("field", vec![rules::present()])]).is_ok());
        }

        #[test]
        fn fails_present() {
            let d = data(vec![]);
            let err = validate(&d, vec![("field", vec![rules::present()])]).unwrap_err();
            assert!(err.has("field"));
        }

        #[test]
        fn passes_prohibited() {
            let d = data(vec![]);
            assert!(validate(&d, vec![("field", vec![rules::prohibited()])]).is_ok());
        }

        #[test]
        fn fails_prohibited() {
            let d = data(vec![("field", json!("value"))]);
            let err = validate(&d, vec![("field", vec![rules::prohibited()])]).unwrap_err();
            assert!(err.has("field"));
        }
    }

    mod validation_errors {
        use super::*;

        #[test]
        fn multiple_errors() {
            let d = data(vec![]);
            let err = validate(
                &d,
                vec![
                    ("name", vec![rules::required()]),
                    ("email", vec![rules::required(), rules::email()]),
                ],
            )
            .unwrap_err();
            assert!(err.has("name"));
            assert!(err.has("email"));
            assert_eq!(err.all().len(), 2);
        }

        #[test]
        fn to_json_format() {
            let mut errors = ValidationErrors::new();
            errors.add("name", "The name field is required.");
            errors.add("name", "The name must be at least 3 characters.");
            let json = errors.to_json();
            assert_eq!(json["errors"]["name"].as_array().unwrap().len(), 2);
            assert_eq!(json["message"], "Validation failed.");
        }

        #[test]
        fn first_error() {
            let mut errors = ValidationErrors::new();
            errors.add("name", "First error.");
            errors.add("name", "Second error.");
            assert_eq!(errors.first("name").unwrap(), "First error.");
        }

        #[test]
        fn is_empty() {
            let errors = ValidationErrors::new();
            assert!(errors.is_empty());
        }
    }

    mod validator_methods {
        use super::*;

        #[test]
        fn passes_returns_true() {
            let d = data(vec![("name", json!("John"))]);
            let v = Validator::new(&d, vec![("name", vec![rules::required()])]);
            assert!(v.passes());
            assert!(!v.fails());
        }

        #[test]
        fn fails_returns_true_when_invalid() {
            let d = data(vec![]);
            let v = Validator::new(&d, vec![("name", vec![rules::required()])]);
            assert!(v.fails());
            assert!(!v.passes());
        }
    }

    mod regex_rule {
        use super::*;

        #[test]
        fn passes_regex() {
            let re = rules::regex(r"^\d{3}-\d{2}-\d{4}$").unwrap();
            let d = data(vec![("ssn", json!("123-45-6789"))]);
            assert!(validate(&d, vec![("ssn", vec![re])]).is_ok());
        }

        #[test]
        fn fails_regex() {
            let re = rules::regex(r"^\d{3}-\d{2}-\d{4}$").unwrap();
            let d = data(vec![("ssn", json!("not-a-ssn"))]);
            let err = validate(&d, vec![("ssn", vec![re])]).unwrap_err();
            assert!(err.has("ssn"));
        }
    }

    mod alpha_numeric {
        use super::*;

        #[test]
        fn passes_alpha_numeric() {
            let d = data(vec![("username", json!("john123"))]);
            assert!(validate(&d, vec![("username", vec![rules::alpha_numeric()])]).is_ok());
        }

        #[test]
        fn fails_alpha_numeric() {
            let d = data(vec![("username", json!("john@123"))]);
            let err = validate(&d, vec![("username", vec![rules::alpha_numeric()])]).unwrap_err();
            assert!(err.has("username"));
        }
    }

    // -------------------------------------------------------------------------
    // Custom rule tests
    // -------------------------------------------------------------------------

    mod custom_rule {
        use super::*;
        use crate::rule;
        use std::sync::Arc;

        #[derive(Debug, Clone)]
        struct UpperCaseRule;

        #[rule]
        impl UpperCaseRule {
            fn validate(&self, field: &str, value: &str) -> Result<(), ValidationError> {
                if value.chars().any(|c| c.is_lowercase()) {
                    return Err(ValidationError::new(format!(
                        "The {} must be uppercase.",
                        field
                    )));
                }
                Ok(())
            }
        }

        #[test]
        fn rule_name_derived_from_struct() {
            let rule = UpperCaseRule;
            assert_eq!(
                <UpperCaseRule as ValidationRule>::name(&rule),
                "upper_case_rule"
            );
        }

        #[test]
        fn custom_rule_validates_correctly() {
            let rule = UpperCaseRule;
            assert!(rule.validate("code", "ABC").is_ok());
            assert!(rule.validate("code", "abc").is_err());
        }

        #[test]
        fn custom_rule_works_with_validate_function() {
            let d = data(vec![("code", json!("ABC"))]);
            assert!(validate(&d, vec![("code", vec![custom(Arc::new(UpperCaseRule))])]).is_ok());

            let d = data(vec![("code", json!("abc"))]);
            let err =
                validate(&d, vec![("code", vec![custom(Arc::new(UpperCaseRule))])]).unwrap_err();
            assert!(err.has("code"));
        }

        #[test]
        fn custom_rule_mixed_with_builtin() {
            let d = data(vec![("code", json!("ABC"))]);
            assert!(validate(
                &d,
                vec![(
                    "code",
                    vec![rules::required(), custom(Arc::new(UpperCaseRule))]
                )]
            )
            .is_ok());

            let d = data(vec![("code", json!("abc"))]);
            let err = validate(
                &d,
                vec![(
                    "code",
                    vec![rules::required(), custom(Arc::new(UpperCaseRule))],
                )],
            )
            .unwrap_err();
            assert!(err.has("code"));
        }
    }
}
