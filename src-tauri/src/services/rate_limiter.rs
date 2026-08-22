use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct RateLimiter {
    bytes_per_sec: u64,
    start: Instant,
    sent: AtomicU64,
}

impl RateLimiter {
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            bytes_per_sec,
            start: Instant::now(),
            sent: AtomicU64::new(0),
        }
    }

    pub async fn wait_for_bytes(&self, bytes: u64) {
        if self.bytes_per_sec == 0 || bytes == 0 {
            return;
        }

        let total = self.sent.fetch_add(bytes, Ordering::Relaxed) + bytes;
        let expected = Duration::from_secs_f64(total as f64 / self.bytes_per_sec as f64);
        let elapsed = self.start.elapsed();
        if expected > elapsed {
            tokio::time::sleep(expected - elapsed).await;
        }
    }
}
