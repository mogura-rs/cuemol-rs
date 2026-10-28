use crate::{Atom, MolCoord};
use cuemol_core::{CueMolError, Result};

#[derive(Debug, Clone)]
pub struct LabelFormat(Vec<Segment>);

#[derive(Debug, Clone)]
enum Segment {
    Literal(String),
    Field(String, String),
}

#[derive(Clone, Copy)]
enum Value<'a> {
    Text(&'a str),
    Integer(i32),
    Real(f64),
}

#[derive(Default)]
struct Spec {
    fill: Option<char>,
    align: Option<char>,
    sign: Option<char>,
    alternate: bool,
    zero: bool,
    localized: bool,
    width: usize,
    precision: Option<usize>,
    kind: Option<char>,
}

impl LabelFormat {
    pub fn new(format: &str) -> Result<Self> {
        let mut segments = Vec::new();
        let mut literal = String::new();
        let mut chars = format.char_indices().peekable();
        while let Some((position, ch)) = chars.next() {
            if matches!(ch, '{' | '}') && chars.peek().is_some_and(|(_, next)| *next == ch) {
                chars.next();
                literal.push(ch);
            } else if ch == '}' {
                return Err(format_error(format, position, "unmatched closing brace"));
            } else if ch != '{' {
                literal.push(ch);
            } else {
                let start = position + 1;
                let mut end = None;
                for (position, ch) in chars.by_ref() {
                    if ch == '{' {
                        return Err(format_error(format, position, "nested field"));
                    }
                    if ch == '}' {
                        end = Some(position);
                        break;
                    }
                }
                let end = end.ok_or_else(|| format_error(format, position, "unclosed field"))?;
                let body = &format[start..end];
                let (field, spec) = body.split_once(':').unwrap_or((body, ""));
                let sample = match field {
                    "name" | "resn" | "resi" | "chain" | "elem" | "alt" | "molname" => {
                        Some(Value::Text(""))
                    }
                    "bfac" | "occ" => Some(Value::Real(0.)),
                    "aid" => Some(Value::Integer(0)),
                    name if name.starts_with("rprop.") && name.len() > 6 => Some(Value::Text("")),
                    name if name.starts_with("aprop.") && name.len() > 6 => None,
                    _ => {
                        return Err(format_error(
                            format,
                            position,
                            &format!("unknown field {field}"),
                        ))
                    }
                };
                if let Some(sample) = sample {
                    format_value(spec, sample).map_err(|error| {
                        format_error(format, position, &format!("{body}: {error}"))
                    })?;
                }
                if !literal.is_empty() {
                    segments.push(Segment::Literal(std::mem::take(&mut literal)));
                }
                segments.push(Segment::Field(field.into(), spec.into()));
            }
        }
        if !literal.is_empty() {
            segments.push(Segment::Literal(literal));
        }
        Ok(Self(segments))
    }

    pub fn apply(&self, atom: &Atom, molecule: &MolCoord, molecule_name: &str) -> String {
        let residue = residue_index(atom);
        let alt = atom
            .alt_loc
            .filter(|ch| *ch != ' ' && *ch != '\0')
            .map(|ch| ch.to_string())
            .unwrap_or_default();
        let mut output = String::new();
        for segment in &self.0 {
            let Segment::Field(field, spec) = segment else {
                if let Segment::Literal(text) = segment {
                    output.push_str(text);
                }
                continue;
            };
            let value = match field.as_str() {
                "name" => Some(Value::Text(atom.name.trim())),
                "resn" => Some(Value::Text(&atom.res_name)),
                "resi" => Some(Value::Text(&residue)),
                "chain" => Some(Value::Text(&atom.chain_id)),
                "elem" => Some(Value::Text(&atom.element)),
                "alt" => Some(Value::Text(&alt)),
                "molname" => Some(Value::Text(molecule_name)),
                "bfac" => Some(Value::Real(f64::from(atom.b_factor))),
                "occ" => Some(Value::Real(f64::from(atom.occupancy))),
                "aid" => Some(Value::Integer(atom.serial)),
                name if name.starts_with("aprop.") => atom
                    .int_prop(&name[6..])
                    .map(Value::Integer)
                    .or_else(|| atom.real_prop(&name[6..]).map(Value::Real)),
                name if name.starts_with("rprop.") => molecule
                    .get_residue_prop_str(&atom.chain_id, &residue, &name[6..])
                    .ok()
                    .flatten()
                    .map(Value::Text),
                _ => None,
            };
            if let Some(value) = value {
                output.push_str(&format_value(spec, value).unwrap_or_else(|_| "?".into()));
            }
        }
        output
    }
}

pub fn builtin_label(atom: &Atom) -> String {
    let mut label = format!(
        "{} {}{} {}",
        atom.chain_id,
        atom.res_name,
        residue_index(atom),
        atom.name.trim()
    );
    if let Some(alt) = atom.alt_loc.filter(|ch| *ch != '\0' && *ch != ' ') {
        label.push(':');
        label.push(alt);
    }
    label
}

fn residue_index(atom: &Atom) -> String {
    let mut index = atom.res_id.to_string();
    if let Some(insertion) = atom.ins_code.filter(|ch| *ch != '\0' && *ch != ' ') {
        index.push(insertion);
    }
    index
}

fn format_error(format: &str, position: usize, reason: &str) -> CueMolError {
    CueMolError::invalid_argument(format!("label format {format:?}: {reason} (at {position})"))
}

fn parse_spec(text: &str) -> std::result::Result<Spec, &'static str> {
    let mut spec = Spec::default();
    let chars = text.chars().collect::<Vec<_>>();
    let mut i = 0;
    if chars.get(1).is_some_and(|ch| matches!(ch, '<' | '>' | '^')) {
        spec.fill = Some(chars[0]);
        spec.align = Some(chars[1]);
        i = 2;
    } else if chars
        .first()
        .is_some_and(|ch| matches!(ch, '<' | '>' | '^'))
    {
        spec.align = Some(chars[0]);
        i = 1;
    }
    if chars.get(i).is_some_and(|ch| matches!(ch, '+' | '-' | ' ')) {
        spec.sign = Some(chars[i]);
        i += 1;
    }
    if chars.get(i) == Some(&'#') {
        spec.alternate = true;
        i += 1;
    }
    if chars.get(i) == Some(&'0') {
        spec.zero = true;
        i += 1;
    }
    let number = |i: &mut usize| -> std::result::Result<usize, &'static str> {
        let start = *i;
        while chars.get(*i).is_some_and(char::is_ascii_digit) {
            *i += 1;
        }
        if *i == start {
            return Err("expected a number");
        }
        let value = chars[start..*i]
            .iter()
            .collect::<String>()
            .parse::<usize>()
            .map_err(|_| "number overflow")?;
        if value > 65536 {
            return Err("label width or precision exceeds 65536");
        }
        Ok(value)
    };
    if chars.get(i).is_some_and(char::is_ascii_digit) {
        spec.width = number(&mut i)?;
    }
    if chars.get(i) == Some(&'.') {
        i += 1;
        spec.precision = Some(number(&mut i)?);
    }
    if chars.get(i) == Some(&'L') {
        spec.localized = true;
        i += 1;
    }
    if let Some(ch) = chars.get(i) {
        spec.kind = Some(*ch);
        i += 1;
    }
    if i != chars.len() {
        return Err("invalid format specifier");
    }
    Ok(spec)
}

fn format_value(text: &str, value: Value<'_>) -> std::result::Result<String, &'static str> {
    let spec = parse_spec(text)?;
    let mut prefix = String::new();
    let numeric = !matches!(value, Value::Text(_));
    let mut body = match value {
        Value::Text(value) => {
            if spec.sign.is_some()
                || spec.alternate
                || spec.zero
                || spec.localized
                || !matches!(spec.kind, None | Some('s' | '?'))
            {
                return Err("invalid string format");
            }
            let value = if spec.kind == Some('?') {
                debug_string(value)
            } else {
                value.into()
            };
            let mut remaining = spec.precision.unwrap_or(usize::MAX);
            value
                .chars()
                .take_while(|ch| {
                    let width = character_width(*ch);
                    if width > remaining {
                        return false;
                    }
                    remaining -= width;
                    true
                })
                .collect::<String>()
        }
        Value::Integer(value) => {
            if spec.precision.is_some()
                || !matches!(
                    spec.kind,
                    None | Some('d' | 'x' | 'X' | 'b' | 'B' | 'o' | 'c')
                )
            {
                return Err("invalid integer format");
            }
            if spec.kind == Some('c') {
                let byte = value.unsigned_abs() as u8;
                if byte == 0 {
                    String::new()
                } else {
                    String::from_utf8_lossy(&[byte]).into_owned()
                }
            } else {
                set_sign(&mut prefix, value < 0, &spec);
                let n = value.unsigned_abs();
                let (base, digits) = match spec.kind {
                    Some('x') => ("0x", format!("{n:x}")),
                    Some('X') => ("0X", format!("{n:X}")),
                    Some('b') => ("0b", format!("{n:b}")),
                    Some('B') => ("0B", format!("{n:b}")),
                    Some('o') => (if n == 0 { "" } else { "0" }, format!("{n:o}")),
                    _ => ("", n.to_string()),
                };
                if spec.alternate {
                    prefix.push_str(base);
                }
                digits
            }
        }
        Value::Real(value) => {
            if !matches!(
                spec.kind,
                None | Some('f' | 'F' | 'e' | 'E' | 'g' | 'G' | 'a' | 'A')
            ) {
                return Err("invalid real format");
            }
            set_sign(&mut prefix, value.is_sign_negative(), &spec);
            format_real(value.abs(), &spec)
        }
    };
    let length = prefix.chars().count() + body.chars().map(character_width).sum::<usize>();
    let pad = spec.width.saturating_sub(length);
    if spec.zero
        && spec.align.is_none()
        && numeric
        && !matches!(body.as_str(), "nan" | "inf" | "NAN" | "INF")
    {
        return Ok(format!("{prefix}{}{body}", "0".repeat(pad)));
    }
    body.insert_str(0, &prefix);
    let align = spec.align.unwrap_or(if numeric { '>' } else { '<' });
    let left = match align {
        '>' => pad,
        '^' => pad / 2,
        _ => 0,
    };
    let fill = spec.fill.unwrap_or(' ').to_string();
    Ok(format!(
        "{}{body}{}",
        fill.repeat(left),
        fill.repeat(pad - left)
    ))
}

fn character_width(ch: char) -> usize {
    let cp = ch as u32;
    let wide = matches!(cp, 0x1100..=0x115f | 0x2329..=0x232a | 0x2e80..=0xa4cf
        | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe10..=0xfe19 | 0xfe30..=0xfe6f
        | 0xff00..=0xff60 | 0xffe0..=0xffe6 | 0x1f300..=0x1f64f | 0x1f900..=0x1f9ff
        | 0x20000..=0x2fffd | 0x30000..=0x3fffd)
        && cp != 0x303f;
    1 + usize::from(wide)
}

fn debug_string(text: &str) -> String {
    let mut result = String::from("\"");
    for ch in text.chars() {
        match ch {
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            ch if ch.is_control() && (ch as u32) < 256 => {
                result.push_str(&format!("\\x{:02x}", ch as u32))
            }
            ch if ch.is_control() => result.push_str(&format!("\\u{:04x}", ch as u32)),
            _ => result.push(ch),
        }
    }
    result.push('"');
    result
}

fn set_sign(prefix: &mut String, negative: bool, spec: &Spec) {
    if negative {
        prefix.push('-');
    } else if matches!(spec.sign, Some('+' | ' ')) {
        prefix.push(spec.sign.unwrap());
    }
}

fn exponent(text: String, uppercase: bool) -> String {
    let Some((mantissa, power)) = text.split_once('e') else {
        return text;
    };
    let power = power.parse::<i32>().unwrap_or(0);
    format!("{mantissa}{}{power:+03}", if uppercase { 'E' } else { 'e' })
}

fn format_real(value: f64, spec: &Spec) -> String {
    let upper = spec.kind.is_some_and(char::is_uppercase);
    if !value.is_finite() {
        let text = if value.is_nan() { "nan" } else { "inf" };
        return if upper {
            text.to_uppercase()
        } else {
            text.into()
        };
    }
    let kind = spec.kind.unwrap_or('g').to_ascii_lowercase();
    if kind == 'a' {
        return hex_real(value, spec, upper);
    }
    let precision = spec.precision.unwrap_or(6);
    let mut output = match kind {
        'f' => format!("{value:.precision$}"),
        'e' => exponent(format!("{value:.precision$e}"), upper),
        _ if spec.kind.is_none() && spec.precision.is_none() => {
            if value != 0. && !(1e-4..1e16).contains(&value) {
                exponent(format!("{value:e}"), false)
            } else {
                value.to_string()
            }
        }
        _ => {
            let precision = precision.max(1);
            let rounded = format!("{value:.digits$e}", digits = precision - 1);
            let power = rounded
                .split_once('e')
                .and_then(|(_, e)| e.parse::<i32>().ok())
                .unwrap_or(0);
            if power < -4 || power >= precision as i32 {
                exponent(rounded, upper)
            } else {
                let places = (precision as i32 - 1 - power).max(0) as usize;
                format!("{value:.places$}")
            }
        }
    };
    if kind == 'g' && !spec.alternate && (spec.kind.is_some() || spec.precision.is_some()) {
        let split = output.find(['e', 'E']).unwrap_or(output.len());
        let mut mantissa = output[..split].to_string();
        if mantissa.contains('.') {
            mantissa.truncate(mantissa.trim_end_matches('0').trim_end_matches('.').len());
        }
        output = mantissa + &output[split..];
    }
    if spec.alternate && !output.contains('.') {
        let split = output.find(['e', 'E']).unwrap_or(output.len());
        output.insert(split, '.');
    }
    output
}

fn hex_real(value: f64, spec: &Spec, upper: bool) -> String {
    let bits = value.to_bits();
    let raw_exponent = ((bits >> 52) & 0x7ff) as i32;
    let power = if raw_exponent == 0 {
        -1022
    } else {
        raw_exponent - 1023
    };
    let mut significand =
        (bits & ((1u64 << 52) - 1)) | if raw_exponent == 0 { 0 } else { 1u64 << 52 };
    let precision = spec.precision.unwrap_or(13);
    if precision < 13 {
        let shift = 52 - precision * 4;
        let unit = 1u64 << shift;
        let remainder = significand & (unit - 1);
        let retained = significand >> shift;
        significand = (retained
            + u64::from(remainder > unit / 2 || (remainder == unit / 2 && retained & 1 != 0)))
            << shift;
    }
    let mut fraction = format!("{:013x}", significand & ((1u64 << 52) - 1));
    fraction.truncate(precision.min(13));
    if precision > 13 {
        fraction.push_str(&"0".repeat(precision - 13));
    }
    if spec.precision.is_none() {
        fraction.truncate(fraction.trim_end_matches('0').len());
    }
    let dot = if !fraction.is_empty() || spec.alternate {
        "."
    } else {
        ""
    };
    let text = format!("0x{:x}{dot}{fraction}p{power:+}", significand >> 52);
    if upper {
        text.to_uppercase()
    } else {
        text
    }
}
