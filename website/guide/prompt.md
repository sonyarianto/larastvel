# CLI Prompts

Larastvel's `Prompt` provides interactive CLI prompt helpers for collecting user input in terminal applications.

## Text Input

```rust
use larastvel_core::Prompt;

let name = Prompt::ask("What is your name?", Some("John"))?;
// Displays: What is your name? [John]
```

## Confirmation

```rust
let result = Prompt::confirm("Delete this file?", Some(false))?;
// Displays: Delete this file? [y/N]
```

## Secret Input

```rust
let password = Prompt::secret("Enter password:")?;
// Displays: Enter password: [hidden input]
```

## Choice Selection

```rust
let color = Prompt::choice(
    "Pick a color",
    &["red", "green", "blue"],
    Some(0),
)?;
// Displays: Pick a color: [↑/↓] with arrow key navigation
```

## Autocomplete

```rust
let city = Prompt::autocomplete(
    "Search city",
    &["Paris", "London", "Tokyo"],
    None,
)?;
// Displays: Search city with fuzzy search
```

## Multiselect

```rust
let toppings = Prompt::multiselect(
    "Select toppings",
    &["Cheese", "Pepperoni", "Mushrooms"],
    Some(&[true, false, false]),
)?;
// Displays: Select toppings with [Space] to toggle
```

## Error Handling

All prompt methods return `Result<T, PromptError>`. Handle errors for non-TTY environments:

```rust
use larastvel_core::{Prompt, PromptError};

match Prompt::ask("Name?", None) {
    Ok(name) => println!("Hello, {name}!"),
    Err(PromptError::IO(e)) => eprintln!("IO error: {e}"),
}
```

## Available Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `ask(question, default)` | `Result<String>` | Text input prompt |
| `confirm(question, default)` | `Result<bool>` | Yes/no confirmation |
| `secret(question)` | `Result<String>` | Hidden input prompt |
| `choice(question, options, default)` | `Result<String>` | Select from list |
| `autocomplete(question, choices, default)` | `Result<String>` | Fuzzy search select |
| `multiselect(question, options, defaults)` | `Result<Vec<String>>` | Multiple selection |
