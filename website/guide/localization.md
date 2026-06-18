# Localization

Larastvel provides JSON-based translation with Laravel-style `__()` and `trans_choice()`.

## Configuration

```rust
use larastvel_core::translation;

// Set locale
translation::set_locale("id");

// Load translations
translation::load_translation_directory("resources/lang")?;
```

## Translation Files

Create JSON translation files in `resources/lang/`:

```json
// resources/lang/id.json
{
    "welcome": "Selamat datang",
    "apples": "{0} Tidak ada apel|[1] Satu apel|[2,*] :count apel"
}
```

## Usage

```rust
use larastvel_core::translation::__;

// Simple string
__("welcome");  // "Selamat datang"

// With replacements
__("Hello :name", vec![("name", "John")]);

// Pluralization
trans_choice("apples", 1);  // "Satu apel"
trans_choice("apples", 5);  // "5 apel"
```

## Available Functions

| Function | Description |
|----------|-------------|
| `__("key")` | Get translation |
| `__with("key", params)` | Get translation with params |
| `trans_choice("key", count)` | Pluralized translation |
| `locale()` | Get current locale |
| `set_locale("id")` | Set locale |
| `set_fallback_locale("en")` | Set fallback locale |
