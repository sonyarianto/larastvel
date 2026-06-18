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

## Defining Listeners with `#[listener]`

Use the `#[listener]` attribute macro to turn an async function into a synchronous listener:

```rust
use larastvel_core::listener;

#[listener(OrderShipped)]
async fn send_shipment_notification(event: OrderShipped) {
    tracing::info!("Order {} shipped!", event.order_id);
}
```

This generates a zero-sized struct `SendShipmentNotificationListener` that implements the `Listener<OrderShipped>` trait. The generated struct provides a `listen()` method for registration.

## Queued Listeners with `#[queued_listener]`

For listeners that should run in the background, use `#[queued_listener]`:

```rust
use larastvel_core::queued_listener;

#[queued_listener(OrderShipped)]
async fn send_shipment_notification(event: OrderShipped) {
    tracing::info!("Order {} shipped!", event.order_id);
}
```

This generates both a job struct and a listener struct. When the event is dispatched, the listener pushes a background job instead of running the handler inline.

## Defining Listeners Manually

Alternatively, implement the `Listener` trait directly:

```rust
use larastvel_core::events::{EventService, Listener, Event};
use async_trait::async_trait;

struct SendShipmentNotification;

#[async_trait]
impl Listener<OrderShipped> for SendShipmentNotification {
    async fn handle(&self, event: OrderShipped) {
        tracing::info!("Order {} shipped!", event.order_id);
    }
}
```

Or use a closure:

```rust
EventService::listen_fn::<OrderShipped, _, _>(move |event| async move {
    tracing::info!("Order {} shipped!", event.order_id);
});
```

## Registration

For macro-generated listeners, call the generated `listen()` method:

```rust
SendShipmentNotificationListener::listen();
```

For manually-defined listeners:

```rust
EventService::listen::<OrderShipped, SendShipmentNotification>(SendShipmentNotification);
```

## Dispatching

```rust
EventService::dispatch(OrderShipped {
    order_id: "ORD-123".into(),
}).await;
```

## CLI Generators

```bash
# Generate an event + listener pair
larastvel make:event OrderShipped

# Generate a standalone listener
larastvel make:listener SendNotification
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

## API Reference

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
