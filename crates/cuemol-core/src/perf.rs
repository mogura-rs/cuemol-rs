use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const PM_IDLE_TIMER: i32 = 0;
pub const PM_DRAW_SCENE: i32 = 1;
pub const PM_LABEL_RENDER: i32 = 2;
pub const PM_RENDER_SCENE: i32 = 3;
const AVERAGE_SIZE: usize = 10;

#[derive(Debug)]
pub struct PerfMeasManager {
    samples: VecDeque<Duration>,
    start: Option<Instant>,
    active_timer_id: Option<i32>,
    last_average: Option<Duration>,
}

impl Default for PerfMeasManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfMeasManager {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(AVERAGE_SIZE),
            start: None,
            active_timer_id: None,
            last_average: None,
        }
    }
    pub fn enable(&mut self, timer_id: i32) {
        self.active_timer_id = Some(timer_id);
    }
    pub fn disable(&mut self) {
        self.active_timer_id = None;
        self.start = None;
    }
    pub fn active_timer_id(&self) -> Option<i32> {
        self.active_timer_id
    }
    pub fn start(&mut self, timer_id: i32) {
        if self.active_timer_id == Some(timer_id) {
            self.start = Some(Instant::now());
        }
    }
    pub fn end(&mut self, timer_id: i32) -> Option<Duration> {
        if self.active_timer_id != Some(timer_id) {
            return None;
        }
        let elapsed = self.start.take()?.elapsed();
        if self.samples.len() == AVERAGE_SIZE {
            self.samples.pop_front();
        }
        self.samples.push_back(elapsed);
        if self.samples.len() == AVERAGE_SIZE {
            self.last_average = self.average();
        }
        Some(elapsed)
    }
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
    pub fn average(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let nanos =
            self.samples.iter().map(Duration::as_nanos).sum::<u128>() / self.samples.len() as u128;
        Some(Duration::new(
            (nanos / 1_000_000_000).min(u128::from(u64::MAX)) as u64,
            (nanos % 1_000_000_000) as u32,
        ))
    }
    pub fn last_completed_average(&self) -> Option<Duration> {
        self.last_average
    }
    pub fn frames_per_second(&self) -> Option<f64> {
        let seconds = self.average()?.as_secs_f64();
        (seconds > 0.0).then_some(1.0 / seconds)
    }
}

pub fn global_perf_manager() -> &'static Mutex<PerfMeasManager> {
    static MANAGER: OnceLock<Mutex<PerfMeasManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(PerfMeasManager::new()))
}

#[derive(Debug)]
pub struct AutoPerfMeas {
    timer_id: i32,
    ended: bool,
}
impl AutoPerfMeas {
    pub fn new(timer_id: i32) -> Self {
        global_perf_manager()
            .lock()
            .expect("performance manager lock poisoned")
            .start(timer_id);
        Self {
            timer_id,
            ended: false,
        }
    }
    pub fn finish(&mut self) -> Option<Duration> {
        if self.ended {
            return None;
        }
        self.ended = true;
        global_perf_manager()
            .lock()
            .expect("performance manager lock poisoned")
            .end(self.timer_id)
    }
}
impl Drop for AutoPerfMeas {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[derive(Debug)]
pub struct AutoTimeMeas {
    message: String,
    start: Instant,
    finished: Option<Duration>,
}
impl AutoTimeMeas {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            start: Instant::now(),
            finished: None,
        }
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn elapsed(&self) -> Duration {
        self.finished.unwrap_or_else(|| self.start.elapsed())
    }
    pub fn finish(&mut self) -> Duration {
        *self.finished.get_or_insert_with(|| self.start.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manager_records_only_enabled_timer() {
        let mut manager = PerfMeasManager::new();
        manager.enable(PM_DRAW_SCENE);
        manager.start(PM_RENDER_SCENE);
        assert_eq!(manager.end(PM_RENDER_SCENE), None);
        manager.start(PM_DRAW_SCENE);
        std::hint::spin_loop();
        assert!(manager.end(PM_DRAW_SCENE).is_some());
        assert_eq!(manager.sample_count(), 1);
        assert_eq!(manager.end(PM_DRAW_SCENE), None);
    }
    #[test]
    fn ten_samples_publish_average_and_fps() {
        let mut manager = PerfMeasManager::new();
        manager.enable(PM_IDLE_TIMER);
        for _ in 0..AVERAGE_SIZE {
            manager.start(PM_IDLE_TIMER);
            std::hint::spin_loop();
            manager.end(PM_IDLE_TIMER).unwrap();
        }
        assert!(manager.last_completed_average().is_some());
        assert!(manager.frames_per_second().is_some_and(f64::is_finite));
    }
    #[test]
    fn auto_time_finish_is_idempotent() {
        let mut timer = AutoTimeMeas::new("operation");
        let first = timer.finish();
        assert_eq!(first, timer.finish());
        assert_eq!(timer.message(), "operation");
    }
}
