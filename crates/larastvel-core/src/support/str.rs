use rand::Rng;

pub struct Str;

impl Str {
    pub fn slug(s: &str, separator: &str) -> String {
        let s = s.to_lowercase();
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
        result
    }

    pub fn studly(s: &str) -> String {
        words(s).into_iter().map(|w| capitalize(&w)).collect()
    }

    pub fn camel(s: &str) -> String {
        let mut result = Self::studly(s);
        if let Some(c) = result.chars().next() {
            result.replace_range(..c.len_utf8(), &c.to_lowercase().to_string());
        }
        result
    }

    pub fn snake(s: &str) -> String {
        words(s).join("_")
    }

    pub fn kebab(s: &str) -> String {
        words(s).join("-")
    }

    pub fn title(s: &str) -> String {
        let small_words = [
            "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "by",
            "with", "nor", "as", "is", "it",
        ];
        let words: Vec<String> = words(s);
        let mut result = Vec::with_capacity(words.len());
        for (i, w) in words.iter().enumerate() {
            let lower = w.to_lowercase();
            if i > 0 && i < words.len() - 1 && small_words.contains(&lower.as_str()) {
                result.push(lower);
            } else {
                result.push(capitalize(w));
            }
        }
        result.join(" ")
    }

    pub fn headline(s: &str) -> String {
        let s = s.replace('_', " ");
        let s = s.replace('-', " ");
        let words: Vec<&str> = s.split_whitespace().collect();
        words
            .into_iter()
            .map(|w| {
                let w = w.to_lowercase();
                capitalize(&w)
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn contains(s: &str, needle: &str) -> bool {
        s.contains(needle)
    }

    pub fn contains_all(s: &str, needles: &[&str]) -> bool {
        needles.iter().all(|n| s.contains(*n))
    }

    pub fn starts_with(s: &str, needle: &str) -> bool {
        s.starts_with(needle)
    }

    pub fn ends_with(s: &str, needle: &str) -> bool {
        s.ends_with(needle)
    }

    pub fn after<'a>(s: &'a str, search: &str) -> &'a str {
        s.split_once(search).map(|(_, after)| after).unwrap_or("")
    }

    pub fn before<'a>(s: &'a str, search: &str) -> &'a str {
        s.split_once(search).map(|(before, _)| before).unwrap_or(s)
    }

    pub fn between<'a>(s: &'a str, from: &str, to: &str) -> &'a str {
        let s = Self::after(s, from);
        Self::before(s, to)
    }

    pub fn limit(s: &str, limit: usize, end: &str) -> String {
        if s.len() <= limit {
            s.to_string()
        } else {
            let mut result = s[..limit].to_string();
            result.push_str(end);
            result
        }
    }

    pub fn substr(s: &str, start: usize, length: Option<usize>) -> String {
        s.chars()
            .skip(start)
            .take(length.unwrap_or(usize::MAX))
            .collect()
    }

    pub fn position(s: &str, needle: &str, offset: usize) -> Option<usize> {
        if offset >= s.len() {
            return None;
        }
        let search = &s[offset..];
        search.find(needle).map(|pos| pos + offset)
    }

    pub fn length(s: &str) -> usize {
        s.chars().count()
    }

    pub fn is_ascii(s: &str) -> bool {
        s.is_ascii()
    }

    pub fn word_count(s: &str) -> usize {
        s.split_whitespace().filter(|w| !w.is_empty()).count()
    }

    pub fn is_json(s: &str) -> bool {
        serde_json::from_str::<serde_json::Value>(s).is_ok()
    }

    pub fn pad_left(s: &str, length: usize, pad: &str) -> String {
        let current = s.chars().count();
        if current >= length {
            return s.to_string();
        }
        let needed = length - current;
        let mut result = String::with_capacity(length);
        let pad_chars: Vec<char> = pad.chars().collect();
        for i in 0..needed {
            result.push(pad_chars[i % pad_chars.len()]);
        }
        result.push_str(s);
        result
    }

    pub fn pad_right(s: &str, length: usize, pad: &str) -> String {
        let current = s.chars().count();
        if current >= length {
            return s.to_string();
        }
        let needed = length - current;
        let mut result = s.to_string();
        let pad_chars: Vec<char> = pad.chars().collect();
        for i in 0..needed {
            result.push(pad_chars[i % pad_chars.len()]);
        }
        result
    }

    pub fn pad_both(s: &str, length: usize, pad: &str) -> String {
        let current = s.chars().count();
        if current >= length {
            return s.to_string();
        }
        let needed = length - current;
        let left = needed / 2;
        let right = needed - left;
        let mut result = String::with_capacity(length);
        let pad_chars: Vec<char> = pad.chars().collect();
        for i in 0..left {
            result.push(pad_chars[i % pad_chars.len()]);
        }
        result.push_str(s);
        for i in 0..right {
            result.push(pad_chars[(left + i) % pad_chars.len()]);
        }
        result
    }

    pub fn replace(s: &str, search: &str, replace: &str) -> String {
        s.replace(search, replace)
    }

    pub fn replace_first(s: &str, search: &str, replace: &str) -> String {
        s.replacen(search, replace, 1)
    }

    pub fn random(length: usize) -> String {
        let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::rngs::OsRng;
        (0..length)
            .map(|_| {
                let idx = rng.gen_range(0..chars.len());
                chars[idx] as char
            })
            .collect()
    }

    pub fn random_numeric(length: usize) -> String {
        let mut rng = rand::rngs::OsRng;
        (0..length)
            .map(|_| rng.gen_range(0..10).to_string())
            .collect()
    }

    pub fn mask(s: &str, character: &str, start: usize, length: usize) -> String {
        let chars: Vec<char> = s.chars().collect();
        let mut result = String::with_capacity(s.len());
        let mask_char = character.chars().next().unwrap_or('*');
        for (i, &c) in chars.iter().enumerate() {
            if i >= start && i < start + length {
                result.push(mask_char);
            } else {
                result.push(c);
            }
        }
        result
    }

    pub fn ucfirst(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        }
    }

    pub fn lcfirst(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_lowercase().to_string() + chars.as_str(),
        }
    }

    pub fn repeat(s: &str, times: usize) -> String {
        s.repeat(times)
    }

    pub fn trim(s: &str) -> String {
        s.trim().to_string()
    }

    pub fn trim_slashes(s: &str) -> String {
        s.trim_matches('/').to_string()
    }

    pub fn finish(s: &str, cap: &str) -> String {
        if s.ends_with(cap) {
            s.to_string()
        } else {
            format!("{}{}", s, cap)
        }
    }

    pub fn start(s: &str, prefix: &str) -> String {
        if s.starts_with(prefix) {
            s.to_string()
        } else {
            format!("{}{}", prefix, s)
        }
    }

    pub fn is_url(s: &str) -> bool {
        s.starts_with("http://") || s.starts_with("https://")
    }

    pub fn is_uuid(s: &str) -> bool {
        let s = s.trim();
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
}

fn words(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.is_empty() {
        return vec![];
    }

    let unified: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();

    let tokens: Vec<&str> = unified.split_whitespace().collect();
    let mut result = Vec::new();
    for token in tokens {
        for word in split_camel(token) {
            if !word.is_empty() {
                result.push(word.to_lowercase());
            }
        }
    }
    result
}

fn split_camel(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() && !current.is_empty() {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let prev_is_lower = prev.is_ascii_lowercase();
            let next_is_lower = next.is_some_and(|n| n.is_ascii_lowercase());

            if prev_is_lower || (next_is_lower && current.chars().all(|pc| pc.is_ascii_uppercase()))
            {
                result.push(current.clone());
                current.clear();
            }
        }
        current.push(c);
    }

    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_basic() {
        assert_eq!(Str::slug("Hello World", "-"), "hello-world");
    }

    #[test]
    fn test_slug_multi_spaces() {
        assert_eq!(Str::slug("Hello   World", "-"), "hello-world");
    }

    #[test]
    fn test_slug_special_chars() {
        assert_eq!(Str::slug("Hello! World?", "-"), "hello-world");
    }

    #[test]
    fn test_slug_trailing_separator() {
        assert_eq!(Str::slug("Hello World! ", "-"), "hello-world");
    }

    #[test]
    fn test_slug_custom_separator() {
        assert_eq!(Str::slug("Hello World", "_"), "hello_world");
    }

    #[test]
    fn test_slug_empty() {
        assert_eq!(Str::slug("", "-"), "");
    }

    #[test]
    fn test_studly() {
        assert_eq!(Str::studly("hello_world"), "HelloWorld");
        assert_eq!(Str::studly("hello-world"), "HelloWorld");
        assert_eq!(Str::studly("hello world"), "HelloWorld");
    }

    #[test]
    fn test_studly_camel_case_input() {
        assert_eq!(Str::studly("helloWorld"), "HelloWorld");
    }

    #[test]
    fn test_camel() {
        assert_eq!(Str::camel("hello_world"), "helloWorld");
        assert_eq!(Str::camel("HelloWorld"), "helloWorld");
        assert_eq!(Str::camel("hello-world"), "helloWorld");
    }

    #[test]
    fn test_snake() {
        assert_eq!(Str::snake("HelloWorld"), "hello_world");
        assert_eq!(Str::snake("helloWorld"), "hello_world");
        assert_eq!(Str::snake("hello-world"), "hello_world");
    }

    #[test]
    fn test_kebab() {
        assert_eq!(Str::kebab("HelloWorld"), "hello-world");
        assert_eq!(Str::kebab("hello_world"), "hello-world");
        assert_eq!(Str::kebab("hello world"), "hello-world");
    }

    #[test]
    fn test_title() {
        assert_eq!(Str::title("hello world"), "Hello World");
        assert_eq!(Str::title("the quick brown fox"), "The Quick Brown Fox");
    }

    #[test]
    fn test_title_with_small_words() {
        assert_eq!(Str::title("a tale of two cities"), "A Tale of Two Cities");
    }

    #[test]
    fn test_headline() {
        assert_eq!(Str::headline("hello_world"), "Hello World");
        assert_eq!(Str::headline("hello-world"), "Hello World");
        assert_eq!(Str::headline("hello world"), "Hello World");
    }

    #[test]
    fn test_contains() {
        assert!(Str::contains("Hello World", "World"));
        assert!(!Str::contains("Hello World", "world"));
    }

    #[test]
    fn test_contains_all() {
        assert!(Str::contains_all("Hello World", &["Hello", "World"]));
        assert!(!Str::contains_all("Hello World", &["Hello", "world"]));
    }

    #[test]
    fn test_starts_with() {
        assert!(Str::starts_with("Hello World", "Hello"));
        assert!(!Str::starts_with("Hello World", "World"));
    }

    #[test]
    fn test_ends_with() {
        assert!(Str::ends_with("Hello World", "World"));
        assert!(!Str::ends_with("Hello World", "Hello"));
    }

    #[test]
    fn test_after() {
        assert_eq!(Str::after("Hello World", "Hello "), "World");
        assert_eq!(Str::after("Hello World", "xyz"), "");
    }

    #[test]
    fn test_before() {
        assert_eq!(Str::before("Hello World", " World"), "Hello");
        assert_eq!(Str::before("Hello World", "xyz"), "Hello World");
    }

    #[test]
    fn test_between() {
        assert_eq!(Str::between("[Hello]", "[", "]"), "Hello");
        assert_eq!(Str::between("Hello {World}", "{", "}"), "World");
    }

    #[test]
    fn test_limit() {
        assert_eq!(Str::limit("Hello World", 5, "..."), "Hello...");
        assert_eq!(Str::limit("Hi", 5, "..."), "Hi");
    }

    #[test]
    fn test_substr() {
        assert_eq!(Str::substr("Hello World", 0, Some(5)), "Hello");
        assert_eq!(Str::substr("Hello World", 6, None), "World");
    }

    #[test]
    fn test_position() {
        assert_eq!(Str::position("Hello World", "World", 0), Some(6));
        assert_eq!(Str::position("Hello World", "xyz", 0), None);
    }

    #[test]
    fn test_position_with_offset() {
        assert_eq!(Str::position("Hello World World", "World", 8), Some(12));
    }

    #[test]
    fn test_length() {
        assert_eq!(Str::length("Hello"), 5);
        assert_eq!(Str::length(""), 0);
        assert_eq!(Str::length("日本語"), 3);
    }

    #[test]
    fn test_is_ascii() {
        assert!(Str::is_ascii("Hello"));
        assert!(!Str::is_ascii("日本語"));
    }

    #[test]
    fn test_word_count() {
        assert_eq!(Str::word_count("Hello World"), 2);
        assert_eq!(Str::word_count(""), 0);
        assert_eq!(Str::word_count("one"), 1);
    }

    #[test]
    fn test_is_json() {
        assert!(Str::is_json(r#"{"key": "value"}"#));
        assert!(Str::is_json("[1, 2, 3]"));
        assert!(!Str::is_json("not json"));
    }

    #[test]
    fn test_pad_left() {
        assert_eq!(Str::pad_left("Hello", 7, "*"), "**Hello");
        assert_eq!(Str::pad_left("Hello", 3, "*"), "Hello");
    }

    #[test]
    fn test_pad_right() {
        assert_eq!(Str::pad_right("Hello", 7, "*"), "Hello**");
    }

    #[test]
    fn test_pad_both() {
        assert_eq!(Str::pad_both("Hello", 9, "*"), "**Hello**");
    }

    #[test]
    fn test_replace() {
        assert_eq!(Str::replace("Hello World", "World", "Moon"), "Hello Moon");
    }

    #[test]
    fn test_replace_first() {
        assert_eq!(
            Str::replace_first("foo bar foo", "foo", "baz"),
            "baz bar foo"
        );
    }

    #[test]
    fn test_random_length() {
        let s = Str::random(10);
        assert_eq!(s.len(), 10);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_random_numeric() {
        let s = Str::random_numeric(5);
        assert_eq!(s.len(), 5);
        assert!(s.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_random_unique() {
        let a = Str::random(20);
        let b = Str::random(20);
        assert_ne!(a, b);
    }

    #[test]
    fn test_mask() {
        assert_eq!(Str::mask("1234-5678", "*", 0, 4), "****-5678");
        assert_eq!(Str::mask("Hello World", "*", 6, 5), "Hello *****");
    }

    #[test]
    fn test_ucfirst() {
        assert_eq!(Str::ucfirst("hello"), "Hello");
        assert_eq!(Str::ucfirst("Hello"), "Hello");
        assert_eq!(Str::ucfirst(""), "");
    }

    #[test]
    fn test_lcfirst() {
        assert_eq!(Str::lcfirst("Hello"), "hello");
        assert_eq!(Str::lcfirst("hello"), "hello");
        assert_eq!(Str::lcfirst(""), "");
    }

    #[test]
    fn test_repeat() {
        assert_eq!(Str::repeat("ab", 3), "ababab");
        assert_eq!(Str::repeat("", 5), "");
    }

    #[test]
    fn test_trim() {
        assert_eq!(Str::trim("  hello  "), "hello");
        assert_eq!(Str::trim("hello"), "hello");
    }

    #[test]
    fn test_trim_slashes() {
        assert_eq!(Str::trim_slashes("/hello/"), "hello");
        assert_eq!(Str::trim_slashes("hello"), "hello");
    }

    #[test]
    fn test_finish() {
        assert_eq!(Str::finish("hello", "/"), "hello/");
        assert_eq!(Str::finish("hello/", "/"), "hello/");
    }

    #[test]
    fn test_start() {
        assert_eq!(Str::start("world", "hello "), "hello world");
        assert_eq!(Str::start("hello world", "hello "), "hello world");
    }

    #[test]
    fn test_is_url() {
        assert!(Str::is_url("https://example.com"));
        assert!(Str::is_url("http://example.com"));
        assert!(!Str::is_url("ftp://example.com"));
        assert!(!Str::is_url("not a url"));
    }

    #[test]
    fn test_is_uuid() {
        assert!(Str::is_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!Str::is_uuid("not-a-uuid"));
        assert!(!Str::is_uuid("550e8400-e29b-41d4-a716-44665544000")); // too short
    }

    #[test]
    fn test_studly_edge_cases() {
        assert_eq!(Str::studly(""), "");
        assert_eq!(Str::studly("a"), "A");
    }

    #[test]
    fn test_camel_edge_cases() {
        assert_eq!(Str::camel(""), "");
        assert_eq!(Str::camel("a"), "a");
    }

    #[test]
    fn test_snake_acronyms() {
        assert_eq!(Str::snake("XMLParser"), "xml_parser");
        assert_eq!(Str::snake("PDFExport"), "pdf_export");
    }

    #[test]
    fn test_kebab_snake_mix() {
        assert_eq!(Str::kebab("snake_case_text"), "snake-case-text");
    }

    #[test]
    fn test_between_not_found() {
        assert_eq!(Str::between("Hello", "[", "]"), "");
        assert_eq!(Str::between("Hello [World", "[", "]"), "World");
    }

    #[test]
    fn test_limit_exact_match() {
        assert_eq!(Str::limit("Hello", 5, "..."), "Hello");
    }

    #[test]
    fn test_limit_shorter_than_limit() {
        assert_eq!(Str::limit("Hi", 10, "..."), "Hi");
    }

    #[test]
    fn test_substr_out_of_bounds() {
        assert_eq!(Str::substr("Hello", 10, None), "");
    }

    #[test]
    fn test_position_offset_beyond_length() {
        assert_eq!(Str::position("Hello", "H", 100), None);
    }

    #[test]
    fn test_word_count_multi_space() {
        assert_eq!(Str::word_count("Hello    World"), 2);
    }

    #[test]
    fn test_mask_full_string() {
        assert_eq!(Str::mask("secret", "*", 0, 6), "******");
    }

    #[test]
    fn test_mask_empty() {
        assert_eq!(Str::mask("", "*", 0, 5), "");
    }
}
