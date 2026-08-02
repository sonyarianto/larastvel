//! Concurrency helpers — Laravel's `Concurrency` facade.

use std::future::Future;

/// Error returned when a concurrent task fails or panics.
#[derive(Debug)]
pub enum ConcurrencyError {
    /// The task panicked or the runtime failed to join it.
    TaskFailed,
}

impl std::fmt::Display for ConcurrencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TaskFailed => write!(f, "concurrent task failed or panicked"),
        }
    }
}

impl std::error::Error for ConcurrencyError {}

/// Run async tasks concurrently — Laravel's `Concurrency::concurrent()`.
///
/// Results are returned in the order the tasks were given, not in
/// completion order.
///
/// ```rust,ignore
/// let results = Concurrency::concurrent(vec![
///     Box::pin(async { fetch_user(1).await }),
///     Box::pin(async { fetch_user(2).await }),
/// ]).await?;
/// ```
pub async fn concurrent<T>(
    tasks: Vec<std::pin::Pin<Box<dyn Future<Output = T> + Send>>>,
) -> Result<Vec<T>, ConcurrencyError>
where
    T: Send + 'static,
{
    let mut set = tokio::task::JoinSet::new();
    for (i, task) in tasks.into_iter().enumerate() {
        set.spawn(async move { (i, task.await) });
    }

    let mut results: Vec<(usize, T)> = Vec::new();
    while let Some(joined) = set.join_next().await {
        let (i, value) = joined.map_err(|_| ConcurrencyError::TaskFailed)?;
        results.push((i, value));
    }

    results.sort_by_key(|(i, _)| *i);
    Ok(results.into_iter().map(|(_, v)| v).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_preserves_input_order() {
        let results = concurrent(vec![
            Box::pin(async { 1 }),
            Box::pin(async { 2 }),
            Box::pin(async { 3 }),
        ])
        .await
        .unwrap();
        assert_eq!(results, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_concurrent_with_different_durations() {
        let results = concurrent(vec![
            Box::pin(async {
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                "slow"
            }),
            Box::pin(async { "fast" }),
        ])
        .await
        .unwrap();
        assert_eq!(results, vec!["slow", "fast"]);
    }
}
