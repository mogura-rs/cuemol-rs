//! Mouse-drag event helper — a port of C++ `qsys::MouseEventHandler`.
//!
//! Tracks a fixed-capacity ring buffer of recent drag samples and computes the
//! drag VELOCITY by a linear least-squares regression of position vs time. The
//! velocity is the pure, headless, independently-verifiable core (a constant
//! velocity in → that velocity out); the platform event delivery / frame timer
//! that feeds samples in is the GUI layer's glue.

use std::collections::VecDeque;

/// Ring-buffer capacity (C++ `EVENTBUF_SIZE`).
const EVENTBUF_SIZE: usize = 16;
/// Averaging window in milliseconds (C++ `AVER_TIME`).
const AVER_TIME_MS: i64 = 500;
/// Max gap (ms) between the last two samples before the drag is considered
/// stalled and no momentum velocity is produced.
const MAX_GAP_MS: i64 = 50;

/// One timestamped drag position sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragSample {
    pub t_ms: i64,
    pub x: f64,
    pub y: f64,
}

/// Drag state machine (C++ `m_nState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragState {
    None,
    Down,
    Drag,
}

/// Mouse-drag tracker: a ring buffer of samples + the drag state machine.
#[derive(Debug, Clone, PartialEq)]
pub struct MouseEventHandler {
    buf: VecDeque<DragSample>,
    state: DragState,
    start: (f64, f64),
    prev: (f64, f64),
}

impl Default for MouseEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseEventHandler {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::with_capacity(EVENTBUF_SIZE),
            state: DragState::None,
            start: (0.0, 0.0),
            prev: (0.0, 0.0),
        }
    }

    pub fn displacement(&self, x: f64, y: f64) -> (f64, f64) {
        (x - self.start.0, y - self.start.1)
    }

    pub fn state(&self) -> DragState {
        self.state
    }

    fn push(&mut self, s: DragSample) {
        if self.buf.len() == EVENTBUF_SIZE {
            self.buf.pop_front();
        }
        self.buf.push_back(s);
    }

    pub fn button_down(&mut self, _t_ms: i64, x: f64, y: f64) {
        if self.state != DragState::None {
            return;
        }
        self.buf.clear();
        self.state = DragState::Down;
        self.start = (x, y);
        self.prev = (x, y);
    }

    pub fn move_to(&mut self, t_ms: i64, x: f64, y: f64) -> (f64, f64) {
        if self.state == DragState::None {
            return (0.0, 0.0);
        }
        if self.state == DragState::Down {
            if (x - self.start.0).abs() < 2.0 && (y - self.start.1).abs() < 2.0 {
                return (0.0, 0.0);
            }
            self.state = DragState::Drag;
            self.prev = (x, y);
            self.buf.clear();
            self.push(DragSample { t_ms, x, y });
            return (0.0, 0.0);
        }
        let delta = (x - self.prev.0, y - self.prev.1);
        self.prev = (x, y);
        self.push(DragSample { t_ms, x, y });
        delta
    }

    pub fn release(&mut self, t_ms: i64, x: f64, y: f64) -> (DragState, Option<(f64, f64)>) {
        if self.state == DragState::Drag {
            self.push(DragSample { t_ms, x, y });
        }
        let velocity = self.velocity(t_ms);
        let state = self.button_up();
        self.buf.clear();
        (state, velocity)
    }

    /// Button released: returns the state at release and resets to `None`.
    pub fn button_up(&mut self) -> DragState {
        let prev = self.state;
        self.state = DragState::None;
        prev
    }

    /// Drag velocity in units/second at time `now_ms`, or `None` when it cannot
    /// be computed (too few samples, stalled, or no spread in time).
    pub fn velocity(&self, now_ms: i64) -> Option<(f64, f64)> {
        calc_velocity(&self.buf, now_ms)
    }
}

/// Linear least-squares drag velocity — the pure port of C++
/// `MouseEventHandler::calcVelocity`. The velocity is the regression slope of
/// position vs time over the samples within `AVER_TIME_MS` of `now_ms`, scaled
/// ms -> sec (×1000). Returns `None` when there are fewer than 3 samples, the
/// last gap exceeds `MAX_GAP_MS`, the pointer barely moved, or the times have no
/// spread.
pub fn calc_velocity(buf: &VecDeque<DragSample>, now_ms: i64) -> Option<(f64, f64)> {
    // Guards (C++: size>=3, last gap <= 50ms, last step moved > 1px).
    let n = buf.len();
    if n < 3 {
        return None;
    }
    let t0 = buf[n - 1];
    let t1 = buf[n - 2];
    let t2 = buf[n - 3];
    if t0.t_ms - t1.t_ms > MAX_GAP_MS {
        return None;
    }
    if (t1.x - t2.x).abs() + (t1.y - t2.y).abs() <= 1.0 {
        return None;
    }

    // Means over the averaging window.
    let lo = now_ms - AVER_TIME_MS;
    let mut nave = 0_u32;
    let (mut avex, mut avey, mut avet) = (0.0, 0.0, 0.0);
    for s in buf.iter().filter(|s| s.t_ms > lo) {
        avex += s.x;
        avey += s.y;
        avet += (s.t_ms - now_ms) as f64;
        nave += 1;
    }
    if nave <= 1 {
        return None;
    }
    let nave = nave as f64;
    avex /= nave;
    avey /= nave;
    avet /= nave;

    // Regression slopes: vx = Σ(x-x̄)(t-t̄) / Σ(t-t̄)².
    let (mut tvar, mut xtvar, mut ytvar) = (0.0, 0.0, 0.0);
    for s in buf.iter().filter(|s| s.t_ms > lo) {
        let dt = (s.t_ms - now_ms) as f64 - avet;
        tvar += dt * dt;
        xtvar += (s.x - avex) * dt;
        ytvar += (s.y - avey) * dt;
    }
    if tvar.abs() < 1.0e-6 {
        return None;
    }
    Some((xtvar / tvar * 1000.0, ytvar / tvar * 1000.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf_from(samples: &[(i64, f64, f64)]) -> VecDeque<DragSample> {
        samples
            .iter()
            .map(|&(t_ms, x, y)| DragSample { t_ms, x, y })
            .collect()
    }

    #[test]
    fn velocity_recovers_a_constant_drag_speed() {
        // Constant velocity: x = 0.1·t, y = -0.2·t (px per ms). The regression
        // slope ×1000 must recover 100 / -200 px per second exactly.
        let samples: Vec<(i64, f64, f64)> = (0..=6)
            .map(|k| {
                let t = k * 16;
                (t, 0.1 * t as f64, -0.2 * t as f64)
            })
            .collect();
        let buf = buf_from(&samples);
        let (vx, vy) = calc_velocity(&buf, 96).expect("velocity");
        assert!((vx - 100.0).abs() < 1.0e-6, "vx={vx}");
        assert!((vy + 200.0).abs() < 1.0e-6, "vy={vy}");
    }

    #[test]
    fn velocity_is_none_for_too_few_or_stalled_samples() {
        // Fewer than 3 samples.
        assert_eq!(calc_velocity(&buf_from(&[(0, 0.0, 0.0)]), 0), None);
        // Last gap too large (200ms > 50ms).
        let stalled = buf_from(&[(0, 0.0, 0.0), (10, 5.0, 5.0), (210, 6.0, 6.0)]);
        assert_eq!(calc_velocity(&stalled, 210), None);
        // Last step barely moved (<= 1px total).
        let frozen = buf_from(&[(0, 0.0, 0.0), (16, 0.4, 0.3), (32, 0.5, 0.4)]);
        assert_eq!(calc_velocity(&frozen, 32), None);
    }

    #[test]
    fn handler_tracks_drag_state_and_ring_buffer_capacity() {
        let mut h = MouseEventHandler::new();
        assert_eq!(h.state(), DragState::None);
        h.button_down(0, 100.0, 100.0);
        assert_eq!(h.state(), DragState::Down);
        // A sub-2px move stays in Down (start range).
        assert_eq!(h.move_to(10, 101.0, 101.0), (0.0, 0.0));
        assert_eq!(h.state(), DragState::Down);
        // The first move starts dragging without changing the camera.
        let d = h.move_to(20, 110.0, 105.0);
        assert_eq!(d, (0.0, 0.0));
        assert_eq!(h.state(), DragState::Drag);
        // The ring buffer never exceeds its capacity.
        for k in 0..40 {
            h.move_to(30 + k, 110.0 + k as f64 * 3.0, 105.0);
        }
        assert!(h.buf.len() <= EVENTBUF_SIZE);
        assert_eq!(h.button_up(), DragState::Drag);
        assert_eq!(h.state(), DragState::None);
    }
}
