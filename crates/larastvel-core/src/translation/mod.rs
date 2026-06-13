use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

static GLOBAL_TRANSLATOR: OnceLock<Mutex<Translator>> = OnceLock::new();

fn global_translator() -> &'static Mutex<Translator> {
    GLOBAL_TRANSLATOR.get_or_init(|| Mutex::new(Translator::new("en", "en")))
}

pub struct TranslationConfig {
    pub locale: String,
    pub fallback_locale: String,
    pub lang_path: Option<String>,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            locale: "en".to_string(),
            fallback_locale: "en".to_string(),
            lang_path: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Translator {
    locale: String,
    fallback_locale: String,
    translations: HashMap<String, HashMap<String, Value>>,
    fallback_translations: HashMap<String, Value>,
    lang_path: Option<String>,
}

impl Translator {
    pub fn new(locale: &str, fallback_locale: &str) -> Self {
        Self {
            locale: locale.to_string(),
            fallback_locale: fallback_locale.to_string(),
            translations: HashMap::new(),
            fallback_translations: HashMap::new(),
            lang_path: None,
        }
    }

    pub fn with_lang_path(mut self, path: &str) -> Self {
        self.lang_path = Some(path.to_string());
        self
    }

    pub fn set_locale(&mut self, locale: &str) {
        self.locale = locale.to_string();
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn set_fallback_locale(&mut self, locale: &str) {
        self.fallback_locale = locale.to_string();
    }

    pub fn fallback_locale(&self) -> &str {
        &self.fallback_locale
    }

    pub fn load_json(&mut self, locale: &str, json: &str) {
        let parsed: Value = serde_json::from_str(json).unwrap_or(Value::Null);
        if let Value::Object(map) = parsed {
            let flat = flatten_json(&map, "");
            if locale == self.fallback_locale {
                self.fallback_translations = flat;
            } else {
                self.translations.insert(locale.to_string(), flat.clone());
                if locale == self.locale {
                    self.fallback_translations = flat;
                }
            }
        }
    }

    pub fn load_file(&mut self, locale: &str, path: &str) -> Result<(), String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.load_json(locale, &content);
        Ok(())
    }

    pub fn load_directory(&mut self, dir: &str) -> Result<(), String> {
        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    self.load_file(stem, path.to_str().unwrap())?;
                }
            }
        }
        Ok(())
    }

    fn resolve(&self, key: &str, locale: &str) -> Option<&str> {
        if locale == self.fallback_locale {
            return self.fallback_translations.get(key).and_then(|v| v.as_str());
        }
        if let Some(trans) = self.translations.get(locale) {
            if let Some(val) = trans.get(key).and_then(|v| v.as_str()) {
                return Some(val);
            }
        }
        if locale != self.fallback_locale {
            return self.fallback_translations.get(key).and_then(|v| v.as_str());
        }
        None
    }

    fn replace_params(template: &str, params: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in params {
            result = result.replace(&format!(":{}", key), value);
        }
        result
    }

    pub fn get(&self, key: &str, replace: Option<HashMap<String, String>>) -> String {
        let template = self
            .resolve(key, &self.locale)
            .or_else(|| self.resolve(key, &self.fallback_locale))
            .unwrap_or(key);

        if let Some(params) = replace {
            Self::replace_params(template, &params)
        } else {
            template.to_string()
        }
    }

    pub fn choice(&self, key: &str, count: i64, replace: Option<HashMap<String, String>>) -> String {
        let template = self
            .resolve(key, &self.locale)
            .or_else(|| self.resolve(key, &self.fallback_locale))
            .unwrap_or(key);

        let translated = select_plural(template, count);

        if let Some(params) = replace {
            Self::replace_params(&translated, &params)
        } else {
            translated
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.resolve(key, &self.locale).is_some()
            || self.resolve(key, &self.fallback_locale).is_some()
    }

pub fn has_for_locale(&self, key: &str, locale: &str) -> bool {
    if locale == self.fallback_locale {
        return self.fallback_translations.get(key).and_then(|v| v.as_str()).is_some();
    }
    if let Some(trans) = self.translations.get(locale) {
        if trans.get(key).and_then(|v| v.as_str()).is_some() {
            return true;
        }
    }
    false
}
}

pub fn set_locale(locale: &str) {
    let mut t = global_translator().lock().unwrap();
    t.set_locale(locale);
}

pub fn locale() -> String {
    let t = global_translator().lock().unwrap();
    t.locale().to_string()
}

pub fn set_fallback_locale(locale: &str) {
    let mut t = global_translator().lock().unwrap();
    t.set_fallback_locale(locale);
}

pub fn load_translation_json(locale: &str, json: &str) {
    let mut t = global_translator().lock().unwrap();
    t.load_json(locale, json);
}

pub fn load_translation_file(locale: &str, path: &str) -> Result<(), String> {
    let mut t = global_translator().lock().unwrap();
    t.load_file(locale, path)
}

pub fn load_translation_directory(dir: &str) -> Result<(), String> {
    let mut t = global_translator().lock().unwrap();
    t.load_directory(dir)
}

pub fn __(key: &str) -> String {
    let t = global_translator().lock().unwrap();
    t.get(key, None)
}

pub fn __with(key: &str, replace: HashMap<String, String>) -> String {
    let t = global_translator().lock().unwrap();
    t.get(key, Some(replace))
}

pub fn trans_choice(key: &str, count: i64) -> String {
    let t = global_translator().lock().unwrap();
    t.choice(key, count, None)
}

pub fn trans_choice_with(key: &str, count: i64, replace: HashMap<String, String>) -> String {
    let t = global_translator().lock().unwrap();
    t.choice(key, count, Some(replace))
}

pub fn has_translation(key: &str) -> bool {
    let t = global_translator().lock().unwrap();
    t.has(key)
}

fn flatten_json(map: &serde_json::Map<String, Value>, prefix: &str) -> HashMap<String, Value> {
    let mut result = HashMap::new();
    for (key, value) in map {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };
        match value {
            Value::Object(inner) => {
                result.extend(flatten_json(inner, &full_key));
            }
            _ => {
                result.insert(full_key, value.clone());
            }
        }
    }
    result
}

fn select_plural(template: &str, count: i64) -> String {
    let segments: Vec<&str> = template.split('|').collect();
    if segments.len() <= 1 {
        return template.replace(":count", &count.to_string());
    }

    let mut has_bracket = false;
    for segment in &segments {
        let segment = segment.trim();

        let extract = segment
            .split_once(']')
            .or_else(|| segment.split_once('}').map(|(a, b)| (a, b)));

        if let Some((range_part, message)) = extract {
            has_bracket = true;
            let open = range_part.find(|c| c == '[' || c == '{');
            let range = match open {
                Some(idx) => &range_part[idx + 1..],
                None => continue,
            };
            let message = message.trim();

            if let Some((start, end)) = range.split_once(',') {
                let start_val: i64 = start.trim().parse().unwrap_or(0);
                let end_str = end.trim();
                if end_str == "*" {
                    if count >= start_val {
                        return message.replace(":count", &count.to_string());
                    }
                } else {
                    let end_val: i64 = end_str.parse().unwrap_or(i64::MAX);
                    if count >= start_val && count <= end_val {
                        return message.replace(":count", &count.to_string());
                    }
                }
            } else {
                let single: i64 = range.trim().parse().unwrap_or(0);
                if count == single {
                    return message.replace(":count", &count.to_string());
                }
            }
        }
    }

    if !has_bracket {
        if count == 1 {
            return segments[0].trim().replace(":count", &count.to_string());
        }
        if segments.len() >= 2 {
            return segments[1].trim().replace(":count", &count.to_string());
        }
    }

    segments.last().unwrap_or(&template).trim().replace(":count", &count.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_translator() -> Translator {
        let mut t = Translator::new("en", "en");
        t.load_json("en", r#"{
            "hello": "Hello World",
            "greeting": "Hello :name",
            "auth": {
                "failed": "These credentials do not match our records.",
                "throttle": "Too many attempts. Please try again in :seconds seconds."
            },
            "messages": {
                "welcome": "Welcome to :app"
            },
            "apples": "{0} No apples|{1} One apple|[2,*] :count apples"
        }"#);
        t.load_json("id", r#"{
            "hello": "Halo Dunia",
            "greeting": "Halo :name",
            "auth": {
                "failed": "Kredensial ini tidak cocok dengan catatan kami."
            }
        }"#);
        t
    }

    #[test]
    fn test_simple_key() {
        let t = setup_translator();
        assert_eq!(t.get("hello", None), "Hello World");
    }

    #[test]
    fn test_key_with_replacement() {
        let t = setup_translator();
        let params = HashMap::from([("name".to_string(), "Taylor".to_string())]);
        assert_eq!(t.get("greeting", Some(params)), "Hello Taylor");
    }

    #[test]
    fn test_dot_notation() {
        let t = setup_translator();
        assert_eq!(
            t.get("auth.failed", None),
            "These credentials do not match our records."
        );
    }

    #[test]
    fn test_dot_notation_with_replacement() {
        let t = setup_translator();
        let params = HashMap::from([("seconds".to_string(), "60".to_string())]);
        assert_eq!(
            t.get("auth.throttle", Some(params)),
            "Too many attempts. Please try again in 60 seconds."
        );
    }

    #[test]
    fn test_nested_dot_notation() {
        let t = setup_translator();
        let params = HashMap::from([("app".to_string(), "Larastvel".to_string())]);
        assert_eq!(t.get("messages.welcome", Some(params)), "Welcome to Larastvel");
    }

    #[test]
    fn test_missing_key_returns_key() {
        let t = setup_translator();
        assert_eq!(t.get("nonexistent.key", None), "nonexistent.key");
    }

    #[test]
    fn test_locale_switching() {
        let mut t = setup_translator();
        assert_eq!(t.get("hello", None), "Hello World");
        t.set_locale("id");
        assert_eq!(t.get("hello", None), "Halo Dunia");
    }

    #[test]
    fn test_fallback_to_english() {
        let mut t = setup_translator();
        t.set_locale("id");
        assert_eq!(
            t.get("auth.throttle", None),
            "Too many attempts. Please try again in :seconds seconds."
        );
    }

    #[test]
    fn test_locale_missing_entirely() {
        let mut t = setup_translator();
        t.set_locale("fr");
        assert_eq!(t.get("hello", None), "Hello World");
    }

    #[test]
    fn test_plural_zero() {
        let t = setup_translator();
        assert_eq!(t.choice("apples", 0, None), "No apples");
    }

    #[test]
    fn test_plural_one() {
        let t = setup_translator();
        assert_eq!(t.choice("apples", 1, None), "One apple");
    }

    #[test]
    fn test_plural_many() {
        let t = setup_translator();
        assert_eq!(t.choice("apples", 5, None), "5 apples");
    }

    #[test]
    fn test_plural_with_replacement() {
        let t = setup_translator();
        assert_eq!(t.choice("apples", 10, None), "10 apples");
    }

    #[test]
    fn test_has_translation() {
        let t = setup_translator();
        assert!(t.has("hello"));
        assert!(t.has("auth.failed"));
        assert!(!t.has("nothing.here"));
    }

    #[test]
    fn test_has_for_locale() {
        let t = setup_translator();
        assert!(t.has_for_locale("hello", "en"));
        assert!(t.has_for_locale("hello", "id"));
        assert!(!t.has_for_locale("hello", "fr"));
    }

    #[test]
    fn test_global_functions() {
        load_translation_json("en", r#"{"hello": "Hello"}"#);
        set_locale("en");
        assert_eq!(__("hello"), "Hello");
    }

    #[test]
    fn test_global_locale() {
        set_locale("en");
        assert_eq!(locale(), "en");
        set_locale("id");
        assert_eq!(locale(), "id");
        set_locale("en");
    }

    #[test]
    fn test_global_fallback() {
        set_fallback_locale("en");
        assert_eq!(super::super::translation::Translator::new("en", "en").fallback_locale(), "en");
    }

    #[test]
    fn test_flatten_json() {
        let mut map = serde_json::Map::new();
        map.insert("auth".to_string(), Value::Object({
            let mut m = serde_json::Map::new();
            m.insert("failed".to_string(), Value::String("Failed".to_string()));
            m
        }));
        let flat = flatten_json(&map, "");
        assert_eq!(flat.get("auth.failed").and_then(|v| v.as_str()), Some("Failed"));
    }

    #[test]
    fn test_choice_missing_key() {
        let t = setup_translator();
        assert_eq!(t.choice("nonexistent", 1, None), "nonexistent");
    }

    #[test]
    fn test_choice_simple_plural() {
        let mut t = Translator::new("en", "en");
        t.load_json("en", r#"{"items": "item|items"}"#);
        assert_eq!(t.choice("items", 1, None), "item");
    }

    #[test]
    fn test_translator_config_default() {
        let config = TranslationConfig::default();
        assert_eq!(config.locale, "en");
        assert_eq!(config.fallback_locale, "en");
    }

    #[test]
    fn test_has_translation_global() {
        load_translation_json("en", r#"{"exists": "yes"}"#);
        set_locale("en");
        assert!(has_translation("exists"));
        assert!(!has_translation("nope"));
    }

    #[test]
    fn test_load_directory_nonexistent() {
        let mut t = Translator::new("en", "en");
        assert!(t.load_directory("/tmp/nonexistent_lang_dir_12345").is_ok());
    }

    #[test]
    fn test_load_file_nonexistent() {
        let mut t = Translator::new("en", "en");
        assert!(t.load_file("en", "/tmp/nonexistent.json").is_err());
    }
}
