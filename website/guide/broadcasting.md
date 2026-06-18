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
use larastvel_core::broadcasting::{
    NativeBroadcaster, SubscriberRegistry, BroadcastManager, Channel,
};

// Create the registry and broadcaster
let registry = SubscriberRegistry::new();
let broadcaster = NativeBroadcaster::new(registry.clone());

// Register WebSocket route
router.ws("/ws", ws_handler);

// Attach the registry to the app
router.layer(Extension(registry));
```

## Broadcast Manager

```rust
let mut manager = BroadcastManager::new("native");
manager.register("native", NativeBroadcaster::new(registry));
manager.register("log", LogBroadcaster::new("log"));
```

## Broadcasting Events

Define broadcast events with the `#[broadcast_event]` macro. See the [full reference](/reference/broadcast-events) for details.

```rust
use larastvel_core::broadcasting::{BroadcastEvent, Channel};
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

Or broadcast raw messages without a struct:

```rust
use larastvel_core::broadcasting::{BroadcastMessage, Channel};

let message = BroadcastMessage::new(
    "order.shipped",
    json!({"order_id": "123"}),
    vec!["orders".to_string()],
);

manager.broadcast(message).await?;
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
