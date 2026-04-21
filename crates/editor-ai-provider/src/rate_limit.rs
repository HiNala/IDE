//! Simple sliding-window limiter: at most N completed `chat` calls per rolling minute.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// At most `max_per_minute` acquisitions per rolling 60-second window.
#[derive(Debug)]
pub struct MinuteRateLimit {
    max_per_minute: u32,
    inner: Mutex<Vec<Instant>>,
}

impl MinuteRateLimit {
    pub fn new(max_per_minute: u32) -> Arc<Self> {
        Arc::new(Self { max_per_minute: max_per_minute.max(1), inner: Mutex::new(Vec::new()) })
    }

    /// Wait until a slot is available, then record this request.
    pub async fn acquire(&self) {
        loop {
            let sleep = {
                let mut g = self.inner.lock().await;
                let now = Instant::now();
                g.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
                if (g.len() as u32) < self.max_per_minute {
                    g.push(now);
                    return;
                }
                let earliest = *g.iter().min().expect("non-empty");
                (earliest + Duration::from_secs(60)).saturating_duration_since(now)
            };
            tokio::time::sleep(sleep.max(Duration::from_millis(1))).await;
        }
    }
}
