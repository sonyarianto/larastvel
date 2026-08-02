# Broadcasting

Larastvel supports real-time event broadcasting via WebSocket and third-party services.

## Drivers

| Driver | Description |
|--------|-------------|
| **Native** | Self-hosted WebSocket server |
| **Pusher** | Pusher Channels |
| **Ably** | Ably Realtime |
| **Log** | Log broadcaster for debugging |

## Native Broadcaster

```rust
use larastvel_core::axum::Extension;
use larastvel_core::broadcasting::{ws_handler, NativeBroadcaster, SubscriberRegistry};

// Create the registry and broadcaster (name + registry)
let registry = SubscriberRegistry::new();
let broadcaster = NativeBroadcaster::new("native", registry.clone());

// Register WebSocket route
router.ws("/ws", ws_handler);

// Attach the registry to the final router via the Application
app.with_layer(|router| router.layer(Extension(registry)));
```

## Broadcast Manager

```rust
use larastvel_core::broadcasting::log::LogBroadcaster;

let mut manager = BroadcastManager::new("native");
manager.register("native", NativeBroadcaster::new("native", registry));
manager.register("log", LogBroadcaster::new("log"));
```

## Broadcasting Events

The manager broadcasts events that implement the `BroadcastEvent` trait. Use the `#[broadcast_event]` macro — the struct must provide a `channels()` method. See the [full reference](/reference/broadcast-events) for details.

```rust
use larastvel_core::broadcast_event;
use larastvel_core::broadcasting::Channel;
use serde::Serialize;

#[broadcast_event("order.shipped")]
#[derive(Debug, Serialize)]
struct OrderShipped {
    order_id: String,
}

impl OrderShipped {
    fn channels(&self) -> Vec<Channel> {
        vec![Channel::public("orders")]
    }
}
```

Broadcast through the manager (the default broadcaster):

```rust
let event = OrderShipped { order_id: "123".to_string() };
manager.broadcast(&event).await?;
```

Or implement `BroadcastEvent` manually for full control:

```rust
use larastvel_core::async_trait;
use larastvel_core::broadcasting::{BroadcastEvent, Channel};

#[derive(Debug, serde::Serialize)]
struct OrderShipped {
    order_id: String,
}

#[async_trait]
impl BroadcastEvent for OrderShipped {
    fn broadcast_event_name(&self) -> &str {
        "order.shipped"
    }

    fn broadcast_channels(&self) -> Vec<Channel> {
        vec![Channel::public("orders")]
    }
}
```

A raw `BroadcastMessage` is the payload sent to clients — broadcast it via a `Broadcaster` directly:

```rust
use larastvel_core::broadcasting::BroadcastMessage;

let message = BroadcastMessage::new(
    "order.shipped",
    serde_json::json!({"order_id": "123"}),
    vec!["orders".to_string()],
);

let broadcaster = manager.default_broadcaster()?;
broadcaster.broadcast(message).await?;
```

## Channels

| Type | Description |
|------|-------------|
| `Channel::Public(name)` | Accessible to anyone |
| `Channel::Private(name)` | Requires authentication |
| `Channel::Presence { name, channel_data }` | Tracks connected users |

```rust
let channel = Channel::Private("orders.42".to_string());
```
