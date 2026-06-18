use super::str::{capitalize, words};

pub struct Stringable {
    value: String,
}

impl Stringable {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn into_value(self) -> String {
        self.value
    }
}

impl Stringable {
    pub fn slug(self, separator: &str) -> Self {
        let s = self.value.to_lowercase();
        let mut result = String::with_capacity(s.len());
        let mut prev_was_sep = true;
        for c in s.chars() {
            if c.is_ascii_alphanumeric() {
                result.push(c);
                prev_was_sep = false;
            } else if !prev_was_sep {
                result.push_str(separator);
                prev_was_sep = true;
            }
        }
        if result.ends_with(separator) && separator.len() <= result.len() {
            result.truncate(result.len() - separator.len());
        }
        Self { value: result }
    }

    pub fn studly(self) -> Self {
        Self {
            value: words(&self.value)
                .into_iter()
                .map(|w| capitalize(&w))
                .collect(),
        }
    }

    pub fn camel(self) -> Self {
        let studly = words(&self.value)
            .into_iter()
            .map(|w| capitalize(&w))
            .collect::<String>();
        let mut result = studly;
        if let Some(c) = result.chars().next() {
            result.replace_range(..c.len_utf8(), &c.to_lowercase().to_string());
        }
        Self { value: result }
    }

    pub fn snake(self) -> Self {
        Self {
            value: words(&self.value).join("_"),
        }
    }

    pub fn kebab(self) -> Self {
        Self {
            value: words(&self.value).join("-"),
        }
    }

    pub fn title(self) -> Self {
        let small_words = [
            "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "by",
            "with", "nor", "as", "is", "it",
        ];
        let words: Vec<String> = words(&self.value);
        let mut result = Vec::with_capacity(words.len());
        for (i, w) in words.iter().enumerate() {
            let lower = w.to_lowercase();
            if i > 0 && i < words.len() - 1 && small_words.contains(&lower.as_str()) {
                result.push(lower);
            } else {
                result.push(capitalize(w));
            }
        }
        Self {
            value: result.join(" "),
        }
    }

    pub fn headline(self) -> Self {
        let s = self.value.replace('_', " ");
        let s = s.replace('-', " ");
        let words: Vec<&str> = s.split_whitespace().collect();
        Self {
            value: words
                .into_iter()
                .map(|w| {
                    let w = w.to_lowercase();
                    capitalize(&w)
                })
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    pub fn ucfirst(self) -> Self {
        let mut chars = self.value.chars();
        match chars.next() {
            None => Self {
                value: String::new(),
            },
            Some(c) => Self {
                value: c.to_uppercase().to_string() + chars.as_str(),
            },
        }
    }

    pub fn lcfirst(self) -> Self {
        let mut chars = self.value.chars();
        match chars.next() {
            None => Self {
                value: String::new(),
            },
            Some(c) => Self {
                value: c.to_lowercase().to_string() + chars.as_str(),
            },
        }
    }

    pub fn upper(self) -> Self {
        Self {
            value: self.value.to_uppercase(),
        }
    }

    pub fn lower(self) -> Self {
        Self {
            value: self.value.to_lowercase(),
        }
    }

    pub fn trim(self) -> Self {
        Self {
            value: self.value.trim().to_string(),
        }
    }

    pub fn trim_slashes(self) -> Self {
        Self {
            value: self.value.trim_matches('/').to_string(),
        }
    }

    pub fn replace(self, search: &str, replace: &str) -> Self {
        Self {
            value: self.value.replace(search, replace),
        }
    }

    pub fn replace_first(self, search: &str, replace: &str) -> Self {
        Self {
            value: self.value.replacen(search, replace, 1),
        }
    }

    pub fn repeat(self, times: usize) -> Self {
        Self {
            value: self.value.repeat(times),
        }
    }

    pub fn after(self, search: &str) -> Self {
        Self {
            value: self
                .value
                .split_once(search)
                .map(|(_, after)| after.to_string())
                .unwrap_or_default(),
        }
    }

    pub fn before(self, search: &str) -> Self {
        Self {
            value: self
                .value
                .split_once(search)
                .map(|(before, _)| before.to_string())
                .unwrap_or(self.value),
        }
    }

    pub fn between(self, from: &str, to: &str) -> Self {
        self.after(from).before(to)
    }

    pub fn limit(self, limit: usize, end: &str) -> Self {
        if self.value.len() <= limit {
            return self;
        }
        let mut result = self.value[..limit].to_string();
        result.push_str(end);
        Self { value: result }
    }

    pub fn substr(self, start: usize, length: Option<usize>) -> Self {
        Self {
            value: self
                .value
                .chars()
                .skip(start)
                .take(length.unwrap_or(usize::MAX))
                .collect(),
        }
    }

    pub fn pad_left(self, length: usize, pad: &str) -> Self {
        let current = self.value.chars().count();
        if current >= length {
            return self;
        }
        let needed = length - current;
        let mut result = String::with_capacity(length);
        let pad_chars: Vec<char> = pad.chars().collect();
        for i in 0..needed {
            result.push(pad_chars[i % pad_chars.len()]);
        }
        result.push_str(&self.value);
        Self { value: result }
    }

    pub fn pad_right(self, length: usize, pad: &str) -> Self {
        let current = self.value.chars().count();
        if current >= length {
            return self;
        }
        let needed = length - current;
        let mut result = self.value;
        let pad_chars: Vec<char> = pad.chars().collect();
        for i in 0..needed {
            result.push(pad_chars[i % pad_chars.len()]);
        }
        Self { value: result }
    }

    pub fn pad_both(self, length: usize, pad: &str) -> Self {
        let current = self.value.chars().count();
        if current >= length {
            return self;
        }
        let needed = length - current;
        let left = needed / 2;
        let right = needed - left;
        let mut result = String::with_capacity(length);
        let pad_chars: Vec<char> = pad.chars().collect();
        for i in 0..left {
            result.push(pad_chars[i % pad_chars.len()]);
        }
        result.push_str(&self.value);
        for i in 0..right {
            result.push(pad_chars[(left + i) % pad_chars.len()]);
        }
        Self { value: result }
    }

    pub fn mask(self, character: &str, start: usize, length: usize) -> Self {
        let chars: Vec<char> = self.value.chars().collect();
        let mut result = String::with_capacity(self.value.len());
        let mask_char = character.chars().next().unwrap_or('*');
        for (i, &c) in chars.iter().enumerate() {
            if i >= start && i < start + length {
                result.push(mask_char);
            } else {
                result.push(c);
            }
        }
        Self { value: result }
    }

    pub fn finish(self, cap: &str) -> Self {
        if self.value.ends_with(cap) {
            self
        } else {
            Self {
                value: format!("{}{}", self.value, cap),
            }
        }
    }

    pub fn start(self, prefix: &str) -> Self {
        if self.value.starts_with(prefix) {
            self
        } else {
            Self {
                value: format!("{}{}", prefix, self.value),
            }
        }
    }

    pub fn contains(&self, needle: &str) -> bool {
        self.value.contains(needle)
    }

    pub fn contains_all(&self, needles: &[&str]) -> bool {
        needles.iter().all(|n| self.value.contains(*n))
    }

    pub fn starts_with(&self, needle: &str) -> bool {
        self.value.starts_with(needle)
    }

    pub fn ends_with(&self, needle: &str) -> bool {
        self.value.ends_with(needle)
    }

    pub fn position(&self, needle: &str, offset: usize) -> Option<usize> {
        if offset >= self.value.len() {
            return None;
        }
        let search = &self.value[offset..];
        search.find(needle).map(|pos| pos + offset)
    }

    pub fn length(&self) -> usize {
        self.value.chars().count()
    }

    pub fn is_ascii(&self) -> bool {
        self.value.is_ascii()
    }

    pub fn is_json(&self) -> bool {
        serde_json::from_str::<serde_json::Value>(&self.value).is_ok()
    }

    pub fn is_url(&self) -> bool {
        self.value.starts_with("http://") || self.value.starts_with("https://")
    }

    pub fn is_uuid(&self) -> bool {
        let s = self.value.trim();
        if s.len() != 36 {
            return false;
        }
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 5 {
            return false;
        }
        parts[0].len() == 8
            && parts[1].len() == 4
            && parts[2].len() == 4
            && parts[3].len() == 4
            && parts[4].len() == 12
            && parts
                .iter()
                .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
    }

    pub fn word_count(&self) -> usize {
        self.value
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .count()
    }

    pub fn append(self, suffix: &str) -> Self {
        Self {
            value: format!("{}{}", self.value, suffix),
        }
    }

    pub fn prepend(self, prefix: &str) -> Self {
        Self {
            value: format!("{}{}", prefix, self.value),
        }
    }
}

impl std::fmt::Display for Stringable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl From<Stringable> for String {
    fn from(s: Stringable) -> Self {
        s.value
    }
}

impl From<String> for Stringable {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl<'a> From<&'a str> for Stringable {
    fn from(value: &'a str) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_studly() {
        let result = Stringable::new("hello_world").studly();
        assert_eq!(result.value(), "HelloWorld");
    }

    #[test]
    fn test_camel() {
        let result = Stringable::new("hello_world").camel();
        assert_eq!(result.value(), "helloWorld");
    }

    #[test]
    fn test_slug() {
        let result = Stringable::new("Hello World").slug("-");
        assert_eq!(result.value(), "hello-world");
    }

    #[test]
    fn test_chain() {
        let result = Stringable::new("hello_world").studly().upper();
        assert_eq!(result.value(), "HELLOWORLD");
    }

    #[test]
    fn test_chain_full() {
        let result = Stringable::new("  Hello World  ").trim().slug("-");
        assert_eq!(result.value(), "hello-world");
    }

    #[test]
    fn test_upper() {
        let result = Stringable::new("hello").upper();
        assert_eq!(result.value(), "HELLO");
    }

    #[test]
    fn test_lower() {
        let result = Stringable::new("HELLO").lower();
        assert_eq!(result.value(), "hello");
    }

    #[test]
    fn test_snake() {
        let result = Stringable::new("HelloWorld").snake();
        assert_eq!(result.value(), "hello_world");
    }

    #[test]
    fn test_kebab() {
        let result = Stringable::new("HelloWorld").kebab();
        assert_eq!(result.value(), "hello-world");
    }

    #[test]
    fn test_title() {
        let result = Stringable::new("the quick brown fox").title();
        assert_eq!(result.value(), "The Quick Brown Fox");
    }

    #[test]
    fn test_headline() {
        let result = Stringable::new("hello_world").headline();
        assert_eq!(result.value(), "Hello World");
    }

    #[test]
    fn test_ucfirst() {
        let result = Stringable::new("hello").ucfirst();
        assert_eq!(result.value(), "Hello");
        assert_eq!(Stringable::new("").ucfirst().value(), "");
    }

    #[test]
    fn test_lcfirst() {
        let result = Stringable::new("Hello").lcfirst();
        assert_eq!(result.value(), "hello");
    }

    #[test]
    fn test_trim() {
        let result = Stringable::new("  hello  ").trim();
        assert_eq!(result.value(), "hello");
    }

    #[test]
    fn test_trim_slashes() {
        let result = Stringable::new("/hello/").trim_slashes();
        assert_eq!(result.value(), "hello");
    }

    #[test]
    fn test_replace() {
        let result = Stringable::new("Hello World").replace("World", "Moon");
        assert_eq!(result.value(), "Hello Moon");
    }

    #[test]
    fn test_replace_first() {
        let result = Stringable::new("foo bar foo").replace_first("foo", "baz");
        assert_eq!(result.value(), "baz bar foo");
    }

    #[test]
    fn test_repeat() {
        let result = Stringable::new("ab").repeat(3);
        assert_eq!(result.value(), "ababab");
    }

    #[test]
    fn test_after() {
        let result = Stringable::new("Hello World").after("Hello ");
        assert_eq!(result.value(), "World");
    }

    #[test]
    fn test_before() {
        let result = Stringable::new("Hello World").before(" World");
        assert_eq!(result.value(), "Hello");
    }

    #[test]
    fn test_between() {
        let result = Stringable::new("[Hello]").between("[", "]");
        assert_eq!(result.value(), "Hello");
    }

    #[test]
    fn test_limit() {
        let result = Stringable::new("Hello World").limit(5, "...");
        assert_eq!(result.value(), "Hello...");
        let result = Stringable::new("Hi").limit(5, "...");
        assert_eq!(result.value(), "Hi");
    }

    #[test]
    fn test_substr() {
        let result = Stringable::new("Hello World").substr(0, Some(5));
        assert_eq!(result.value(), "Hello");
        let result = Stringable::new("Hello World").substr(6, None);
        assert_eq!(result.value(), "World");
    }

    #[test]
    fn test_pad_left() {
        let result = Stringable::new("Hello").pad_left(7, "*");
        assert_eq!(result.value(), "**Hello");
    }

    #[test]
    fn test_pad_right() {
        let result = Stringable::new("Hello").pad_right(7, "*");
        assert_eq!(result.value(), "Hello**");
    }

    #[test]
    fn test_pad_both() {
        let result = Stringable::new("Hello").pad_both(9, "*");
        assert_eq!(result.value(), "**Hello**");
    }

    #[test]
    fn test_mask() {
        let result = Stringable::new("1234-5678").mask("*", 0, 4);
        assert_eq!(result.value(), "****-5678");
    }

    #[test]
    fn test_finish() {
        let result = Stringable::new("hello").finish("/");
        assert_eq!(result.value(), "hello/");
        let result = Stringable::new("hello/").finish("/");
        assert_eq!(result.value(), "hello/");
    }

    #[test]
    fn test_start() {
        let result = Stringable::new("world").start("hello ");
        assert_eq!(result.value(), "hello world");
    }

    #[test]
    fn test_contains() {
        assert!(Stringable::new("Hello World").contains("World"));
        assert!(!Stringable::new("Hello World").contains("world"));
    }

    #[test]
    fn test_contains_all() {
        assert!(Stringable::new("Hello World").contains_all(&["Hello", "World"]));
        assert!(!Stringable::new("Hello World").contains_all(&["Hello", "world"]));
    }

    #[test]
    fn test_starts_with() {
        assert!(Stringable::new("Hello World").starts_with("Hello"));
        assert!(!Stringable::new("Hello World").starts_with("World"));
    }

    #[test]
    fn test_ends_with() {
        assert!(Stringable::new("Hello World").ends_with("World"));
        assert!(!Stringable::new("Hello World").ends_with("Hello"));
    }

    #[test]
    fn test_length() {
        assert_eq!(Stringable::new("Hello").length(), 5);
        assert_eq!(Stringable::new("日本語").length(), 3);
    }

    #[test]
    fn test_is_ascii() {
        assert!(Stringable::new("Hello").is_ascii());
        assert!(!Stringable::new("日本語").is_ascii());
    }

    #[test]
    fn test_is_json() {
        assert!(Stringable::new(r#"{"key":"val"}"#).is_json());
        assert!(!Stringable::new("not json").is_json());
    }

    #[test]
    fn test_is_url() {
        assert!(Stringable::new("https://example.com").is_url());
        assert!(!Stringable::new("not a url").is_url());
    }

    #[test]
    fn test_is_uuid() {
        assert!(Stringable::new("550e8400-e29b-41d4-a716-446655440000").is_uuid());
        assert!(!Stringable::new("not-a-uuid").is_uuid());
    }

    #[test]
    fn test_word_count() {
        assert_eq!(Stringable::new("Hello World").word_count(), 2);
        assert_eq!(Stringable::new("").word_count(), 0);
    }

    #[test]
    fn test_position() {
        assert_eq!(Stringable::new("Hello World").position("World", 0), Some(6));
        assert_eq!(Stringable::new("Hello World").position("xyz", 0), None);
    }

    #[test]
    fn test_append() {
        let result = Stringable::new("Hello").append(" World");
        assert_eq!(result.value(), "Hello World");
    }

    #[test]
    fn test_prepend() {
        let result = Stringable::new("World").prepend("Hello ");
        assert_eq!(result.value(), "Hello World");
    }

    #[test]
    fn test_display() {
        let s = Stringable::new("hello");
        assert_eq!(format!("{}", s), "hello");
    }

    #[test]
    fn test_into_string() {
        let s: String = Stringable::new("hello").into();
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_from_string() {
        let s: Stringable = "hello".into();
        assert_eq!(s.value(), "hello");
    }

    #[test]
    fn test_into_value() {
        assert_eq!(Stringable::new("hello").into_value(), "hello");
    }

    #[test]
    fn test_complex_chain() {
        let result = Stringable::new("the_quick_brown_fox")
            .replace("_", " ")
            .title()
            .replace(" ", "");
        assert_eq!(result.value(), "TheQuickBrownFox");
    }

    #[test]
    fn test_chain_slug_camel() {
        let result = Stringable::new("hello_world").camel().upper();
        assert_eq!(result.value(), "HELLOWORLD");
    }
}
