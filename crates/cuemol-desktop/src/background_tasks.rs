use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct BackgroundTasks {
    tasks: Vec<Task>,
}

struct Task {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl BackgroundTasks {
    pub fn spawn(
        &mut self,
        cancel: Arc<AtomicBool>,
        action: impl FnOnce(Arc<AtomicBool>) + Send + 'static,
    ) {
        self.reap();
        let worker_cancel = cancel.clone();
        let thread = std::thread::spawn(move || action(worker_cancel));
        self.tasks.push(Task { cancel, thread });
    }

    pub fn reap(&mut self) {
        let mut index = 0;
        while index < self.tasks.len() {
            if self.tasks[index].thread.is_finished() {
                let task = self.tasks.swap_remove(index);
                let _ = task.thread.join();
            } else {
                index += 1;
            }
        }
    }
}

impl Drop for BackgroundTasks {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.cancel.store(true, Ordering::Relaxed);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while !self.tasks.is_empty() && Instant::now() < deadline {
            self.reap();
            if !self.tasks.is_empty() {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_tasks_keep_the_shared_turn_live_and_drop_waits_for_active_tasks() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut tasks = BackgroundTasks::default();
        tasks.spawn(cancel.clone(), |_| {});
        let deadline = Instant::now() + Duration::from_secs(5);
        while !tasks.tasks.is_empty() {
            tasks.reap();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!cancel.load(Ordering::Relaxed));
        let completed = (0..2)
            .map(|_| {
                let completed = Arc::new(AtomicBool::new(false));
                let worker_completed = completed.clone();
                tasks.spawn(cancel.clone(), move |cancel| {
                    while !cancel.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    worker_completed.store(true, Ordering::Relaxed);
                });
                completed
            })
            .collect::<Vec<_>>();
        drop(tasks);
        assert!(completed.iter().all(|done| done.load(Ordering::Relaxed)));
    }
}
