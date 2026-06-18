# Str (String Helpers)

Larastvel's `Str` provides a fluent set of static string manipulation methods inspired by Laravel's `Str` class.

## Case Conversion

```rust
use larastvel_core::Str;

Str::slug("Hello World", "-");       // "hello-world"
Str::slug("Hello World", "_");       // "hello_world"

Str::camel("hello_world");           // "helloWorld"
Str::studly("hello_world");          // "HelloWorld"
Str::snake("HelloWorld");            // "hello_world"
Str::snake("XMLParser");             // "xml_parser"
Str::kebab("hello_world");           // "hello-world"
Str::title("the quick brown fox");   // "The Quick Brown Fox"
Str::headline("hello_world");        // "Hello World"

Str::ucfirst("hello");               // "Hello"
Str::lcfirst("Hello");               // "hello"
```

## Substring Operations

```rust
Str::contains("Hello World", "World");              // true
Str::contains_all("Hello World", &["Hello", "World"]); // true
Str::starts_with("Hello World", "Hello");            // true
Str::ends_with("Hello World", "World");              // true

Str::after("Hello World", "Hello ");                 // "World"
Str::before("Hello World", " World");                // "Hello"
Str::between("{Hello}", "{", "}");                   // "Hello"
Str::substr("Hello World", 0, Some(5));              // "Hello"
Str::substr("Hello World", 6, None);                 // "World"
Str::position("Hello World", "World", 0);            // Some(6)
Str::limit("Hello World", 5, "...");                 // "Hello..."
```

## String Analysis

```rust
Str::length("Hello");               // 5
Str::length("日本語");              // 3

Str::is_ascii("Hello");             // true
Str::is_ascii("日本語");            // false

Str::word_count("Hello World");     // 2
Str::is_json(r#"{"key":"val"}"#);   // true
Str::is_url("https://example.com"); // true
Str::is_uuid("550e8400-e29b-41d4-a716-446655440000"); // true
```

## Transformation

```rust
Str::pad_left("Hello", 7, "*");     // "**Hello"
Str::pad_right("Hello", 7, "*");    // "Hello**"
Str::pad_both("Hello", 9, "*");     // "**Hello**"

Str::replace("Hello World", "World", "Moon");  // "Hello Moon"
Str::replace_first("foo bar foo", "foo", "baz"); // "baz bar foo"

Str::mask("1234-5678", "*", 0, 4);  // "****-5678"

Str::repeat("ab", 3);               // "ababab"
Str::trim("  hello  ");             // "hello"
Str::trim_slashes("/hello/");       // "hello"
Str::finish("hello", "/");          // "hello/"
Str::start("world", "hello ");      // "hello world"
```

## Random Strings

```rust
Str::random(10);                     // e.g. "aB3xK9mQ2p"
Str::random_numeric(5);              // e.g. "48391"
```

## Available Methods

| Method | Description |
|--------|-------------|
| `slug(s, sep)` | URL-friendly slug with custom separator |
| `camel(s)` | Convert to `camelCase` |
| `studly(s)` | Convert to `StudlyCase` (PascalCase) |
| `snake(s)` | Convert to `snake_case` |
| `kebab(s)` | Convert to `kebab-case` |
| `title(s)` | Convert to Title Case (respects small words) |
| `headline(s)` | Convert to Headline Case |
| `ucfirst(s)` | Uppercase first character |
| `lcfirst(s)` | Lowercase first character |
| `contains(s, needle)` | Check if string contains substring |
| `contains_all(s, needles)` | Check if string contains all substrings |
| `starts_with(s, needle)` | Check prefix |
| `ends_with(s, needle)` | Check suffix |
| `after(s, search)` | Everything after first occurrence |
| `before(s, search)` | Everything before first occurrence |
| `between(s, from, to)` | String between two substrings |
| `substr(s, start, length)` | Substring by character positions |
| `position(s, needle, offset)` | Find position of substring |
| `length(s)` | Character count (not byte length) |
| `is_ascii(s)` | Check if all characters are ASCII |
| `word_count(s)` | Count whitespace-separated words |
| `is_json(s)` | Check if string is valid JSON |
| `is_url(s)` | Check if string starts with http(s):// |
| `is_uuid(s)` | Check if string is valid UUID format |
| `pad_left(s, len, pad)` | Left-pad to specified length |
| `pad_right(s, len, pad)` | Right-pad to specified length |
| `pad_both(s, len, pad)` | Pad both sides to specified length |
| `replace(s, search, replace)` | Replace all occurrences |
| `replace_first(s, search, replace)` | Replace first occurrence |
| `mask(s, char, start, len)` | Mask portion of string |
| `repeat(s, times)` | Repeat string N times |
| `trim(s)` | Trim whitespace |
| `trim_slashes(s)` | Trim leading/trailing slashes |
| `finish(s, cap)` | Ensure string ends with given character |
| `start(s, prefix)` | Ensure string starts with given prefix |
| `random(len)` | Generate random alphanumeric string |
| `random_numeric(len)` | Generate random numeric string |
