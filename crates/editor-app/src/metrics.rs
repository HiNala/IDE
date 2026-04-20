//! Rolling frame timings + optional RSS sampling (M07 observability).

use std::collections::VecDeque;
use std::time::Duration;

/// Single heap sample from the OS (best-effort).
#[derive(Debug, Clone, Copy)]
pub struct HeapSample {
    pub resident_bytes: u64,
}

/// Point-in-time view for logging / HUD.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub p50_frame: Duration,
    pub p95_frame: Duration,
    pub p99_frame: Duration,
    pub avg_fps: f32,
    pub last_prepare: Duration,
    pub last_submit: Duration,
    pub last_heap: Option<HeapSample>,
}

/// Rolling window of recent frame timings (default 120 frames).
pub struct MetricsCollector {
    frame_times: VecDeque<Duration>,
    prepare_times: VecDeque<Duration>,
    submit_times: VecDeque<Duration>,
    last_prepare: Duration,
    last_submit: Duration,
    last_heap: Option<HeapSample>,
    heap_sample_interval: Duration,
    last_heap_sample_at: std::time::Instant,
}

impl MetricsCollector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            frame_times: VecDeque::new(),
            prepare_times: VecDeque::new(),
            submit_times: VecDeque::new(),
            last_prepare: Duration::ZERO,
            last_submit: Duration::ZERO,
            last_heap: None,
            heap_sample_interval: Duration::from_secs(1),
            last_heap_sample_at: std::time::Instant::now()
                .checked_sub(Duration::from_secs(10))
                .unwrap_or_else(std::time::Instant::now),
        }
    }

    pub fn record_frame(&mut self, prepare: Duration, submit: Duration, total: Duration) {
        self.last_prepare = prepare;
        self.last_submit = submit;
        Self::push_duration(&mut self.frame_times, total);
        Self::push_duration(&mut self.prepare_times, prepare);
        Self::push_duration(&mut self.submit_times, submit);

        let now = std::time::Instant::now();
        if now.duration_since(self.last_heap_sample_at) >= self.heap_sample_interval {
            self.last_heap_sample_at = now;
            if let Some(ms) = memory_stats::memory_stats() {
                self.last_heap = Some(HeapSample { resident_bytes: ms.physical_mem as u64 });
            }
        }
    }

    fn push_duration(q: &mut VecDeque<Duration>, d: Duration) {
        q.push_back(d);
        const CAP: usize = 120;
        while q.len() > CAP {
            q.pop_front();
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut v: Vec<Duration> = self.frame_times.iter().copied().collect();
        v.sort_unstable();
        let p50 = percentile_sorted(&v, 0.50);
        let p95 = percentile_sorted(&v, 0.95);
        let p99 = percentile_sorted(&v, 0.99);
        let avg_fps = if self.frame_times.is_empty() {
            0.0
        } else {
            let sum: f64 = self.frame_times.iter().map(|d| d.as_secs_f64()).sum();
            let mean = sum / self.frame_times.len() as f64;
            if mean > 0.0 {
                (1.0 / mean) as f32
            } else {
                0.0
            }
        };
        MetricsSnapshot {
            p50_frame: p50,
            p95_frame: p95,
            p99_frame: p99,
            avg_fps,
            last_prepare: self.last_prepare,
            last_submit: self.last_submit,
            last_heap: self.last_heap,
        }
    }

    /// One-line overlay for the dev HUD (F11).
    #[must_use]
    pub fn hud_line(&self) -> String {
        let s = self.snapshot();
        let mb = s.last_heap.map(|h| h.resident_bytes as f64 / (1024.0 * 1024.0)).unwrap_or(0.0);
        format!(
            "p50 {:.1}ms p95 {:.1}ms p99 {:.1}ms | {:.0} fps | prep {:.1}ms gpu {:.1}ms | rss {:.1}MB",
            s.p50_frame.as_secs_f64() * 1000.0,
            s.p95_frame.as_secs_f64() * 1000.0,
            s.p99_frame.as_secs_f64() * 1000.0,
            s.avg_fps,
            s.last_prepare.as_secs_f64() * 1000.0,
            s.last_submit.as_secs_f64() * 1000.0,
            mb,
        )
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn percentile_sorted(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_monotonic() {
        let mut m = MetricsCollector::new();
        for i in 1..=50u64 {
            let d = Duration::from_micros(i * 100);
            m.record_frame(d / 2, d / 2, d);
        }
        let s = m.snapshot();
        assert!(s.p99_frame >= s.p50_frame);
        assert!(s.avg_fps > 0.0);
    }
}
