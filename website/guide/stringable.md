# Stringable (Fluent String)

Larastvel's `Stringable` provides a fluent, chainable API for string manipulation. Wrap a string with `Str::of()` and chain methods together.

## Basic Usage

```rust
use larastvel_core::Str;

Str::of("hello_world")
    .studly()       // "HelloWorld"
    .upper();       // "HELLOWORLD"
```

## Chaining

```rust
Str::of("  Hello World  ")
    .trim()
    .slug("-")
    .upper();       // "HELLO-WORLD"

Str::of("the_quick_brown_fox")
    .replace("_", " ")
    .title()
    .replace(" ", "");  // "TheQuickBrownFox"
```

## Case Conversion

```rust
Str::of("hello_world").slug("-");       // "hello-world"
Str::of("hello_world").camel();         // "helloWorld"
Str::of("hello_world").studly();        // "HelloWorld"
Str::of("HelloWorld").snake();          // "hello_world"
Str::of("HelloWorld").kebab();          // "hello-world"

Str::of("hello").upper();               // "HELLO"
Str::of("HELLO").lower();               // "hello"
Str::of("hello").ucfirst();             // "Hello"
Str::of("Hello").lcfirst();             // "hello"

Str::of("the quick brown fox").title(); // "The Quick Brown Fox"
Str::of("hello_world").headline();      // "Hello World"
```

## Substring Operations

```rust
Str::of("Hello World").after("Hello ");       // "World"
Str::of("Hello World").before(" World");      // "Hello"
Str::of("[Hello]").between("[", "]");         // "Hello"
Str::of("Hello World").substr(0, Some(5));    // "Hello"
Str::of("Hello World").limit(5, "...");       // "Hello..."
```

## Query Methods

```rust
let s = Str::of("Hello World");
s.contains("World");          // true
s.contains_all(&["Hello", "World"]); // true
s.starts_with("Hello");       // true
s.ends_with("World");         // true
s.length();                   // 11
s.is_ascii();                 // true
s.is_json();                  // false
s.is_url();                   // false
s.is_uuid();                  // false
s.word_count();               // 2
s.position("World", 0);       // Some(6)
```

## Padding & Masking

```rust
Str::of("Hello").pad_left(7, "*");     // "**Hello"
Str::of("Hello").pad_right(7, "*");    // "Hello**"
Str::of("Hello").pad_both(9, "*");     // "**Hello**"
Str::of("1234-5678").mask("*", 0, 4);  // "****-5678"
```

## Transformation

```rust
Str::of("Hello World").replace("World", "Moon");   // "Hello Moon"
Str::of("foo bar foo").replace_first("foo", "baz"); // "baz bar foo"
Str::of("ab").repeat(3);                             // "ababab"
Str::of("  hello  ").trim();                         // "hello"
Str::of("/hello/").trim_slashes();                   // "hello"
Str::of("hello").finish("/");                        // "hello/"
Str::of("world").start("hello ");                    // "hello world"
Str::of("Hello").append(" World");                   // "Hello World"
Str::of("World").prepend("Hello ");                  // "Hello World"
```

## Conversion

```rust
let s = Str::of("hello");
s.to_string();               // "hello" (Display trait)
let s: String = s.into();    // Into<String>
s.value();                   // &str
s.into_value();              // String (consuming)
```

## Available Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `slug(sep)` | Self | URL-friendly slug |
| `camel()` | Self | Convert to `camelCase` |
| `studly()` | Self | Convert to `StudlyCase` |
| `snake()` | Self | Convert to `snake_case` |
| `kebab()` | Self | Convert to `kebab-case` |
| `title()` | Self | Convert to Title Case |
| `headline()` | Self | Convert to Headline Case |
| `ucfirst()` | Self | Uppercase first character |
| `lcfirst()` | Self | Lowercase first character |
| `upper()` | Self | Convert to uppercase |
| `lower()` | Self | Convert to lowercase |
| `trim()` | Self | Trim whitespace |
| `trim_slashes()` | Self | Trim slashes |
| `replace(search, replace)` | Self | Replace all occurrences |
| `replace_first(search, replace)` | Self | Replace first occurrence |
| `repeat(times)` | Self | Repeat string |
| `after(search)` | Self | Everything after substring |
| `before(search)` | Self | Everything before substring |
| `between(from, to)` | Self | String between two substrings |
| `limit(len, end)` | Self | Truncate to length |
| `substr(start, len)` | Self | Substring by character positions |
| `pad_left(len, pad)` | Self | Left-pad |
| `pad_right(len, pad)` | Self | Right-pad |
| `pad_both(len, pad)` | Self | Pad both sides |
| `mask(char, start, len)` | Self | Mask portion |
| `finish(cap)` | Self | Ensure string ends with character |
| `start(prefix)` | Self | Ensure string starts with prefix |
| `append(suffix)` | Self | Append to string |
| `prepend(prefix)` | Self | Prepend to string |
| `contains(needle)` | bool | Check substring |
| `contains_all(needles)` | bool | Check all substrings |
| `starts_with(needle)` | bool | Check prefix |
| `ends_with(needle)` | bool | Check suffix |
| `position(needle, offset)` | `Option<usize>` | Find position |
| `length()` | `usize` | Character count |
| `is_ascii()` | bool | Check if all ASCII |
| `is_json()` | bool | Check if valid JSON |
| `is_url()` | bool | Check if http(s):// |
| `is_uuid()` | bool | Check if valid UUID |
| `word_count()` | `usize` | Count words |
