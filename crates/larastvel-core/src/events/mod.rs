use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use once_cell::sync::Lazy;

type EventPayload = Box<dyn Any + Send + Sync>;
type ListenerFnInner =
    dyn Fn(EventPayload) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync;
type ListenerFn = Arc<ListenerFnInner>;

static REGISTRY: Lazy<Mutex<HashMap<TypeId, Vec<ListenerFn>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static FAKE_MODE: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static DISPATCHED_EVENTS: Lazy<Mutex<Vec<(TypeId, String)>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub trait Event: Any + Clone + Send + Sync {}
impl<T: Any + Clone + Send + Sync> Event for T {}

#[async_trait]
pub trait Listener<E: Event>: Send + Sync {
    async fn handle(&self, event: E);
}

pub struct EventService;

impl EventService {
    pub fn listen<E: Event, L>(listener: L)
    where
        L: Listener<E> + 'static,
    {
        let type_id = TypeId::of::<E>();
        let listener = Arc::new(listener);
        let fn_box: ListenerFn = Arc::new(move |payload: EventPayload| {
            let event = *payload.downcast::<E>().expect("Event type mismatch");
            let listener = listener.clone();
            Box::pin(async move {
                listener.handle(event).await;
            })
        });

        let mut registry = REGISTRY.lock().unwrap();
        registry.entry(type_id).or_default().push(fn_box);
    }

    pub fn listen_fn<E: Event, F, Fut>(f: F)
    where
        F: Fn(E) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let type_id = TypeId::of::<E>();
        let fn_box: ListenerFn = Arc::new(move |payload: EventPayload| {
            let event = *payload.downcast::<E>().expect("Event type mismatch");
            Box::pin(f(event))
        });

        let mut registry = REGISTRY.lock().unwrap();
        registry.entry(type_id).or_default().push(fn_box);
    }

    pub async fn dispatch<E: Event>(event: E) {
        {
            let fake = FAKE_MODE.lock().unwrap();
            if *fake {
                let mut dispatched = DISPATCHED_EVENTS.lock().unwrap();
                dispatched.push((TypeId::of::<E>(), std::any::type_name::<E>().to_string()));
                return;
            }
        }

        let type_id = TypeId::of::<E>();
        let listeners = {
            let registry = REGISTRY.lock().unwrap();
            registry.get(&type_id).cloned().unwrap_or_default()
        };

        for listener in &listeners {
            (listener.as_ref())(Box::new(event.clone())).await;
        }
    }

    pub fn fake() {
        let mut fake = FAKE_MODE.lock().unwrap();
        *fake = true;
        let mut dispatched = DISPATCHED_EVENTS.lock().unwrap();
        dispatched.clear();
    }

    pub fn assert_dispatched<E: Event>() -> bool {
        let dispatched = DISPATCHED_EVENTS.lock().unwrap();
        dispatched.iter().any(|(id, _)| *id == TypeId::of::<E>())
    }

    pub fn assert_not_dispatched<E: Event>() -> bool {
        !Self::assert_dispatched::<E>()
    }

    pub fn assert_dispatched_times<E: Event>(expected: usize) -> bool {
        let dispatched = DISPATCHED_EVENTS.lock().unwrap();
        let count = dispatched
            .iter()
            .filter(|(id, _)| *id == TypeId::of::<E>())
            .count();
        count == expected
    }

    pub fn clear_listeners<E: Event>() {
        let mut registry = REGISTRY.lock().unwrap();
        registry.remove(&TypeId::of::<E>());
    }

    pub fn clear_all_listeners() {
        let mut registry = REGISTRY.lock().unwrap();
        registry.clear();
    }

    pub fn reset() {
        Self::clear_all_listeners();
        let mut fake = FAKE_MODE.lock().unwrap();
        *fake = false;
        let mut dispatched = DISPATCHED_EVENTS.lock().unwrap();
        dispatched.clear();
    }

    pub fn has_listeners<E: Event>() -> bool {
        let registry = REGISTRY.lock().unwrap();
        registry.contains_key(&TypeId::of::<E>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[tokio::test]
    #[serial_test::serial]
    async fn test_all_events() {
        test_minimal_arc_fn_call().await;
        test_listen_and_dispatch_with_struct().await;
        test_listen_fn_and_dispatch().await;
        test_multiple_listeners().await;
        test_no_listeners_does_not_panic().await;
        test_clear_listeners().await;
        test_has_listeners().await;
        test_fake_mode_integration().await;
        test_clear_all_listeners().await;
        test_multiple_event_types().await;
        test_listener_receives_correct_data().await;
    }

    async fn test_minimal_arc_fn_call() {
        let called = Arc::new(AtomicBool::new(false));
        let c = called.clone();

        let f: ListenerFn = Arc::new(move |payload: EventPayload| {
            let _event = *payload.downcast::<String>().unwrap();
            let c = c.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
            })
        });

        f(Box::new("hello".to_string())).await;
        assert!(called.load(Ordering::SeqCst));
    }

    #[derive(Debug, Clone, PartialEq)]
    struct OrderShipped {
        order_id: String,
    }

    struct SendNotification;

    #[async_trait]
    impl Listener<OrderShipped> for SendNotification {
        async fn handle(&self, _event: OrderShipped) {}
    }

    async fn test_listen_and_dispatch_with_struct() {
        EventService::clear_all_listeners();
        let handled = Arc::new(AtomicBool::new(false));
        let h = handled.clone();

        struct TestListener(Arc<AtomicBool>);
        #[async_trait]
        impl Listener<OrderShipped> for TestListener {
            async fn handle(&self, _event: OrderShipped) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        EventService::listen::<OrderShipped, TestListener>(TestListener(h));
        EventService::dispatch(OrderShipped {
            order_id: "123".into(),
        })
        .await;

        assert!(handled.load(Ordering::SeqCst));
        EventService::clear_listeners::<OrderShipped>();
    }

    async fn test_listen_fn_and_dispatch() {
        EventService::clear_all_listeners();
        let handled = Arc::new(AtomicBool::new(false));
        let h = handled.clone();

        EventService::listen_fn::<OrderShipped, _, _>(move |_event| {
            let h = h.clone();
            async move {
                h.store(true, Ordering::SeqCst);
            }
        });

        EventService::dispatch(OrderShipped {
            order_id: "456".into(),
        })
        .await;

        assert!(handled.load(Ordering::SeqCst));
        EventService::clear_listeners::<OrderShipped>();
    }

    async fn test_multiple_listeners() {
        EventService::clear_all_listeners();
        let call_count = Arc::new(AtomicUsize::new(0));
        let c1 = call_count.clone();
        let c2 = call_count.clone();

        EventService::listen_fn::<OrderShipped, _, _>(move |_| {
            let c = c1.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        });

        EventService::listen_fn::<OrderShipped, _, _>(move |_| {
            let c = c2.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        });

        EventService::dispatch(OrderShipped {
            order_id: "789".into(),
        })
        .await;

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
        EventService::clear_listeners::<OrderShipped>();
    }

    async fn test_no_listeners_does_not_panic() {
        EventService::clear_all_listeners();
        EventService::dispatch(OrderShipped {
            order_id: "no-listeners".into(),
        })
        .await;
    }

    async fn test_clear_listeners() {
        EventService::clear_all_listeners();
        let handled = Arc::new(AtomicBool::new(false));
        let h = handled.clone();

        EventService::listen_fn::<OrderShipped, _, _>(move |_| {
            let h = h.clone();
            async move {
                h.store(true, Ordering::SeqCst);
            }
        });

        EventService::clear_listeners::<OrderShipped>();
        assert!(!EventService::has_listeners::<OrderShipped>());

        EventService::dispatch(OrderShipped {
            order_id: "cleared".into(),
        })
        .await;

        assert!(!handled.load(Ordering::SeqCst));
    }

    async fn test_has_listeners() {
        EventService::clear_all_listeners();
        assert!(!EventService::has_listeners::<OrderShipped>());

        EventService::listen::<OrderShipped, SendNotification>(SendNotification);
        assert!(EventService::has_listeners::<OrderShipped>());

        EventService::clear_all_listeners();
    }

    async fn test_fake_mode_integration() {
        EventService::clear_all_listeners();

        assert!(EventService::assert_not_dispatched::<OrderShipped>());

        EventService::fake();

        let handled = Arc::new(AtomicBool::new(false));
        let h = handled.clone();
        EventService::listen_fn::<OrderShipped, _, _>(move |_| {
            let h = h.clone();
            async move {
                h.store(true, Ordering::SeqCst);
            }
        });

        EventService::dispatch(OrderShipped {
            order_id: "fake-test".into(),
        })
        .await;
        assert!(
            !handled.load(Ordering::SeqCst),
            "listener should not be called in fake mode"
        );

        assert!(EventService::assert_dispatched::<OrderShipped>());

        EventService::dispatch(OrderShipped {
            order_id: "another".into(),
        })
        .await;
        assert!(EventService::assert_dispatched_times::<OrderShipped>(2));

        EventService::reset();
        assert!(EventService::assert_not_dispatched::<OrderShipped>());
    }

    async fn test_clear_all_listeners() {
        EventService::clear_all_listeners();
        EventService::listen::<OrderShipped, SendNotification>(SendNotification);
        assert!(EventService::has_listeners::<OrderShipped>());
        EventService::clear_all_listeners();
        assert!(!EventService::has_listeners::<OrderShipped>());
    }

    #[derive(Debug, Clone, PartialEq)]
    struct UserRegistered {
        email: String,
    }

    async fn test_multiple_event_types() {
        EventService::clear_all_listeners();
        let order_handled = Arc::new(AtomicBool::new(false));
        let user_handled = Arc::new(AtomicBool::new(false));
        let oh = order_handled.clone();
        let uh = user_handled.clone();

        EventService::listen_fn::<OrderShipped, _, _>(move |_| {
            let oh = oh.clone();
            async move {
                oh.store(true, Ordering::SeqCst);
            }
        });

        EventService::listen_fn::<UserRegistered, _, _>(move |_| {
            let uh = uh.clone();
            async move {
                uh.store(true, Ordering::SeqCst);
            }
        });

        EventService::dispatch(OrderShipped {
            order_id: "multi".into(),
        })
        .await;

        assert!(order_handled.load(Ordering::SeqCst));
        assert!(!user_handled.load(Ordering::SeqCst));

        EventService::dispatch(UserRegistered {
            email: "test@test.com".into(),
        })
        .await;

        assert!(user_handled.load(Ordering::SeqCst));
        EventService::clear_all_listeners();
    }

    async fn test_listener_receives_correct_data() {
        EventService::clear_all_listeners();
        let received = Arc::new(Mutex::new(None));
        let r = received.clone();

        EventService::listen_fn::<OrderShipped, _, _>(move |event| {
            let r = r.clone();
            async move {
                let mut recv = r.lock().unwrap();
                *recv = Some(event.order_id);
            }
        });

        EventService::dispatch(OrderShipped {
            order_id: "data-check".into(),
        })
        .await;

        let recv = received.lock().unwrap();
        assert_eq!(recv.as_deref(), Some("data-check"));
        EventService::clear_all_listeners();
    }
}
