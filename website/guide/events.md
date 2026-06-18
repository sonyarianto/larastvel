# Events

Larastvel's event system lets you dispatch and listen for application events.

## Defining Events

Any `Clone + Send + Sync` type can be used as an event:

```rust
#[derive(Debug, Clone)]
struct OrderShipped {
    order_id: String,
}
```

## Defining Listeners

Implement the `Listener` trait:

```rust
use larastvel_core::events::{EventService, Listener, Event};
use async_trait::async_trait;

struct SendShipmentNotification;

#[async_trait]
impl Listener<OrderShipped> for SendShipmentNotification {
    async fn handle(&self, event: OrderShipped) {
        info!("Order {} shipped!", event.order_id);
    }
}
```

Or use a closure:

```rust
EventService::listen_fn::<OrderShipped, _, _>(move |event| async move {
    info!("Order {} shipped!", event.order_id);
});
```

## Registration

Register listeners during application bootstrap:

```rust
EventService::listen::<OrderShipped, SendShipmentNotification>(SendShipmentNotification);
```

## Dispatching

```rust
EventService::dispatch(OrderShipped {
    order_id: "ORD-123".into(),
}).await;
```

## Testing

Use `fake()` mode to capture dispatched events without running listeners:

```rust
EventService::fake();
EventService::dispatch(OrderShipped { order_id: "1".into() }).await;

assert!(EventService::assert_dispatched::<OrderShipped>());
assert_eq!(EventService::assert_dispatched_times::<OrderShipped>(1), true);

EventService::reset();
```

| Method | Description |
|--------|-------------|
| `listen::<E, L>(listener)` | Register a listener struct |
| `listen_fn::<E, F, Fut>(f)` | Register a closure listener |
| `dispatch::<E>(event)` | Dispatch an event |
| `fake()` | Enable fake mode |
| `assert_dispatched::<E>()` | Check if event was dispatched |
| `assert_not_dispatched::<E>()` | Check if event was not dispatched |
| `assert_dispatched_times::<E>(n)` | Check dispatch count |
| `has_listeners::<E>()` | Check if event has listeners |
| `clear_listeners::<E>()` | Remove listeners for an event |
| `clear_all_listeners()` | Remove all listeners |
| `reset()` | Reset everything |
