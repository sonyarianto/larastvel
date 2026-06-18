use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Type alias for a pipe stage implemented as a boxed closure.
///
/// Each pipe receives the current value and a `Next<T>` to pass the
/// transformed value to the next stage.
pub type PipeFn<T> =
    Box<dyn Fn(T, Next<T>) -> Pin<Box<dyn Future<Output = T> + Send>> + Send + Sync>;

/// Wrapper for the next stage in a pipeline.
#[allow(clippy::type_complexity)]
pub struct Next<T> {
    next: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send>,
}

impl<T: Send + 'static> Next<T> {
    /// Pass the value to the next stage.
    pub async fn call(self, value: T) -> T {
        (self.next)(value).await
    }
}

/// Trait for pipeline stages.
///
/// Implement this trait on your struct and use [`into_pipe_fn`] to
/// convert it into a [`PipeFn`] closure.
///
/// # Example
///
/// ```ignore
/// struct UpperCasePipe;
///
/// #[async_trait::async_trait]
/// impl Pipe<String> for UpperCasePipe {
///     async fn handle(&self, value: String, next: Next<String>) -> String {
///         next.call(value.to_uppercase()).await
///     }
/// }
///
/// let pipe: PipeFn<String> = into_pipe_fn(UpperCasePipe);
/// ```
#[async_trait::async_trait]
pub trait Pipe<T: Send + 'static>: Send + Sync {
    /// Process the value and pass it to the next stage.
    async fn handle(&self, value: T, next: Next<T>) -> T;
}

/// Convert a [`Pipe`] trait implementation into a [`PipeFn`] closure.
pub fn into_pipe_fn<T, P>(pipe: P) -> PipeFn<T>
where
    T: Send + 'static,
    P: Pipe<T> + 'static,
{
    let pipe = Arc::new(pipe);
    Box::new(move |value, next| {
        let pipe = pipe.clone();
        Box::pin(async move { pipe.handle(value, next).await })
    })
}

/// Create a [`PipeFn`] from a closure.
///
/// # Example
///
/// ```ignore
/// pipe_fn(|value: String, next: Next<String>| async move {
///     let modified = format!("Hello, {value}!");
///     next.call(modified).await
/// })
/// ```
pub fn pipe_fn<T, F, Fut>(f: F) -> PipeFn<T>
where
    T: Send + 'static,
    F: Fn(T, Next<T>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static,
{
    Box::new(move |value, next| Box::pin(f(value, next)))
}

/// A Laravel-style pipeline that sends a value through a series of pipes.
pub struct Pipeline<T: Send + 'static> {
    value: Option<T>,
    pipes: Vec<PipeFn<T>>,
}

impl<T: Send + 'static> Pipeline<T> {
    /// Create a new pipeline with the given initial value.
    pub fn send(value: T) -> Self {
        Self {
            value: Some(value),
            pipes: vec![],
        }
    }

    /// Set the pipes the value should be sent through.
    pub fn through(mut self, pipes: Vec<PipeFn<T>>) -> Self {
        self.pipes = pipes;
        self
    }

    /// Execute the pipeline, passing the value through all pipes and then
    /// to the destination closure.
    #[allow(clippy::type_complexity)]
    pub async fn then<F, Fut>(mut self, destination: F) -> T
    where
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        let value = self.value.take().expect("Pipeline already executed");

        // Build the chain from the inside out so pipe0 receives the
        // original value and pipeN passes its result to destination.
        let mut next: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send> =
            Box::new(move |val| Box::pin(destination(val)));

        for pipe in self.pipes.into_iter().rev() {
            let prev_next = next;
            next = Box::new(move |val| {
                let next = Next { next: prev_next };
                pipe(val, next)
            });
        }

        next(value).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_pipeline() {
        let result = Pipeline::send(42).then(|val| async move { val }).await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_single_pipe() {
        let result = Pipeline::send(1)
            .through(vec![pipe_fn(|value: i32, next: Next<i32>| async move {
                next.call(value + 1).await
            })])
            .then(|val| async move { val })
            .await;
        assert_eq!(result, 2);
    }

    #[tokio::test]
    async fn test_multiple_pipes() {
        let result = Pipeline::send(1)
            .through(vec![
                pipe_fn(|value: i32, next: Next<i32>| async move { next.call(value + 1).await }),
                pipe_fn(|value: i32, next: Next<i32>| async move { next.call(value * 2).await }),
                pipe_fn(|value: i32, next: Next<i32>| async move { next.call(value - 3).await }),
            ])
            .then(|val| async move { val })
            .await;
        // ((1 + 1) * 2) - 3 = 1
        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn test_pipes_execute_in_order() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(vec![]));

        let log1 = log.clone();
        let log2 = log.clone();
        let log_dest = log.clone();

        let result = Pipeline::send(0)
            .through(vec![
                pipe_fn(move |value: i32, next: Next<i32>| {
                    let log = log1.clone();
                    async move {
                        log.lock().unwrap().push("pipe1");
                        next.call(value).await
                    }
                }),
                pipe_fn(move |value: i32, next: Next<i32>| {
                    let log = log2.clone();
                    async move {
                        log.lock().unwrap().push("pipe2");
                        next.call(value).await
                    }
                }),
            ])
            .then(move |val| {
                let log = log_dest.clone();
                async move {
                    log.lock().unwrap().push("dest");
                    val
                }
            })
            .await;

        assert_eq!(result, 0);
        let log = log.lock().unwrap();
        assert_eq!(*log, vec!["pipe1", "pipe2", "dest"]);
    }

    #[tokio::test]
    async fn test_struct_pipe() {
        use async_trait::async_trait;

        struct AddPipe(i32);

        #[async_trait]
        impl Pipe<i32> for AddPipe {
            async fn handle(&self, value: i32, next: Next<i32>) -> i32 {
                next.call(value + self.0).await
            }
        }

        let result = Pipeline::send(10)
            .through(vec![into_pipe_fn(AddPipe(5))])
            .then(|val| async move { val })
            .await;
        assert_eq!(result, 15);
    }

    #[tokio::test]
    async fn test_pipe_does_not_need_to_call_next() {
        // A pipe can return a value without calling next (short-circuit).
        let result = Pipeline::send("hello")
            .through(vec![pipe_fn(
                |_value: &'static str, _next: Next<&'static str>| async move { "intercepted" },
            )])
            .then(|val| async move { val })
            .await;
        assert_eq!(result, "intercepted");
    }

    #[tokio::test]
    async fn test_pipeline_with_strings() {
        let result = Pipeline::send("hello")
            .through(vec![pipe_fn(
                |_value: &'static str, next: Next<&'static str>| async move {
                    next.call("modified").await
                },
            )])
            .then(|val| async move { val })
            .await;
        assert_eq!(result, "modified");
    }
}
