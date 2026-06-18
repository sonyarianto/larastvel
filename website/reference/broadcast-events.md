# Broadcast Events

The `#[broadcast_event]` attribute macro generates a `BroadcastEvent` trait implementation for broadcasting real-time messages over WebSockets.

## Usage

```rust
use larastvel_core::broadcasting::{BroadcastEvent, Channel};
use serde::Serialize;

#[broadcast_event("new-message")]
#[derive(Debug, Serialize)]
struct NewMessage {
    pub text: String,
}

impl NewMessage {
    fn channels(&self) -> Vec<Channel> {
        vec![Channel::public("chat")]
    }
}
```

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string literal | yes | Event name sent to clients (e.g. `"new-message"`) |

## Generated Implementation

The macro generates:

```rust
impl BroadcastEvent for NewMessage {
    fn broadcast_event_name(&self) -> &str {
        "new-message"
    }

    fn broadcast_channels(&self) -> Vec<Channel> {
        self.channels()
    }
}
```

The `broadcast_data()` and `broadcast_via()` methods use the trait's default implementations.

## Requirements

- Your struct must derive or implement `serde::Serialize` (required by the `BroadcastEvent` trait)
- Your struct must define a `channels` method returning `Vec<Channel>`

## User Method

Your struct must define a `channels` method (name chosen to avoid collision with `BroadcastEvent::broadcast_channels`):

```rust
fn channels(&self) -> Vec<Channel>
```

## Broadcast Channels

| Constructor | Channel Type | Example |
|-------------|--------------|---------|
| `Channel::public(name)` | Public | `Channel::public("chat")` |
| `Channel::private(name)` | Private (auth required) | `Channel::private("chat")` |
| `Channel::presence(name, user_id, user_info)` | Presence (with user list) | `Channel::presence("chat", "user-1", None)` |

## Usage

```rust
let mut manager = BroadcastManager::new("log");
manager.register("log", LogBroadcaster::new());

let event = NewMessage {
    text: "Hello!".to_string(),
};
manager.broadcast(&event).await?;
```

## CLI Generator

```bash
larastvel make:broadcast-event NewMessage
```
