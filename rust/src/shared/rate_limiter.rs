/// Generic rate limiter — enforces a minimum interval between API calls.
///
/// Pre-built instances are defined below for each external API.

use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    last_request: Mutex<Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            last_request: Mutex::new(Instant::now() - Duration::from_secs(1)),
            min_interval,
        }
    }

    pub fn wait(&self) {
        let mut last = self.last_request.lock().unwrap_or_else(|e| e.into_inner());
        let elapsed = last.elapsed();
        if elapsed < self.min_interval {
            std::thread::sleep(self.min_interval - elapsed);
        }
        *last = Instant::now();
    }
}

pub static TMDB: Lazy<RateLimiter> =
    Lazy::new(|| RateLimiter::new(Duration::from_millis(250)));

pub static OPENSUBS: Lazy<RateLimiter> =
    Lazy::new(|| RateLimiter::new(Duration::from_millis(350)));
