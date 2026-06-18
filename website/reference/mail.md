# Mail

The `#[mail]` attribute macro generates a `send()` method for mailing structs, wrapping the `Mailable` builder pattern.

## Usage

```rust
use larastvel_core::mail::Mailer;

#[mail(subject = "Welcome!", from = "noreply@example.com")]
#[derive(Debug)]
struct WelcomeMail {
    /// Recipient email addresses.
    pub to: Vec<String>,
    pub name: String,
}

impl WelcomeMail {
    /// Build the HTML body for this mail.
    pub fn html(&self) -> String {
        format!("<h1>Welcome, {}</h1>", self.name)
    }
}
```

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `subject` | string | yes | Email subject line |
| `from` | string | no | Sender address |
| `reply_to` | string | no | Reply-to address |

## Generated Implementation

The macro generates a `send()` method:

```rust
impl WelcomeMail {
    pub async fn send(
        &self,
        mailer: &dyn Mailer,
    ) -> Result<(), MailError> {
        let mailable = Mailable::html(
            self.to.clone(),
            "Welcome!",
            &self.html(),
        )
        .from("noreply@example.com");
        mailer.send(mailable).await
    }
}
```

## Requirements

- A `to` field of type `Vec<String>` for recipients
- An `html(&self) -> String` method returning the HTML body

## Usage

```rust
let mail = WelcomeMail {
    to: vec!["user@example.com".to_string()],
    name: "Alice".to_string(),
};

let mailer = LogMailer::new("log");
mail.send(&mailer).await?;
```

## CLI Generator

```bash
larastvel make:mail WelcomeMail
```
