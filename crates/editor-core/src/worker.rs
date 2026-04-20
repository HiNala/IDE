//! Small background worker pool for blocking work (file I/O in M06+).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam_channel::{bounded, Receiver, Sender};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Cancellation flag passed into background jobs.
#[derive(Clone, Debug)]
pub struct JobToken {
    cancelled: Arc<AtomicBool>,
}

impl JobToken {
    /// Request cooperative cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Fixed-size thread pool with a shared job queue.
pub struct WorkerPool {
    tx: Option<Sender<Job>>,
    handles: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for WorkerPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerPool")
            .field("thread_count", &self.handles.len())
            .finish_non_exhaustive()
    }
}

impl WorkerPool {
    /// `n_threads`: worker count; `None` → `num_cpus - 1` clamped to `[1, 8]`.
    #[must_use]
    pub fn new(n_threads: Option<usize>) -> Self {
        let n = n_threads.unwrap_or_else(|| num_cpus::get().saturating_sub(1).clamp(1, 8));
        let (tx, rx) = crossbeam_channel::unbounded::<Job>();
        let mut handles = Vec::with_capacity(n);
        for _ in 0..n {
            let rx = rx.clone();
            handles.push(std::thread::spawn(move || {
                while let Ok(job) = rx.recv() {
                    job();
                }
            }));
        }
        Self { tx: Some(tx), handles }
    }

    /// Enqueue `job`; receive the result on `Receiver` (blocking).
    pub fn spawn<F, T>(&self, job: F) -> (JobToken, Receiver<T>)
    where
        F: FnOnce(&JobToken) -> T + Send + 'static,
        T: Send + 'static,
    {
        let cancelled = Arc::new(AtomicBool::new(false));
        let token = JobToken { cancelled: Arc::clone(&cancelled) };
        let token_for_job = token.clone();
        let (out_tx, out_rx) = bounded(1);
        let job = Box::new(move || {
            let r = job(&token_for_job);
            let _ = out_tx.send(r);
        });
        self.tx.as_ref().expect("worker pool sender").send(job).expect("worker thread pool");
        (token, out_rx)
    }

    /// Number of worker threads.
    #[must_use]
    pub fn thread_count(&self) -> usize {
        self.handles.len()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        drop(self.tx.take());
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_runs_job() {
        let pool = WorkerPool::new(Some(1));
        let (_tok, rx) = pool.spawn(|_| 42u32);
        assert_eq!(rx.recv().expect("result"), 42);
    }

    #[test]
    fn cancellation_flag() {
        let pool = WorkerPool::new(Some(1));
        let (tok, rx) = pool.spawn(|t| {
            assert!(!t.is_cancelled());
            t.cancel();
            t.is_cancelled()
        });
        assert!(rx.recv().expect("result"));
        drop(tok);
    }
}
