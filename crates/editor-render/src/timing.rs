//! Per-frame timing with a rolling window of recent frame deltas (M03).

use std::time::{Duration, Instant};

/// Number of recent frame intervals kept for percentile stats (~1 s at 120 fps).
const ROLLING_LEN: usize = 120;

/// Records wall-clock deltas between successive [`FrameTimer::tick`] calls (frame starts).
#[derive(Debug)]
pub struct FrameTimer {
    last_tick: Option<Instant>,
    last_delta: Duration,
    deltas: [Duration; ROLLING_LEN],
    head: usize,
    filled: usize,
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameTimer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_tick: None,
            last_delta: Duration::ZERO,
            deltas: [Duration::ZERO; ROLLING_LEN],
            head: 0,
            filled: 0,
        }
    }

    /// Call once at the beginning of each frame. Computes the delta since the previous tick.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if let Some(prev) = self.last_tick {
            self.last_delta = now.saturating_duration_since(prev);
            self.deltas[self.head] = self.last_delta;
            self.head = (self.head + 1) % ROLLING_LEN;
            self.filled = self.filled.saturating_add(1).min(ROLLING_LEN);
        }
        self.last_tick = Some(now);
    }

    #[must_use]
    pub fn last_delta(&self) -> Duration {
        self.last_delta
    }

    #[must_use]
    pub fn average_fps(&self) -> f32 {
        let n = self.filled.min(ROLLING_LEN);
        if n == 0 {
            return 0.0;
        }
        let sum_ns: u128 = (0..n)
            .map(|i| {
                let idx = (self.head + ROLLING_LEN - 1 - i) % ROLLING_LEN;
                self.deltas[idx].as_nanos()
            })
            .sum();
        let mean_ns = sum_ns / (n as u128);
        if mean_ns == 0 {
            return f32::INFINITY;
        }
        1.0e9_f32 / mean_ns as f32
    }

    /// Percentile of frame times over the rolling window (`p` in 0.0…1.0).
    #[must_use]
    pub fn percentile_frame_time(&self, p: f64) -> Duration {
        let n = self.filled.min(ROLLING_LEN);
        if n == 0 {
            return Duration::ZERO;
        }
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let idx = (self.head + ROLLING_LEN - 1 - i) % ROLLING_LEN;
            v.push(self.deltas[idx]);
        }
        v.sort_by_key(|d| d.as_nanos());
        let idx = ((n as f64 - 1.0) * p.clamp(0.0, 1.0)).round() as usize;
        v[idx.min(n - 1)]
    }

    #[must_use]
    pub fn p95_frame_time(&self) -> Duration {
        self.percentile_frame_time(0.95)
    }

    #[must_use]
    pub fn p99_frame_time(&self) -> Duration {
        self.percentile_frame_time(0.99)
    }
}

#[cfg(test)]
mod tests {
    use super::FrameTimer;
    use std::time::Duration;
    use std::{thread, time};

    #[test]
    fn rolling_window_and_percentiles() {
        let mut t = FrameTimer::new();
        t.tick();
        thread::sleep(time::Duration::from_millis(5));
        t.tick();
        assert!(t.last_delta() >= Duration::from_millis(4));

        for _ in 0..50 {
            t.tick();
            thread::sleep(time::Duration::from_millis(2));
        }
        assert!(t.average_fps() > 0.0);
        assert!(t.p95_frame_time() >= Duration::from_nanos(1));
        assert!(t.p99_frame_time() >= t.p95_frame_time());
    }
}
