# Pipeline

Larastvel's Pipeline provides a way to send a value through a series of pipes (stages), where each pipe can inspect or transform the value before passing it to the next stage. This is useful for data transformation pipelines, middleware-like processing chains, and multi-step workflows.

## Basic Usage

```rust
use larastvel_core::{Pipeline, pipe_fn};

let result = Pipeline::send(1)
    .through(vec![
        pipe_fn(|value: i32, next| async move {
            next.call(value + 1).await
        }),
        pipe_fn(|value: i32, next| async move {
            next.call(value * 2).await
        }),
    ])
    .then(|val| async move { val })
    .await;

assert_eq!(result, 4); // ((1 + 1) * 2)
```

## Using Struct Pipes

For reusable pipe logic, implement the `Pipe` trait:

```rust
use larastvel_core::{Pipeline, into_pipe_fn, Next, Pipe};
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
```

## Short-Circuiting

A pipe can return a value without calling `next`, which short-circuits the pipeline:

```rust
let result = Pipeline::send("hello")
    .through(vec![
        pipe_fn(|_: &'static str, _next| async move { "intercepted" }),
    ])
    .then(|val| async move { val })
    .await;

assert_eq!(result, "intercepted");
```

## The `Pipe` Trait

```rust
#[async_trait]
pub trait Pipe<T: Send + 'static>: Send + Sync {
    async fn handle(&self, value: T, next: Next<T>) -> T;
}
```

Use `into_pipe_fn(MyStruct)` to convert a `Pipe` implementation into a closure for the pipeline.
