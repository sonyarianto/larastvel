# SMS

Larastvel supports sending SMS messages through log or Vonage drivers.

## Drivers

```rust
use larastvel_core::sms::{LogSmsSender, VonageSmsSender, SmsMessage, SmsSender};

// Log sender (development)
let sender = LogSmsSender::new();

// Vonage sender (production)
let sender = VonageSmsSender::new("api_key", "api_secret");
```

## Sending Messages

```rust
let message = SmsMessage::new(
    "+15551234567",          // to
    "Your verification code is 123456",
).from("Larastvel");

sender.send(&message).await?;
```

## Notifications Integration

SMS can be used as a notification channel:

```rust
impl Notification for OrderShipped {
    fn via(&self) -> Vec<NotificationChannel> {
        vec![NotificationChannel::Sms]
    }

    fn to_sms(&self) -> Option<SmsMessage> {
        Some(SmsMessage::new("", "Your order has shipped!").from("Orders"))
    }
}
```
