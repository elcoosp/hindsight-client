//! Internal helpers: bounded-concurrency fan-out.

use futures::stream::{self, StreamExt};

/// Run `f` over `items` with at most `max_concurrent` in flight, preserving
/// input order in the output. `max_concurrent` of 0 means unlimited.
///
/// Useful for multi-bank fan-out (recall across N user banks, batch retains,
/// mental-model refresh sweeps) without pulling `futures` into user code.
pub async fn map_concurrent<T, R, F, Fut>(
    items: impl IntoIterator<Item = T>,
    max_concurrent: usize,
    f: F,
) -> Vec<R>
where
    T: Send + 'static,
    R: Send + 'static,
    F: Fn(T) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = R> + Send,
{
    let items: Vec<T> = items.into_iter().collect();
    let n = if max_concurrent == 0 {
        items.len().max(1)
    } else {
        max_concurrent
    };
    stream::iter(items.into_iter().map(f))
        .buffered(n)
        .collect::<Vec<R>>()
        .await
}
