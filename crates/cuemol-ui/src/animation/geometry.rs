pub const DEFAULT_SCALE: f64 = 0.1;
pub const LABEL_WIDTH: f32 = 140.;
pub const RULER_HEIGHT: f32 = 26.;
pub const LANE_HEIGHT: f32 = 26.;

pub fn clamp_scale(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.004, 4.)
    } else {
        DEFAULT_SCALE
    }
}

pub fn fit_scale(length: f64, width: f64) -> f64 {
    if length <= 0. || width <= 0. {
        DEFAULT_SCALE
    } else {
        clamp_scale((width - 40.) / length)
    }
}

pub fn timeline_width(length: f64, scale: f64) -> f64 {
    ((length * scale).ceil() + 40.).max(320.)
}

pub fn tick_step(scale: f64) -> f64 {
    const STEPS: [f64; 18] = [
        50., 100., 200., 250., 500., 1000., 2000., 2500., 5000., 10000., 15000., 30000., 60000.,
        120000., 300000., 600000., 1200000., 3600000.,
    ];
    STEPS
        .into_iter()
        .find(|step| *step >= 72. / scale.max(1e-9))
        .unwrap_or(3600000.)
}

pub fn tick_label(ms: f64, step: f64) -> String {
    let seconds = ms / 1000.;
    if step < 1000. {
        return format!("{seconds:.2}s");
    }
    if step < 10000. {
        return format!("{seconds:.1}s");
    }
    let total = seconds.round() as u64;
    if total >= 60 {
        format!("{}:{:02}", total / 60, total % 60)
    } else {
        format!("{total}s")
    }
}

pub fn clock(ms: f64) -> String {
    let value = ms.abs().round() as u64;
    format!(
        "{}{}:{:02}.{:03}",
        if ms < 0. { "-" } else { "" },
        value / 60000,
        value / 1000 % 60,
        value % 1000
    )
}

pub fn parse_clock(text: &str) -> Option<f64> {
    let text = text.trim();
    let sign = if text.starts_with('-') { -1. } else { 1. };
    let text = text.strip_prefix('-').unwrap_or(text);
    let mut total = 0.;
    for part in text.split(':') {
        let value: f64 = part.parse().ok()?;
        if !value.is_finite() || value < 0. {
            return None;
        }
        total = total * 60. + value;
    }
    let milliseconds = sign * total * 1000.;
    milliseconds.is_finite().then_some(milliseconds)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditMode {
    Move,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: f64,
    pub end: f64,
    pub abs_start: f64,
    pub abs_end: f64,
    pub relative: bool,
}

impl Span {
    pub fn shifted(self, mode: EditMode, delta: f64) -> Self {
        let minimum = -self.abs_start.min(if self.relative {
            self.start
        } else {
            f64::INFINITY
        });
        let (start_delta, end_delta) = match mode {
            EditMode::Move => {
                let d = delta.max(minimum);
                (d, d)
            }
            EditMode::Left => (delta.max(minimum).min(self.abs_end - self.abs_start), 0.),
            EditMode::Right => (0., delta.max(self.abs_start - self.abs_end)),
        };
        Self {
            start: self.start + start_delta,
            end: self.end + end_delta,
            abs_start: self.abs_start + start_delta,
            abs_end: self.abs_end + end_delta,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chained_moves_keep_offsets_nonnegative_and_resizes_keep_other_edge() {
        let span = Span {
            start: 100.,
            end: 1100.,
            abs_start: 2100.,
            abs_end: 3100.,
            relative: true,
        };
        assert_eq!(span.shifted(EditMode::Move, -900.).start, 0.);
        assert_eq!(span.shifted(EditMode::Move, -900.).abs_start, 2000.);
        let left = span.shifted(EditMode::Left, 5000.);
        assert_eq!(
            (left.start, left.end, left.abs_start, left.abs_end),
            (1100., 1100., 3100., 3100.)
        );
        let right = span.shifted(EditMode::Right, -5000.);
        assert_eq!(
            (right.start, right.end, right.abs_start, right.abs_end),
            (100., 100., 2100., 2100.)
        );
        let absolute = Span {
            relative: false,
            start: 2100.,
            end: 3100.,
            ..span
        };
        assert_eq!(absolute.shifted(EditMode::Move, -3000.).start, 0.);
    }
    #[test]
    fn clock_carries_rounded_milliseconds() {
        assert_eq!(clock(59999.6), "1:00.000");
        assert_eq!(parse_clock("1:02.125"), Some(62125.));
        assert_eq!(parse_clock("nan"), None);
        assert_eq!(parse_clock("-1"), Some(-1000.));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[test]
fn reference_geometry_matches_current_timeline() {
    use serde_json::{json, Value};
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Ok(reference) = std::env::var("CUEMOL_ANIM_REFERENCE") else {
        eprintln!("Set CUEMOL_ANIM_REFERENCE to compare the read-only reference geometry");
        return;
    };
    let cases=(0..128).map(|index|json!({"scale":([-1.,0.004,0.01,0.1,1.,4.,10.][index%7]),"length":index as f64*1789.,"width":320.+index as f64*11.,"index":index,"time":index*10371})).collect::<Vec<_>>();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("node")
        .arg(root.join("scripts/check-reference-animation-geometry.mjs"))
        .arg(reference)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start reference geometry");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&cases).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    for (case, expected) in cases.iter().zip(expected) {
        let scale = clamp_scale(case["scale"].as_f64().unwrap());
        let tick = tick_step(scale);
        for (key, value) in [
            ("scale", scale),
            (
                "fit",
                fit_scale(
                    case["length"].as_f64().unwrap(),
                    case["width"].as_f64().unwrap(),
                ),
            ),
            (
                "width",
                timeline_width(case["length"].as_f64().unwrap(), scale),
            ),
            ("tick", tick),
        ] {
            assert!(
                (expected[key].as_f64().unwrap() - value).abs() < 1e-8,
                "{key}: {case}"
            );
        }
        assert_eq!(
            expected["label"],
            tick_label(tick * case["index"].as_f64().unwrap(), tick),
            "{case}"
        );
        assert_eq!(
            expected["clock"],
            clock(case["time"].as_f64().unwrap()),
            "{case}"
        );
    }
    eprintln!("Compared {} reference timeline geometry cases", cases.len());
}
