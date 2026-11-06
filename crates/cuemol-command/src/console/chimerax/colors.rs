use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
pub(super) struct Tables {
    pub named: BTreeMap<String, String>,
    pub elements: BTreeMap<String, String>,
}
pub(super) fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!("colors.json")).expect("ChimeraX color tables")
    })
}

#[derive(Debug, PartialEq)]
pub(super) struct Color<'a> {
    pub hex: String,
    pub alpha: Option<f64>,
    pub rest: &'a str,
}
impl Color<'_> {
    pub fn value(&self) -> String {
        match self.alpha {
            Some(alpha) => format!("{}@{alpha}", self.hex),
            None => self.hex.clone(),
        }
    }
}
fn hex(rgb: [f64; 3]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (rgb[0].clamp(0., 1.) * 255.).round() as u8,
        (rgb[1].clamp(0., 1.) * 255.).round() as u8,
        (rgb[2].clamp(0., 1.) * 255.).round() as u8
    )
}
fn scaled(number: &str, unit: &str, maximum: f64) -> Option<f64> {
    let mut value = number.parse::<f64>().ok().filter(|n| n.is_finite())?;
    if unit == "%" {
        value /= 100.;
    } else if !unit.is_empty() {
        return None;
    } else if !number.contains('.') {
        value /= maximum;
    }
    Some(value.clamp(0., 1.))
}
fn hue(number: &str, unit: &str) -> Option<f64> {
    let value = number.parse::<f64>().ok().filter(|n| n.is_finite())?;
    Some(
        value
            / match unit {
                "" | "deg" => 360.,
                "rad" => std::f64::consts::TAU,
                "grad" => 400.,
                "turn" => 1.,
                _ => return None,
            },
    )
}
fn hsl(h: f64, s: f64, l: f64) -> [f64; 3] {
    if s == 0. {
        return [l; 3];
    }
    let m2 = if l <= 0.5 {
        l * (1. + s)
    } else {
        l + s - l * s
    };
    let m1 = 2. * l - m2;
    let v = |angle: f64| {
        let x = angle.rem_euclid(1.);
        if x < 1. / 6. {
            m1 + (m2 - m1) * x * 6.
        } else if x < 0.5 {
            m2
        } else if x < 2. / 3. {
            m1 + (m2 - m1) * (2. / 3. - x) * 6.
        } else {
            m1
        }
    };
    [v(h + 1. / 3.), v(h), v(h - 1. / 3.)]
}
fn function(text: &str) -> Option<Color<'_>> {
    let m = re!(r"(?i)^(rgba|rgb|hsla|hsl|gray)\s*\(([^)]*)\)").captures(text)?;
    let space = m[1].to_ascii_lowercase();
    let numbers = m[2]
        .split(',')
        .map(|part| {
            let p = re!(r"^\s*([-+]?(?:\d+(?:\.\d*)?|\.\d+))\s*(%|deg|grad|rad|turn|)\s*$")
                .captures(part)?;
            Some((p[1].to_string(), p[2].to_string()))
        })
        .collect::<Option<Vec<_>>>()?;
    let n = |i: usize, max: f64| scaled(&numbers[i].0, &numbers[i].1, max);
    let mut alpha = None;
    let rgb = match (space.as_str(), numbers.len()) {
        ("gray", 1 | 2) => {
            let gray = n(0, 255.)?;
            if numbers.len() == 2 {
                alpha = n(1, 1.);
            }
            [gray; 3]
        }
        ("rgb", 3) | ("rgba", 4) => {
            if numbers.len() == 4 {
                alpha = n(3, 1.);
            }
            [n(0, 255.)?, n(1, 255.)?, n(2, 255.)?]
        }
        ("hsl", 3) | ("hsla", 4) => {
            if numbers.len() == 4 {
                alpha = n(3, 1.);
            }
            hsl(hue(&numbers[0].0, &numbers[0].1)?, n(1, 1.)?, n(2, 1.)?)
        }
        _ => return None,
    };
    Some(Color {
        hex: hex(rgb),
        alpha,
        rest: text[m[0].len()..].trim_start(),
    })
}
pub(super) fn read(text: &str) -> Option<Color<'_>> {
    if let Some(m) = re!(r"^#([0-9A-Fa-f]+)(?:\s|$)").captures(text) {
        let digits = &m[1];
        let width = match digits.len() {
            3 | 4 => 1,
            6 | 8 => 2,
            12 | 16 => 4,
            _ => return None,
        };
        let maximum = ((1u32 << (width * 4)) - 1) as f64;
        let values = (0..digits.len())
            .step_by(width)
            .map(|i| u32::from_str_radix(&digits[i..i + width], 16).unwrap() as f64 / maximum)
            .collect::<Vec<_>>();
        return Some(Color {
            hex: hex([values[0], values[1], values[2]]),
            alpha: values.get(3).copied(),
            rest: text[digits.len() + 1..].trim_start(),
        });
    }
    if let Some(m) = re!(r"^(\d[\d.]*),([\d.]+),([\d.]+)(?:,([\d.]+))?(?:\s|$)").captures(text) {
        let rgb = [
            m[1].parse::<f64>().ok()? / 100.,
            m[2].parse::<f64>().ok()? / 100.,
            m[3].parse::<f64>().ok()? / 100.,
        ];
        if rgb.iter().all(|n| n.is_finite()) {
            return Some(Color {
                hex: hex(rgb),
                alpha: m
                    .get(4)
                    .map(|m| m.as_str().parse::<f64>().unwrap_or(f64::NAN) / 100.),
                rest: text[m[0].len()..].trim_start(),
            });
        }
    }
    if let Some(color) = function(text) {
        return Some(color);
    }
    let mut rest = text;
    let mut parts = Vec::new();
    for _ in 0..3 {
        if rest.is_empty() {
            break;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        parts.push((&rest[..end], rest[end..].trim_start()));
        rest = parts.last().unwrap().1;
    }
    for count in (1..=parts.len()).rev() {
        let key = parts[..count]
            .iter()
            .map(|(value, _)| *value)
            .collect::<String>()
            .to_ascii_lowercase();
        if let Some(value) = tables().named.get(&key) {
            return Some(Color {
                hex: value.clone(),
                alpha: None,
                rest: parts[count - 1].1,
            });
        }
    }
    None
}
pub(super) fn split(value: &str) -> (&str, Option<f64>) {
    value
        .split_once('@')
        .map_or((value, None), |(color, alpha)| (color, alpha.parse().ok()))
}
