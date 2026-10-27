use crate::clock::{Duration, Instant};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct LThread {
    task: Option<Box<dyn FnOnce() + Send>>,
    handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl LThread {
    pub fn new<F>(task: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self {
            task: Some(Box::new(task)),
            handle: None,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn empty() -> Self {
        Self {
            task: None,
            handle: None,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn kick(&mut self) -> bool {
        if self.handle.is_some() || self.task.is_none() {
            return false;
        }
        let task = self.task.take().unwrap();
        let running = Arc::clone(&self.running);
        running.store(true, Ordering::SeqCst);
        self.handle = Some(std::thread::spawn(move || {
            task();
            running.store(false, Ordering::SeqCst);
        }));
        true
    }

    pub fn is_running(&mut self) -> bool {
        if self.running.load(Ordering::SeqCst) {
            return true;
        }
        self.join_finished();
        false
    }

    pub fn wait_termination(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn wait_termination_timeout(&mut self, seconds: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < deadline {
            if !self.running.load(Ordering::SeqCst) {
                self.join_finished();
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if !self.running.load(Ordering::SeqCst) {
            self.join_finished();
            true
        } else {
            false
        }
    }

    fn join_finished(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for LThread {
    fn drop(&mut self) {
        self.wait_termination();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn lthread_kick_wait_and_running_state_match_reference() {
        let started = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let started_for_thread = Arc::clone(&started);
        let finished_for_thread = Arc::clone(&finished);
        let mut thread = LThread::new(move || {
            started_for_thread.store(true, Ordering::SeqCst);
            finished_for_thread.store(true, Ordering::SeqCst);
        });

        assert!(!started.load(Ordering::SeqCst));
        assert!(!thread.is_running());
        assert!(thread.kick());
        thread.wait_termination();
        assert!(started.load(Ordering::SeqCst));
        assert!(finished.load(Ordering::SeqCst));
        assert!(!thread.is_running());
        assert!(!thread.is_running());
    }

    #[test]
    fn lthread_timed_wait_reports_success_and_timeout() {
        let mut thread = LThread::new(|| std::thread::sleep(Duration::from_millis(100)));
        thread.kick();
        assert!(thread.wait_termination_timeout(5));

        let mut thread = LThread::new(|| std::thread::sleep(Duration::from_millis(300)));
        thread.kick();
        assert!(!thread.wait_termination_timeout(0));
        thread.wait_termination();

        let mut never_started = LThread::empty();
        never_started.wait_termination();
        assert!(!never_started.is_running());
    }
}
