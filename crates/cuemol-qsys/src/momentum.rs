//! Momentum-scroll integration core — the pure, headless part of C++
//! `qsys::MomentumScroll`.
//!
//! After the user flings a drag, the view keeps moving/spinning for ~500 ms with
//! a decelerating "decay". The per-tick integration — how far the centre has
//! translated, or how far the orientation has spun, at a given normalized
//! progress `rho ∈ [0, 1]` — is a pure function with an exact oracle (the
//! deceleration curve + linear scaling). The glue that this core does NOT
//! include (and which belongs to the GUI/View layer): deriving the initial drag
//! velocity / trackball spin-axis from pointer events (`View::getTrackRotQuat`),
//! and the frame timer that advances `rho`.

/// Momentum easing — the quadratic ease-out `rho·(2 − rho)·0.5` C++
/// `MomentumScroll` applies to both the translation displacement and the spin
/// angle. Decelerating decay: `0` at `rho = 0`, `0.5` at `rho = 1`, monotonic
/// increasing with a falling derivative. `rho` is clamped to `[0, 1]`.
pub fn momentum_ease(rho: f64) -> f64 {
    let r = rho.clamp(0.0, 1.0);
    r * (2.0 - r) * 0.5
}

/// Translational momentum (C++ `MMS_TRANSXY`): the view-centre offset `(dx, dy)`
/// in the drag plane at progress `rho` for an initial drag velocity `(vx, vy)`.
pub fn momentum_translation(vx: f64, vy: f64, rho: f64) -> (f64, f64) {
    let f = momentum_ease(rho);
    (vx * f, vy * f)
}

/// Rotational momentum (C++ `MMS_ROTXY`): the spin angle in radians about the
/// (caller-supplied) spin axis at progress `rho` for an initial angular speed.
pub fn momentum_spin_angle(spin_ini_vel: f64, rho: f64) -> f64 {
    spin_ini_vel * momentum_ease(rho)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_is_a_decelerating_curve_from_zero_to_half() {
        assert_eq!(momentum_ease(0.0), 0.0);
        assert_eq!(momentum_ease(1.0), 0.5);
        // Clamped outside [0, 1].
        assert_eq!(momentum_ease(-1.0), 0.0);
        assert_eq!(momentum_ease(2.0), 0.5);
        // Monotonic increasing.
        let f = [0.0, 0.25, 0.5, 0.75, 1.0].map(momentum_ease);
        for w in f.windows(2) {
            assert!(w[1] > w[0], "not monotonic: {w:?}");
        }
        // Decelerating: each equal-width step advances less than the previous.
        let d: Vec<f64> = f.windows(2).map(|w| w[1] - w[0]).collect();
        for w in d.windows(2) {
            assert!(w[1] < w[0], "not decelerating: {d:?}");
        }
    }

    #[test]
    fn translation_and_spin_scale_linearly_with_eased_progress() {
        // At rho = 1 the displacement / angle is exactly half the raw velocity
        // (the ·0.5 factor), and linear in the velocity at fixed rho.
        assert_eq!(momentum_translation(10.0, -20.0, 1.0), (5.0, -10.0));
        let (dx_half, _) = momentum_translation(10.0, 0.0, 0.5);
        assert!((dx_half - 10.0 * momentum_ease(0.5)).abs() < 1.0e-12);
        assert!((momentum_spin_angle(2.0, 1.0) - 1.0).abs() < 1.0e-12);
        // Linear in the input velocity at fixed progress.
        assert!(
            (momentum_spin_angle(4.0, 0.3) - 2.0 * momentum_spin_angle(2.0, 0.3)).abs() < 1.0e-12
        );
    }
}
