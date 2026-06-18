# Mail

Larastvel provides SMTP and log mailers with a builder API for creating emails. The `#[mail]` macro generates clean mailable structs — see the [full reference](/reference/mail).

## Mailers

### SMTP

```rust
use larastvel_core::mail::{SmtpMailer, MailManager};

let smtp = SmtpMailer::new("smtp", "smtp.example.com", 587, "user", "pass")?;

let mut manager = MailManager::new("smtp");
manager.register("smtp", smtp);
```

### Log Mailer

```rust
use larastvel_core::mail::LogMailer;

manager.register("log", LogMailer::new("log"));
```

## Building Mail

```rust
use larastvel_core::mail::Mailable;

// Plain text
let email = Mailable::new(
    vec!["user@example.com".to_string()],
    "Welcome!",
    "Thank you for joining.",
);

// HTML
let email = Mailable::html(
    vec!["user@example.com".to_string()],
    "Welcome!",
    "<h1>Welcome</h1><p>Thanks for joining!</p>",
);

// Builder methods
let email = email
    .from("noreply@example.com")
    .reply_to("support@example.com")
    .cc(vec!["admin@example.com".to_string()])
    .bcc(vec!["audit@example.com".to_string()]);
```

## Sending

```rust
let mailer = manager.default_mailer()?;
mailer.send(email).await?;
```
