use crate::{CueMolError, GuiConfigEntry, Result};

const HEADER: &str = "# cuemol-config-v2";

pub(crate) fn encode(entries: Vec<GuiConfigEntry>) -> String {
    let mut output = String::from(HEADER);
    for entry in entries {
        output.push('\n');
        escape(&entry.key, &mut output);
        output.push('\t');
        escape(&entry.value, &mut output);
    }
    output
}

fn escape(value: &str, output: &mut String) {
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '#' => output.push_str("\\#"),
            _ => output.push(ch),
        }
    }
}

fn unescape(value: &str) -> Option<String> {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        output.push(if ch == '\\' {
            match chars.next()? {
                '\\' => '\\',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '#' => '#',
                _ => return None,
            }
        } else {
            ch
        });
    }
    Some(output)
}

pub(crate) fn decode(input: &str) -> Result<Vec<GuiConfigEntry>> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let escaped = input.lines().next() == Some(HEADER);
    let mut entries = Vec::new();
    for (index, line) in input.lines().enumerate() {
        let has_fields = line.contains('\t');
        if (!escaped || !has_fields)
            && (line.trim().is_empty() || line.trim_start().starts_with('#'))
        {
            continue;
        }
        let invalid = || CueMolError::parse(format!("invalid config line {}", index + 1));
        let (key, value) = line.split_once('\t').ok_or_else(invalid)?;
        let (key, value) = if escaped {
            (
                unescape(key).ok_or_else(invalid)?,
                unescape(value).ok_or_else(invalid)?,
            )
        } else {
            (key.trim().to_string(), value.replace("\\n", "\n"))
        };
        if key.is_empty() {
            return Err(invalid());
        }
        entries.push(GuiConfigEntry { key, value });
    }
    Ok(entries)
}
