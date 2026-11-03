//! Auto-paging iterator for the server's `{items,total,limit,offset}` list endpoints.

use std::future::Future;
use std::pin::Pin;

use crate::error::Result;
use crate::models::Page;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Lazily walks all pages of a paginated endpoint.
///
/// Constructed by the `*_paged` methods on [`crate::client::BankClient`] and
/// [`crate::client::Client`]. Fetches one page per [`Pager::next_page`] call;
/// [`Pager::collect_all`] drains everything (with the page size you configured).
pub struct Pager<T> {
    fetch: Box<dyn FnMut(i64) -> BoxFuture<'static, Result<Page<T>>> + Send>,
    next_offset: Option<i64>,
    total: Option<i64>,
}

impl<T: Send + 'static> Pager<T> {
    /// Build a pager from an async fetch closure that takes an offset and
    /// returns the page starting at that offset.
    pub fn new<F, Fut>(mut fetch: F) -> Self
    where
        F: FnMut(i64) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Page<T>>> + Send + 'static,
    {
        Self {
            fetch: Box::new(move |offset| Box::pin(fetch(offset))),
            next_offset: Some(0),
            total: None,
        }
    }

    /// Fetch the next page, or `None` when the list is exhausted.
    pub async fn next_page(&mut self) -> Option<Result<crate::models::Page<T>>> {
        let offset = self.next_offset?;
        let page = (self.fetch)(offset).await;
        match page {
            Ok(p) => {
                self.next_offset = p.next_offset();
                self.total = Some(p.total);
                Some(Ok(p))
            }
            Err(e) => {
                // Stop iterating after an error; the caller can retry the call.
                self.next_offset = None;
                Some(Err(e))
            }
        }
    }

    /// Drain every remaining item across pages into one vector.
    ///
    /// # Errors
    /// Returns the first error encountered while paging.
    pub async fn collect_all(mut self) -> Result<Vec<T>> {
        let mut out = Vec::new();
        while let Some(page) = self.next_page().await {
            out.extend(page?.items);
        }
        Ok(out)
    }

    /// Total count reported by the server, once the first page has been fetched.
    pub fn total(&self) -> Option<i64> {
        self.total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_of(offset: i64, total: i64) -> crate::models::Page<u32> {
        let items: Vec<u32> = (offset..(offset + 10).min(total))
            .map(|i| i as u32)
            .collect();
        crate::models::Page {
            items,
            total,
            limit: 10,
            offset,
        }
    }

    #[tokio::test]
    async fn pages_until_exhausted() {
        let mut pager = Pager::new(|offset| async move { Ok(page_of(offset, 25)) });
        let mut offsets = Vec::new();
        while let Some(p) = pager.next_page().await {
            offsets.push(p.unwrap().offset);
        }
        assert_eq!(offsets, vec![0, 10, 20]);
    }

    #[tokio::test]
    async fn collect_all_gathers_everything() {
        let pager = Pager::new(|offset| async move { Ok(page_of(offset, 25)) });
        let all = pager.collect_all().await.unwrap();
        assert_eq!(all, (0..25).collect::<Vec<_>>());
    }
}
