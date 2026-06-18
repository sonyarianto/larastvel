# Notifications

Larastvel's notification system supports multiple delivery channels.

## Channels

| Channel | Description |
|---------|-------------|
| **Mail** | Send via configured mailer |
| **Database** | Store in `notifications` table |
| **Broadcast** | Real-time WebSocket events |
| **SMS** | Text messages via Vonage or log |
| **Webhook** | HTTP POST to a URL |

## Defining Notifications

Use the `#[notification]` attribute macro on your struct's `impl` block:

```rust
use larastvel_core::notifications::{NotificationChannel, NotificationSender};
use larastvel_core::mail::Mailable;
use larastvel_core::notification;

#[derive(Debug)]
struct OrderShipped {
    order_id: String,
}

#[notification]
impl OrderShipped {
    fn via(&self) -> Vec<NotificationChannel> {
        vec![NotificationChannel::Mail, NotificationChannel::Broadcast]
    }

    fn to_mail(&self) -> Option<Mailable> {
        Some(Mailable::html(
            vec![],
            &format!("Order #{} Shipped!", self.order_id),
            &format!("<h1>Order {} has shipped!</h1>", self.order_id),
        ).from("orders@example.com"))
    }

    fn to_broadcast(&self) -> Option<BroadcastPayload> {
        Some(BroadcastPayload {
            event: "order.shipped".to_string(),
            data: json!({"order_id": self.order_id}),
        })
    }
}
```

The macro scans for `via`, `to_mail`, `to_broadcast`, `to_database`, `to_webhook`, and `to_sms` methods, then generates `impl Notification for OrderShipped` containing those methods. Only `via` is required — every `to_*` method has a default that returns `None`.

You can define other helper methods alongside notification methods; they remain on the original `impl` block.

## Notifiable Trait

Implement `Notifiable` on your user/model:

```rust
use larastvel_core::notifications::Notifiable;

impl Notifiable for User {
    fn notification_id(&self) -> String {
        self.id.to_string()
    }

    fn route_email(&self) -> Option<String> {
        Some(self.email.clone())
    }

    fn route_phone(&self) -> Option<String> {
        self.phone.clone()
    }
}
```

## Sending Notifications

```rust
let sender = NotificationSender::new()
    .with_mailer(Arc::new(LogMailer::new("log")))
    .with_broadcaster(Arc::new(broadcaster))
    .with_database(db)
    .with_from("noreply@example.com");

let results = sender.send(&user, OrderShipped {
    order_id: "ORD-123".into(),
}).await;

// Check individual channel results
let mail_result = results.get(&NotificationChannel::Mail);
```
