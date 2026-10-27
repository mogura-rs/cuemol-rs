use crate::clock::Instant;
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::ThreadId;

pub use crate::time::TimeValue;
pub type TimerId = u64;
pub type ListenerId = u64;
pub type SharedEventListener<E> = Arc<Mutex<Box<dyn FnMut(&mut E) + Send + 'static>>>;

pub fn shared_event_listener<E, F>(listener: F) -> SharedEventListener<E>
where
    F: FnMut(&mut E) + Send + 'static,
{
    Arc::new(Mutex::new(Box::new(listener)))
}

pub trait TimerListener: Send {
    fn on_timer(&mut self, progress: f64, current: TimeValue, last: bool) -> bool;
}

pub type SharedTimerListener = Arc<Mutex<Box<dyn TimerListener>>>;

pub fn shared_timer_listener<T>(listener: T) -> SharedTimerListener
where
    T: TimerListener + 'static,
{
    Arc::new(Mutex::new(Box::new(listener)))
}

pub trait TimerClock: Send {
    fn current_time(&self) -> TimeValue;
    fn start(&mut self, _period: TimeValue) {}
    fn stop(&mut self) {}
}

struct SystemTimerClock {
    started: Instant,
}

impl SystemTimerClock {
    fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl TimerClock for SystemTimerClock {
    fn current_time(&self) -> TimeValue {
        self.started.elapsed().as_nanos().min(i64::MAX as u128) as i64
    }
}

pub struct EventCaster<E> {
    state: Mutex<EventCasterState<E>>,
}

impl<E> fmt::Debug for EventCaster<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = lock_unpoison(&self.state);
        formatter
            .debug_struct("EventCaster")
            .field("locked", &state.locked)
            .field("next_listener_id", &state.next_listener_id)
            .field(
                "listener_ids",
                &state
                    .listeners
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl<E> PartialEq for EventCaster<E> {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        let left = lock_unpoison(&self.state);
        let right = lock_unpoison(&other.state);
        left.locked == right.locked
            && left.next_listener_id == right.next_listener_id
            && left.listeners.len() == right.listeners.len()
            && left.listeners.iter().zip(&right.listeners).all(
                |((left_id, left_listener), (right_id, right_listener))| {
                    left_id == right_id && Arc::ptr_eq(left_listener, right_listener)
                },
            )
    }
}

impl<E> Eq for EventCaster<E> {}

struct EventCasterState<E> {
    locked: bool,
    next_listener_id: ListenerId,
    listeners: Vec<(ListenerId, SharedEventListener<E>)>,
}

impl<E> Default for EventCaster<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> EventCaster<E> {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(EventCasterState {
                locked: false,
                next_listener_id: 0,
                listeners: Vec::new(),
            }),
        }
    }

    pub fn is_locked(&self) -> bool {
        lock_unpoison(&self.state).locked
    }

    pub fn add(&self, listener: SharedEventListener<E>) -> Option<ListenerId> {
        let mut state = lock_unpoison(&self.state);
        if state.locked
            || state
                .listeners
                .iter()
                .any(|(_, current)| Arc::ptr_eq(current, &listener))
        {
            return None;
        }
        let id = state.next_listener_id;
        state.next_listener_id += 1;
        state.listeners.push((id, listener));
        Some(id)
    }

    pub fn is_registered(&self, id: ListenerId) -> bool {
        lock_unpoison(&self.state)
            .listeners
            .iter()
            .any(|(current, _)| *current == id)
    }

    pub fn is_listener_registered(&self, listener: &SharedEventListener<E>) -> bool {
        lock_unpoison(&self.state)
            .listeners
            .iter()
            .any(|(_, current)| Arc::ptr_eq(current, listener))
    }

    pub fn remove(&self, id: ListenerId) -> Option<SharedEventListener<E>> {
        let mut state = lock_unpoison(&self.state);
        if state.locked {
            return None;
        }
        let index = state
            .listeners
            .iter()
            .position(|(current, _)| *current == id)?;
        Some(state.listeners.remove(index).1)
    }

    pub fn remove_listener(&self, listener: &SharedEventListener<E>) -> bool {
        let mut state = lock_unpoison(&self.state);
        if state.locked {
            return false;
        }
        let Some(index) = state
            .listeners
            .iter()
            .position(|(_, current)| Arc::ptr_eq(current, listener))
        else {
            return false;
        };
        state.listeners.remove(index);
        true
    }

    pub fn clear(&self) -> bool {
        let mut state = lock_unpoison(&self.state);
        if state.locked {
            return false;
        }
        state.listeners.clear();
        true
    }

    pub fn is_empty(&self) -> bool {
        lock_unpoison(&self.state).listeners.is_empty()
    }

    pub fn listener_count(&self) -> usize {
        lock_unpoison(&self.state).listeners.len()
    }

    pub fn fire(&self, event: &mut E) {
        self.fire_listeners(event, self.listener_snapshot());
    }

    pub fn locked_fire(&self, event: &mut E) -> bool {
        let Some(_guard) = self.cast_lock() else {
            return false;
        };
        self.fire(event);
        true
    }

    pub fn replica_fire(&self, event: &mut E) -> bool {
        let Some(guard) = self.cast_lock() else {
            return false;
        };
        let listeners = self.listener_snapshot();
        drop(guard);
        self.fire_listeners(event, listeners);
        true
    }

    fn listener_snapshot(&self) -> Vec<SharedEventListener<E>> {
        lock_unpoison(&self.state)
            .listeners
            .iter()
            .map(|(_, listener)| Arc::clone(listener))
            .collect()
    }

    fn fire_listeners(&self, event: &mut E, listeners: Vec<SharedEventListener<E>>) {
        for listener in listeners {
            let mut callback = lock_unpoison(&listener);
            callback(event);
        }
    }

    fn cast_lock(&self) -> Option<EventCastGuard<'_, E>> {
        let mut state = lock_unpoison(&self.state);
        if state.locked {
            return None;
        }
        state.locked = true;
        Some(EventCastGuard { caster: self })
    }
}

struct EventCastGuard<'a, E> {
    caster: &'a EventCaster<E>,
}

impl<E> Drop for EventCastGuard<'_, E> {
    fn drop(&mut self) {
        lock_unpoison(&self.caster.state).locked = false;
    }
}

#[derive(Clone)]
pub struct MthrEventCaster<E> {
    caster: Arc<EventCaster<E>>,
}

impl<E> Default for MthrEventCaster<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> MthrEventCaster<E> {
    pub fn new() -> Self {
        Self {
            caster: Arc::new(EventCaster::new()),
        }
    }

    pub fn caster(&self) -> &Arc<EventCaster<E>> {
        &self.caster
    }

    pub fn add(&self, listener: SharedEventListener<E>) -> Option<ListenerId> {
        self.caster.add(listener)
    }

    pub fn remove(&self, id: ListenerId) -> Option<SharedEventListener<E>> {
        self.caster.remove(id)
    }

    pub fn listener_count(&self) -> usize {
        self.caster.listener_count()
    }
}

impl<E: Clone + Send + 'static> MthrEventCaster<E> {
    pub fn locked_fire(&self, event: &mut E, manager: &mut EventManager) -> bool {
        if manager.is_main_thread() {
            return self.caster.locked_fire(event);
        }

        let caster = Arc::clone(&self.caster);
        let mut cloned_event = event.clone();
        manager.delegate_event_fire(move || {
            caster.locked_fire(&mut cloned_event);
        });
        true
    }
}

fn lock_unpoison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub struct EventManager {
    main_thread: ThreadId,
    queue: VecDeque<Box<dyn FnOnce() + Send>>,
    idle_tasks: VecDeque<Box<dyn FnMut() + Send>>,
    timers: Vec<TimerEntry>,
    listener_timers: Vec<ListenerTimerEntry>,
    next_timer_id: TimerId,
    timer_clock: Box<dyn TimerClock>,
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
            idle_tasks: VecDeque::new(),
            timers: Vec::new(),
            listener_timers: Vec::new(),
            next_timer_id: 1,
            timer_clock: Box::new(SystemTimerClock::new()),
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

    pub fn add_idle_task<F>(&mut self, task: F, last: bool)
    where
        F: FnMut() + Send + 'static,
    {
        if last {
            self.idle_tasks.push_back(Box::new(task));
        } else {
            self.idle_tasks.push_front(Box::new(task));
        }
    }

    pub fn perform_idle_tasks(&mut self) {
        self.message_loop();
        let now = self.current_time();
        self.check_timer_queue(now);
        self.check_listener_timer_queue_at(now);
        for task in &mut self.idle_tasks {
            task();
        }
    }

    pub fn idle_task_count(&self) -> usize {
        self.idle_tasks.len()
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

    pub fn init_timer_clock(&mut self, mut clock: Box<dyn TimerClock>) {
        self.timer_clock.stop();
        clock.start(0);
        self.timer_clock = clock;
    }

    pub fn current_time(&self) -> TimeValue {
        self.timer_clock.current_time()
    }

    pub fn set_timer_listener(&mut self, listener: SharedTimerListener, duration: TimeValue) {
        let start = self.current_time();
        self.listener_timers.push(ListenerTimerEntry {
            start,
            end: start.saturating_add(duration),
            listener,
        });
    }

    pub fn remove_timer_listener(&mut self, listener: &SharedTimerListener) -> bool {
        let before = self.listener_timers.len();
        self.listener_timers
            .retain(|timer| !Arc::ptr_eq(&timer.listener, listener));
        self.listener_timers.len() != before
    }

    pub fn check_listener_timer_queue(&mut self) {
        self.check_listener_timer_queue_at(self.current_time());
    }

    pub fn check_listener_timer_queue_at(&mut self, current: TimeValue) {
        let mut remaining = Vec::new();
        for timer in self.listener_timers.drain(..) {
            let mut listener = lock_unpoison(&timer.listener);
            if timer.end.saturating_sub(current) <= 0 {
                listener.on_timer(1.0, current, true);
                continue;
            }
            let duration = timer.end.saturating_sub(timer.start);
            let progress = if duration == 0 {
                1.0
            } else {
                current.saturating_sub(timer.start) as f64 / duration as f64
            };
            if listener.on_timer(progress, current, false) {
                drop(listener);
                remaining.push(timer);
            }
        }
        self.listener_timers = remaining;
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
        self.listener_timers.clear();
        self.timer_clock.stop();
    }

    pub fn timer_count(&self) -> usize {
        self.timers.len() + self.listener_timers.len()
    }
}

struct TimerEntry {
    id: TimerId,
    start: TimeValue,
    duration: TimeValue,
    callback: Box<dyn FnMut(f64, TimeValue, bool) -> bool + Send>,
}

struct ListenerTimerEntry {
    start: TimeValue,
    end: TimeValue,
    listener: SharedTimerListener,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

    struct MockClock {
        now: Arc<AtomicI64>,
    }

    impl TimerClock for MockClock {
        fn current_time(&self) -> TimeValue {
            self.now.load(Ordering::Relaxed)
        }
    }

    struct RecordingTimerListener {
        calls: Arc<Mutex<Vec<(f64, TimeValue, bool)>>>,
        keep: bool,
    }

    impl TimerListener for RecordingTimerListener {
        fn on_timer(&mut self, progress: f64, current: TimeValue, last: bool) -> bool {
            lock_unpoison(&self.calls).push((progress, current, last));
            self.keep
        }
    }

    #[test]
    fn event_caster_preserves_ids_order_and_duplicate_rules() {
        let caster = EventCaster::new();
        let first = shared_event_listener(|event: &mut Vec<i32>| event.push(1));
        let second = shared_event_listener(|event: &mut Vec<i32>| event.push(2));
        let first_id = caster.add(Arc::clone(&first)).unwrap();
        let second_id = caster.add(Arc::clone(&second)).unwrap();

        assert_eq!(first_id, 0);
        assert_eq!(second_id, 1);
        assert!(caster.add(Arc::clone(&first)).is_none());
        assert!(caster.is_registered(first_id));
        assert!(caster.is_listener_registered(&second));

        let mut event = Vec::new();
        assert!(caster.locked_fire(&mut event));
        assert_eq!(event, vec![1, 2]);
        assert!(caster.remove(first_id).is_some());
        assert!(caster.remove_listener(&second));
        assert!(caster.is_empty());
    }

    #[test]
    fn event_caster_blocks_mutation_during_locked_fire() {
        let caster = Arc::new(EventCaster::new());
        let mutation_failed = Arc::new(AtomicBool::new(false));
        let callback_caster = Arc::clone(&caster);
        let callback_result = Arc::clone(&mutation_failed);
        caster
            .add(shared_event_listener(move |_: &mut ()| {
                callback_result.store(!callback_caster.clear(), Ordering::Relaxed);
            }))
            .unwrap();

        assert!(caster.locked_fire(&mut ()));
        assert!(mutation_failed.load(Ordering::Relaxed));
        assert!(!caster.is_locked());
        assert_eq!(caster.listener_count(), 1);
    }

    #[test]
    fn replica_fire_uses_snapshot_after_unlocking() {
        let caster = Arc::new(EventCaster::new());
        let callback_caster = Arc::clone(&caster);
        caster
            .add(shared_event_listener(move |event: &mut usize| {
                *event += 1;
                callback_caster.clear();
            }))
            .unwrap();

        let mut event = 0;
        assert!(caster.replica_fire(&mut event));
        assert_eq!(event, 1);
        assert!(caster.is_empty());
    }

    #[test]
    fn multithread_caster_delegates_a_clone_to_the_main_thread() {
        let mut manager = EventManager::new();
        let caster = MthrEventCaster::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let callback_events = Arc::clone(&events);
        caster
            .add(shared_event_listener(move |event: &mut String| {
                lock_unpoison(&callback_events).push(event.clone());
            }))
            .unwrap();

        let worker_caster = caster.clone();
        let worker_events = Arc::clone(&events);
        manager = std::thread::spawn(move || {
            let mut event = "background".to_string();
            assert!(worker_caster.locked_fire(&mut event, &mut manager));
            assert!(lock_unpoison(&worker_events).is_empty());
            manager
        })
        .join()
        .unwrap();

        manager.message_loop();
        assert_eq!(lock_unpoison(&events).as_slice(), &["background"]);
    }

    #[test]
    fn idle_tasks_follow_front_and_back_registration_order() {
        let mut manager = EventManager::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        for (value, last) in [(1, true), (2, false), (3, true)] {
            let task_calls = Arc::clone(&calls);
            manager.add_idle_task(move || lock_unpoison(&task_calls).push(value), last);
        }

        assert_eq!(manager.idle_task_count(), 3);
        manager.perform_idle_tasks();
        assert_eq!(lock_unpoison(&calls).as_slice(), &[2, 1, 3]);
    }

    #[test]
    fn listener_timer_reports_progress_final_event_and_removes_itself() {
        let now = Arc::new(AtomicI64::new(0));
        let mut manager = EventManager::new();
        manager.init_timer_clock(Box::new(MockClock {
            now: Arc::clone(&now),
        }));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let listener = shared_timer_listener(RecordingTimerListener {
            calls: Arc::clone(&calls),
            keep: true,
        });
        manager.set_timer_listener(Arc::clone(&listener), 1000);

        now.store(500, Ordering::Relaxed);
        manager.check_listener_timer_queue();
        assert_eq!(manager.timer_count(), 1);
        now.store(1500, Ordering::Relaxed);
        manager.check_listener_timer_queue();
        assert_eq!(manager.timer_count(), 0);

        let calls = lock_unpoison(&calls);
        assert_eq!(calls.len(), 2);
        assert!((calls[0].0 - 0.5).abs() < 1.0e-12);
        assert_eq!(calls[0], (0.5, 500, false));
        assert_eq!(calls[1], (1.0, 1500, true));
    }

    #[test]
    fn listener_timer_can_cancel_and_remove_all_matching_registrations() {
        let mut manager = EventManager::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let canceling = shared_timer_listener(RecordingTimerListener {
            calls: Arc::clone(&calls),
            keep: false,
        });
        manager.set_timer_listener(Arc::clone(&canceling), 1000);
        manager.check_listener_timer_queue_at(500);
        assert_eq!(manager.timer_count(), 0);
        assert_eq!(lock_unpoison(&calls).len(), 1);

        manager.set_timer_listener(Arc::clone(&canceling), 1000);
        manager.set_timer_listener(Arc::clone(&canceling), 2000);
        assert_eq!(manager.timer_count(), 2);
        assert!(manager.remove_timer_listener(&canceling));
        assert_eq!(manager.timer_count(), 0);
        assert!(!manager.remove_timer_listener(&canceling));
    }

    #[test]
    fn idle_cycle_processes_events_timers_then_idle_tasks() {
        let now = Arc::new(AtomicI64::new(10));
        let mut manager = EventManager::new();
        manager.init_timer_clock(Box::new(MockClock {
            now: Arc::clone(&now),
        }));
        let order = Arc::new(Mutex::new(Vec::new()));
        let event_order = Arc::clone(&order);
        manager.delegate_event_fire(move || lock_unpoison(&event_order).push("event"));
        let timer_order = Arc::clone(&order);
        manager.set_timer_listener(
            shared_timer_listener(RecordingOrderTimer { order: timer_order }),
            0,
        );
        let idle_order = Arc::clone(&order);
        manager.add_idle_task(move || lock_unpoison(&idle_order).push("idle"), true);

        manager.perform_idle_tasks();
        assert_eq!(
            lock_unpoison(&order).as_slice(),
            &["event", "timer", "idle"]
        );
    }

    struct RecordingOrderTimer {
        order: Arc<Mutex<Vec<&'static str>>>,
    }

    impl TimerListener for RecordingOrderTimer {
        fn on_timer(&mut self, _progress: f64, _current: TimeValue, _last: bool) -> bool {
            lock_unpoison(&self.order).push("timer");
            true
        }
    }
}
