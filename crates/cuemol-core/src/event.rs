use std::collections::VecDeque;
use std::thread::ThreadId;

pub type TimeValue = u64;
pub type TimerId = u64;

pub struct EventManager {
    main_thread: ThreadId,
    queue: VecDeque<Box<dyn FnOnce() + Send>>,
    timers: Vec<TimerEntry>,
    next_timer_id: TimerId,
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EventManager {
    pub fn new() -> Self {
        Self {
            main_thread: std::thread::current().id(),
            queue: VecDeque::new(),
            timers: Vec::new(),
            next_timer_id: 1,
        }
    }

    pub fn is_main_thread(&self) -> bool {
        std::thread::current().id() == self.main_thread
    }

    pub fn delegate_event_fire<F>(&mut self, event: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.queue.push_back(Box::new(event));
    }

    pub fn message_loop(&mut self) {
        while let Some(event) = self.queue.pop_front() {
            event();
        }
    }

    pub fn set_timer<F>(&mut self, now: TimeValue, duration: TimeValue, callback: F) -> TimerId
    where
        F: FnMut(f64, TimeValue, bool) -> bool + Send + 'static,
    {
        let id = self.next_timer_id;
        self.next_timer_id += 1;
        self.timers.push(TimerEntry {
            id,
            start: now,
            duration,
            callback: Box::new(callback),
        });
        id
    }

    pub fn remove_timer(&mut self, id: TimerId) -> bool {
        let before = self.timers.len();
        self.timers.retain(|timer| timer.id != id);
        self.timers.len() != before
    }

    pub fn check_timer_queue(&mut self, now: TimeValue) {
        let mut remaining = Vec::new();
        for mut timer in self.timers.drain(..) {
            let elapsed = now.saturating_sub(timer.start);
            let last = timer.duration == 0 || elapsed >= timer.duration;
            let t = if timer.duration == 0 {
                1.0
            } else {
                (elapsed as f64 / timer.duration as f64).min(1.0)
            };
            if (timer.callback)(t, now, last) && !last {
                remaining.push(timer);
            }
        }
        self.timers = remaining;
    }

    pub fn fini_timer(&mut self) {
        self.timers.clear();
    }

    pub fn timer_count(&self) -> usize {
        self.timers.len()
    }
}

struct TimerEntry {
    id: TimerId,
    start: TimeValue,
    duration: TimeValue,
    callback: Box<dyn FnMut(f64, TimeValue, bool) -> bool + Send>,
}
