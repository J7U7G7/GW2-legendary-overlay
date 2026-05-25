use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::{StatusCode, header::USER_AGENT};
use serde::de::DeserializeOwned;
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::api::auth::ApiKey;
use crate::error::{AppError, Result};

const BASE_URL: &str = "https://api.guildwars2.com";
const USER_AGENT_VALUE: &str = concat!("gw2-overlay/", env!("CARGO_PKG_VERSION"));

/// Token bucket: 300 requests/minute (5/s), capacity 300 — safe margin under
/// the 600/min documented limit.
const RATE_CAPACITY: f64 = 300.0;
const RATE_REFILL_PER_SEC: f64 = 5.0;

/// Backoff schedule for 429/5xx: 1s, 2s, 4s, 8s — then give up.
const MAX_RETRIES: usize = 4;
const BASE_BACKOFF_MS: u64 = 1000;
const MAX_BACKOFF_MS: u64 = 30_000;

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl Bucket {
    fn new() -> Self {
        Self::with_origin(Instant::now())
    }

    /// Testable constructor: lets the caller anchor `last_refill` to a known
    /// instant. Production callers use `new()` which anchors to the current
    /// instant.
    fn with_origin(origin: Instant) -> Self {
        Self { tokens: RATE_CAPACITY, last_refill: origin }
    }

    fn try_take(&mut self) -> std::result::Result<(), Duration> {
        self.try_take_at(Instant::now())
    }

    /// Time-injectable variant of `try_take`. Production code goes through
    /// `try_take` which calls `Instant::now()` itself; tests pass synthetic
    /// instants derived from a fixed origin so they don't depend on the
    /// host's monotonic-clock origin (Windows runners may underflow when
    /// subtracting a Duration from `Instant::now()` shortly after boot).
    fn try_take_at(&mut self, now: Instant) -> std::result::Result<(), Duration> {
        let elapsed = now.saturating_duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * RATE_REFILL_PER_SEC).min(RATE_CAPACITY);
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            let needed = 1.0 - self.tokens;
            Err(Duration::from_secs_f64(needed / RATE_REFILL_PER_SEC))
        }
    }
}

pub struct ApiClient {
    http: reqwest::Client,
    key: Option<ApiKey>,
    bucket: Mutex<Bucket>,
}

#[allow(dead_code)] // public API consumed by sync/* in upcoming step
impl ApiClient {
    pub fn new(key: Option<ApiKey>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .gzip(true)
            .build()?;
        Ok(Self { http, key, bucket: Mutex::new(Bucket::new()) })
    }

    async fn acquire_token(&self) {
        loop {
            let wait = {
                let mut b = self.bucket.lock().expect("rate bucket poisoned");
                match b.try_take() {
                    Ok(()) => return,
                    Err(d) => d,
                }
            };
            debug!(?wait, "rate-limited, sleeping");
            sleep(wait).await;
        }
    }

    /// Authenticated GET returning a deserialized JSON payload.
    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let key = self.key.as_ref().ok_or(AppError::NoApiKey)?;
        let url = format!("{BASE_URL}{path}");

        let mut backoff = BASE_BACKOFF_MS;
        let mut last_status: Option<u16> = None;
        let mut last_was_rate_limit = false;
        for attempt in 0..MAX_RETRIES {
            self.acquire_token().await;
            debug!(%url, attempt, "GET");
            let res = self
                .http
                .get(&url)
                .header(USER_AGENT, USER_AGENT_VALUE)
                .header(reqwest::header::AUTHORIZATION, key.as_bearer())
                .send()
                .await;

            match res {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        return r.json::<T>().await.map_err(Into::into);
                    }
                    match status {
                        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                            return Err(AppError::Unauthorized);
                        }
                        StatusCode::TOO_MANY_REQUESTS => {
                            warn!(%url, "429 rate-limited by GW2 API");
                            last_status = Some(status.as_u16());
                            last_was_rate_limit = true;
                            sleep(Duration::from_millis(backoff)).await;
                            backoff = (backoff * 2).min(MAX_BACKOFF_MS);
                            continue;
                        }
                        s if s.is_server_error() => {
                            warn!(status = %s, %url, "server error, retrying");
                            last_status = Some(s.as_u16());
                            last_was_rate_limit = false;
                            sleep(Duration::from_millis(backoff)).await;
                            backoff = (backoff * 2).min(MAX_BACKOFF_MS);
                            continue;
                        }
                        s => {
                            let body = r.text().await.unwrap_or_default();
                            return Err(AppError::Api { status: s.as_u16(), body });
                        }
                    }
                }
                Err(e) if e.is_timeout() || e.is_connect() => {
                    warn!(error = %e, %url, "transient HTTP error, retrying");
                    last_status = None;
                    last_was_rate_limit = false;
                    sleep(Duration::from_millis(backoff)).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF_MS);
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        if last_was_rate_limit {
            Err(AppError::RateLimited)
        } else {
            Err(AppError::Unavailable(last_status.unwrap_or(0)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_starts_full() {
        let mut b = Bucket::new();
        for _ in 0..300 {
            assert!(b.try_take().is_ok());
        }
        assert!(b.try_take().is_err());
    }

    #[test]
    fn bucket_refills_proportionally() {
        // Synthesise the timeline forward from a fixed origin so we never
        // subtract from `Instant::now()` (which can underflow on freshly-
        // booted machines, e.g. Windows CI runners).
        let t0 = Instant::now();
        let mut b = Bucket::with_origin(t0);
        // Drain — all 300 tokens taken at t0.
        for _ in 0..(RATE_CAPACITY as usize) {
            assert!(b.try_take_at(t0).is_ok());
        }
        assert!(b.try_take_at(t0).is_err());

        // Two seconds later → 10 fresh tokens.
        let t1 = t0 + Duration::from_secs(2);
        for _ in 0..10 {
            assert!(b.try_take_at(t1).is_ok());
        }
        assert!(b.try_take_at(t1).is_err());
    }

    #[test]
    fn bucket_caps_at_capacity() {
        let t0 = Instant::now();
        let mut b = Bucket::with_origin(t0);
        // Drain at t0.
        for _ in 0..(RATE_CAPACITY as usize) {
            assert!(b.try_take_at(t0).is_ok());
        }
        // Jump forward an hour. Naively, 3600s × 5 tok/s = 18000 tokens, but
        // the bucket must cap at RATE_CAPACITY.
        let t_future = t0 + Duration::from_secs(3600);
        for _ in 0..(RATE_CAPACITY as usize) {
            assert!(b.try_take_at(t_future).is_ok());
        }
        assert!(b.try_take_at(t_future).is_err());
    }
}
