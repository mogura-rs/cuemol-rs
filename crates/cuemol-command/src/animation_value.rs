use cuemol_anim::{AnimMotion, AnimPropertySpec};
use cuemol_core::{CueMolError, Result, Vector4};

fn whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

pub(super) fn trim(value: &str) -> &str {
    value.trim_matches(whitespace)
}

fn number(value: &str) -> Option<f64> {
    let value = trim(value);
    let radix = match value.as_bytes().get(..2) {
        Some(b"0x" | b"0X") => Some(4),
        Some(b"0o" | b"0O") => Some(3),
        Some(b"0b" | b"0B") => Some(1),
        _ => None,
    };
    let value = if let Some(width) = radix {
        let digits = &value[2..];
        if digits.is_empty() {
            return None;
        }
        let mut significant = 0_u64;
        let mut bits = 0_i32;
        let mut guard = false;
        let mut sticky = false;
        for c in digits.chars() {
            let digit = c.to_digit(1 << width)?;
            for shift in (0..width).rev() {
                let bit = (digit >> shift) & 1;
                if bits == 0 && bit == 0 {
                    continue;
                }
                bits += 1;
                if bits <= 53 {
                    significant = (significant << 1) | u64::from(bit);
                } else if bits == 54 {
                    guard = bit != 0;
                } else {
                    sticky |= bit != 0;
                }
                if bits > 1024 {
                    return None;
                }
            }
        }
        if guard && (sticky || significant & 1 != 0) {
            significant += 1;
        }
        significant as f64 * 2_f64.powi((bits - 53).max(0))
    } else {
        value.parse::<f64>().ok()?
    };
    value.is_finite().then_some(value)
}

enum GenericValue<'a> {
    Text(&'a str),
    Number(f64),
    Boolean(bool),
}

fn generic(value: &str) -> GenericValue<'_> {
    match trim(value) {
        "true" => GenericValue::Boolean(true),
        "false" => GenericValue::Boolean(false),
        _ => number(value).map_or(GenericValue::Text(value), GenericValue::Number),
    }
}

pub(super) fn guarded(value: &str, structured: bool) -> String {
    if structured {
        return trim(value).into();
    }
    match generic(value) {
        GenericValue::Text(value) => value.into(),
        GenericValue::Boolean(value) => value.to_string(),
        GenericValue::Number(value) => ryu_js::Buffer::new().format(value).into(),
    }
}

fn boolean(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" => Some(true),
        "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

pub(super) fn property(
    motion: &AnimMotion,
    spec: &AnimPropertySpec,
    value: &str,
    structured: bool,
) -> Result<String> {
    let invalid = || {
        CueMolError::invalid_argument(format!(
            "invalid {} value for {}",
            spec.type_name, spec.name
        ))
    };
    if spec.type_name == "object<Vector>" {
        let vector = if structured {
            let values = trim(value)
                .split(whitespace)
                .filter(|part| !part.is_empty())
                .map(number)
                .collect::<Option<Vec<_>>>()
                .ok_or_else(invalid)?;
            if values.len() != 3 {
                return Err(invalid());
            }
            Vector4::new3(values[0], values[1], values[2])
        } else {
            let GenericValue::Text(text) = generic(value) else {
                return Err(invalid());
            };
            Vector4::from_script_string(text)?
        };
        if ![vector.x, vector.y, vector.z, vector.w]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(invalid());
        }
        if spec.name == "axis" {
            let AnimMotion::Spin(spin) = motion else {
                return Err(invalid());
            };
            let mut spin = spin.clone();
            spin.set_axis(vector);
            return Ok(spin.axis().script_string());
        }
        return Ok(vector.script_string());
    }
    let parsed = if structured {
        let text = trim(value);
        match spec.type_name {
            "boolean" => GenericValue::Boolean(match text {
                "1" => true,
                "0" => false,
                _ => boolean(text).ok_or_else(invalid)?,
            }),
            "real" => GenericValue::Number(number(text).ok_or_else(invalid)?),
            _ => GenericValue::Text(text),
        }
    } else {
        generic(value)
    };
    match (spec.type_name, parsed) {
        ("string", GenericValue::Text(value)) => Ok(value.into()),
        ("real", GenericValue::Number(value)) => Ok(value.to_string()),
        ("boolean", GenericValue::Boolean(value)) => Ok(value.to_string()),
        ("boolean", GenericValue::Text(value)) => {
            boolean(value).map(|v| v.to_string()).ok_or_else(invalid)
        }
        _ => Err(invalid()),
    }
}
