//! Retry policy with full-jitter exponential backoff and `Retry-After` support.

use std::time::Duration;

/// Which requests a policy applies to (also read via [`RetryPolicy::scope`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Idempotent reads only (GET/HEAD).
    Reads,
    /// Every request, including writes (POST/PUT/PATCH/DELETE).
    Everything,
}

/// Retry policy: how hard the client tries before giving up.
///
/// Defaults to 2 retries (3 attempts total) with full-jitter exponential
/// backoff between 100 ms and 5 s, retrying `429`, `500`, `502`, `503`, `504`
/// and transport errors on read requests. Enable [`RetryPolicy::ALL_REQUESTS`]
/// (or `ClientBuilder::retry_writes`) to also retry writes.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Number of *retries* after the initial attempt (0 = call once, never retry).
    pub(crate) max_retries: u32,
    /// Base delay for the first backoff.
    pub(crate) base_delay: Duration,
    /// Upper bound for any single backoff.
    pub(crate) max_delay: Duration,
    /// Apply full jitter (randomized delay in `[0, computed]`) instead of the exact delay.
    pub(crate) jitter: bool,
    /// Honor the server's `Retry-After` header when present on 429/503.
    pub(crate) respect_retry_after: bool,
    /// Which requests this policy applies to.
    pub scope: Scope,
}

impl RetryPolicy {
    /// Policy that retries reads only.
    pub const READS_ONLY: Scope = Scope::Reads;
    /// Policy that retries every request, writes included.
    pub const ALL_REQUESTS: Scope = Scope::Everything;

    /// Default policy: 2 retries, jittered backoff, reads only.
    pub fn new() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            jitter: true,
            respect_retry_after: true,
            scope: Scope::Reads,
        }
    }

    /// Never retry anything.
    pub fn disabled() -> Self {
        Self {
            max_retries: 0,
            scope: Scope::Reads,
            ..Self::new()
        }
    }

    /// Set the number of retries (initial attempt excluded).
    #[must_use]
    pub fn retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Set the base backoff delay.
    #[must_use]
    pub fn base_delay(mut self, d: Duration) -> Self {
        self.base_delay = d;
        self
    }

    /// Set the maximum backoff delay.
    #[must_use]
    pub fn max_delay(mut self, d: Duration) -> Self {
        self.max_delay = d;
        self
    }

    /// Disable jitter (use the exact exponential delay).
    #[must_use]
    pub fn no_jitter(mut self) -> Self {
        self.jitter = false;
        self
    }

    /// Ignore the server's `Retry-After` header.
    #[must_use]
    pub fn ignore_retry_after(mut self) -> Self {
        self.respect_retry_after = false;
        self
    }

    /// Retry writes as well as reads. Only enable this for operations that are
    /// idempotent from your application's point of view (e.g. reflect, or
    /// retain calls that deduplicate by content hash).
    #[must_use]
    pub fn all_requests(mut self) -> Self {
        self.scope = Scope::Everything;
        self
    }

    /// Whether a request with this method should be retried by this policy.
    pub(crate) fn applies_to(&self, method: &reqwest::Method) -> bool {
        match self.scope {
            Scope::Everything => true,
            Scope::Reads => matches!(*method, reqwest::Method::GET | reqwest::Method::HEAD),
        }
    }

    /// Whether this HTTP status is retryable.
    pub(crate) fn is_retryable_status(status: reqwest::StatusCode) -> bool {
        matches!(
            status,
            reqwest::StatusCode::TOO_MANY_REQUESTS
                | reqwest::StatusCode::INTERNAL_SERVER_ERROR
                | reqwest::StatusCode::BAD_GATEWAY
                | reqwest::StatusCode::SERVICE_UNAVAILABLE
                | reqwest::StatusCode::GATEWAY_TIMEOUT
        )
    }

    /// Compute the delay before attempt `n` (0-based retry index).
    pub(crate) fn delay_for(&self, retry_index: u32, retry_after: Option<Duration>) -> Duration {
        if self.respect_retry_after {
            if let Some(ra) = retry_after {
                return ra;
            }
        }
        let exp = self
            .base_delay
            .saturating_mul(2u32.saturating_pow(retry_index.min(16)));
        let capped = exp.min(self.max_delay);
        if self.jitter {
            full_jitter(capped)
        } else {
            capped
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Full jitter: uniform random in `[0, delay]`.
pub(crate) fn full_jitter(delay: Duration) -> Duration {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Cheap xorshift seeded from the clock — good enough for backoff spacing.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0x9E37_79B9_7F4A_7C15, |d| {
            u64::from(d.subsec_nanos()) ^ d.as_secs()
        });
    let mut x = nanos | 1;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    let frac = (x % 10_000) as f64 / 10_000.0;
    Duration::from_secs_f64(delay.as_secs_f64() * frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_grows_exponentially_and_caps() {
        let p = RetryPolicy::new().no_jitter();
        assert_eq!(p.delay_for(0, None), Duration::from_millis(100));
        assert_eq!(p.delay_for(1, None), Duration::from_millis(200));
        assert_eq!(p.delay_for(2, None), Duration::from_millis(400));
        assert_eq!(p.delay_for(20, None), Duration::from_secs(5));
    }

    #[test]
    fn retry_after_wins_when_respected() {
        let p = RetryPolicy::new();
        assert_eq!(
            p.delay_for(0, Some(Duration::from_secs(7))),
            Duration::from_secs(7)
        );
        let p = p.ignore_retry_after();
        assert_ne!(
            p.delay_for(0, Some(Duration::from_secs(7))),
            Duration::from_secs(7)
        );
    }

    #[test]
    fn jitter_stays_within_delay() {
        let p = RetryPolicy::new()
            .base_delay(Duration::from_secs(1))
            .max_delay(Duration::from_secs(1));
        for _ in 0..50 {
            assert!(p.delay_for(3, None) <= Duration::from_secs(1));
        }
    }

    #[test]
    fn scope_gating() {
        let reads = RetryPolicy::new();
        assert!(reads.applies_to(&reqwest::Method::GET));
        assert!(!reads.applies_to(&reqwest::Method::POST));
        let all = RetryPolicy::new().all_requests();
        assert!(all.applies_to(&reqwest::Method::DELETE));
    }

    #[test]
    fn retryable_statuses() {
        assert!(RetryPolicy::is_retryable_status(
            reqwest::StatusCode::TOO_MANY_REQUESTS
        ));
        assert!(RetryPolicy::is_retryable_status(
            reqwest::StatusCode::BAD_GATEWAY
        ));
        assert!(!RetryPolicy::is_retryable_status(
            reqwest::StatusCode::BAD_REQUEST
        ));
        assert!(!RetryPolicy::is_retryable_status(reqwest::StatusCode::OK));
    }
}
