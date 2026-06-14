//! # EventServiceProvider
//!
//! Registers event listeners with the `EventService` during the register phase.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use larastvel_core::foundation::{Application, EventServiceProvider};
//! use larastvel_core::events::{Event, Listener};
//!
//! #[derive(Debug, Clone)]
//! struct OrderShipped { order_id: String }
//!
//! struct ShipNotification;
//!
//! #[async_trait::async_trait]
//! impl Listener<OrderShipped> for ShipNotification {
//!     async fn handle(&self, event: OrderShipped) {
//!         println!("Order {} shipped!", event.order_id);
//!     }
//! }
//!
//! let app = Application::new(None);
//! app.register_provider(std::sync::Arc::new(
//!     EventServiceProvider::new()
//!         .listen::<OrderShipped, ShipNotification>(ShipNotification)
//! ));
//! ```

use std::future::Future;

use crate::events::{Event, EventService, Listener};

use crate::foundation::{Application, ServiceProvider};

/// Registers event listeners with the `EventService`.
///
/// Listeners are registered immediately when `listen()` or `listen_fn()`
/// is called (during the provider's construction), not deferred to `boot()`.
/// This is because `EventService` uses global state, so there's no
/// container dependency to wait for.
///
/// This is the Laravel-inspired way to organise event-to-listener mappings.
pub struct EventServiceProvider;

impl EventServiceProvider {
    /// Create a new `EventServiceProvider`.
    pub fn new() -> Self {
        Self
    }

    /// Register a structured listener for an event.
    pub fn listen<E: Event, L: Listener<E> + 'static>(self, listener: L) -> Self {
        EventService::listen::<E, L>(listener);
        self
    }

    /// Register a closure listener for an event.
    pub fn listen_fn<E: Event, F, Fut>(self, f: F) -> Self
    where
        F: Fn(E) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        EventService::listen_fn::<E, F, Fut>(f);
        self
    }
}

impl Default for EventServiceProvider {
    fn default() -> Self {
        Self
    }
}

impl ServiceProvider for EventServiceProvider {
    fn register(&self, _app: &Application) {
        // Listeners were registered during construction via listen()/listen_fn().
        // No additional work needed here.
    }

    fn boot(&self, _app: &Application) {
        // Listeners are already registered — nothing to do.
    }

    fn provides(&self) -> Vec<&'static str> {
        vec!["events"]
    }
}

/// Convenience function to register event listeners inline.
///
/// ```rust,ignore
/// use larastvel_core::foundation::register_event_listeners;
///
/// register_event_listeners(&app, |p| {
///     p.listen::<MyEvent, MyListener>(MyListener)
/// });
/// ```
pub fn register_event_listeners(
    _app: &Application,
    f: impl FnOnce(EventServiceProvider) -> EventServiceProvider,
) {
    let _provider = f(EventServiceProvider);
    // Providers register listeners during construction, so calling the
    // builder function is sufficient. The provider itself is not stored
    // in the application (use app.register_provider() for that).
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::Application;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq)]
    struct TestEvent {
        value: String,
    }

    struct TestListener {
        called: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl Listener<TestEvent> for TestListener {
        async fn handle(&self, _event: TestEvent) {
            self.called.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_all_event_provider_features() {
        // Run all async tests sequentially within one #[tokio::test] to avoid
        // parallel interference on the global EventService registry.

        // --- listen ---
        EventService::clear_all_listeners();
        let called = Arc::new(AtomicBool::new(false));
        let _p = EventServiceProvider::new().listen::<TestEvent, TestListener>(TestListener {
            called: called.clone(),
        });
        EventService::dispatch(TestEvent {
            value: "listen".into(),
        })
        .await;
        assert!(called.load(Ordering::SeqCst));

        // --- listen_fn ---
        EventService::clear_all_listeners();
        let called_fn = Arc::new(AtomicBool::new(false));
        let _p = EventServiceProvider::new().listen_fn::<TestEvent, _, _>({
            let c = called_fn.clone();
            move |_| {
                let c = c.clone();
                async move {
                    c.store(true, Ordering::SeqCst);
                }
            }
        });
        EventService::dispatch(TestEvent { value: "fn".into() }).await;
        assert!(called_fn.load(Ordering::SeqCst));

        // --- chain (listen + listen_fn) ---
        EventService::clear_all_listeners();
        let chain_a = Arc::new(AtomicBool::new(false));
        let chain_b = Arc::new(AtomicBool::new(false));
        let _p = EventServiceProvider::new()
            .listen::<TestEvent, TestListener>(TestListener {
                called: chain_a.clone(),
            })
            .listen_fn::<TestEvent, _, _>({
                let cb = chain_b.clone();
                move |_| {
                    let cb = cb.clone();
                    async move {
                        cb.store(true, Ordering::SeqCst);
                    }
                }
            });
        EventService::dispatch(TestEvent {
            value: "chain".into(),
        })
        .await;
        assert!(chain_a.load(Ordering::SeqCst));
        assert!(chain_b.load(Ordering::SeqCst));

        // --- registers immediately (no boot needed) ---
        EventService::clear_all_listeners();
        let immediate = Arc::new(AtomicBool::new(false));
        let _p = EventServiceProvider::new().listen::<TestEvent, TestListener>(TestListener {
            called: immediate.clone(),
        });
        EventService::dispatch(TestEvent {
            value: "immediate".into(),
        })
        .await;
        assert!(immediate.load(Ordering::SeqCst));

        // --- convenience function ---
        EventService::clear_all_listeners();
        let convenience = Arc::new(AtomicBool::new(false));
        let app = Application::new(None);
        register_event_listeners(&app, |p| {
            p.listen_fn::<TestEvent, _, _>({
                let c = convenience.clone();
                move |_| {
                    let c = c.clone();
                    async move {
                        c.store(true, Ordering::SeqCst);
                    }
                }
            })
        });
        EventService::dispatch(TestEvent {
            value: "convenience".into(),
        })
        .await;
        assert!(convenience.load(Ordering::SeqCst));
    }

    #[test]
    fn test_event_service_provider_default() {
        let _provider = EventServiceProvider::default();
        // No-op: just shouldn't panic
    }

    #[test]
    fn test_service_provider_trait_impl() {
        let provider = EventServiceProvider::new();
        assert_eq!(provider.provides(), vec!["events"]);
    }
}
