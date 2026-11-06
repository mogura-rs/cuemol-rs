use super::{colors, spec, Arg, Command};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Token<'a> {
    pub value: &'a str,
    pub rest: &'a str,
    pub quoted: bool,
}
pub(super) fn next_token(text: &str) -> Option<Token<'_>> {
    let text = text.trim_start();
    let first = text.chars().next()?;
    if first == '\'' || first == '"' {
        let end = text[1..].find(first).map(|end| end + 1);
        return Some(Token {
            value: &text[1..end.unwrap_or(text.len())],
            rest: end.map_or("", |end| text[end + 1..].trim_start()),
            quoted: true,
        });
    }
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    Some(Token {
        value: &text[..end],
        rest: text[end..].trim_start(),
        quoted: false,
    })
}
pub(super) fn unescape(text: &str) -> String {
    let mut chars = text.chars().peekable();
    let mut output = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.peek().copied().filter(|c| "nt\\'\"".contains(*c)) {
                chars.next();
                output.push(match next {
                    'n' => '\n',
                    't' => '\t',
                    _ => next,
                });
                continue;
            }
        }
        output.push(ch);
    }
    output
}
pub(crate) fn split_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if re!(r"^\s*alias\s+\S+\s+\S").is_match(line)
            && !re!(r"^\s*alias\s+(?:list|delete)\b").is_match(line)
        {
            out.push(line.trim().into());
            continue;
        }
        let mut quote = None;
        let mut start = 0;
        for (i, ch) in line
            .char_indices()
            .chain(std::iter::once((line.len(), ';')))
        {
            if let Some(q) = quote {
                if q == ch {
                    quote = None;
                }
            } else if ch == '\'' || ch == '"' {
                quote = Some(ch);
            } else if ch == ';' {
                let part = line[start..i].trim();
                if part.starts_with('#') {
                    break;
                }
                if !part.is_empty() {
                    out.push(part.into());
                }
                start = i + 1;
            }
        }
        if quote.is_some() && start <= line.len() {
            let part = line[start..].trim();
            if !part.is_empty() && !part.starts_with('#') {
                out.push(part.into());
            }
        }
    }
    out
}
pub(super) fn resolve<'a>(
    text: &str,
    commands: &'a [Command],
) -> Result<(&'a Command, usize), String> {
    let typed = text
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let mut path = Vec::<&str>::new();
    let mut found = None;
    for (i, word) in typed.iter().enumerate() {
        let mut names = Vec::<&str>::new();
        for command in commands {
            let words = command.name.split_whitespace().collect::<Vec<_>>();
            if words.len() > i && words[..i] == path && !names.contains(&words[i]) {
                names.push(words[i]);
            }
        }
        let candidate = names.iter().copied().find(|n| *n == word).or_else(|| {
            names
                .iter()
                .copied()
                .find(|n| !word.is_empty() && n.to_ascii_lowercase().starts_with(word))
        });
        let Some(word) = candidate else { break };
        path.push(word);
        if let Some(command) = commands
            .iter()
            .find(|c| c.name.split_whitespace().eq(path.iter().copied()))
        {
            found = Some((command, path.len()));
        }
    }
    found.ok_or_else(|| {
        if path.is_empty() {
            format!(
                "Error: unknown command: \"{}\"",
                typed.first().map(String::as_str).unwrap_or("")
            )
        } else {
            format!("Error: incomplete command: \"{}\"", path.join(" "))
        }
    })
}
pub(super) fn after_words(mut text: &str, count: usize) -> &str {
    for _ in 0..count {
        text = next_token(text).map_or("", |token| token.rest);
    }
    text
}
pub(super) fn keyword<'a>(typed: &str, args: &'a [Arg]) -> Result<Option<&'a Arg>, String> {
    let original = typed;
    let typed = typed.to_ascii_lowercase();
    let candidates = args
        .iter()
        .filter(|arg| arg.role != "required")
        .collect::<Vec<_>>();
    if let Some(arg) = candidates
        .iter()
        .find(|arg| arg.name.eq_ignore_ascii_case(&typed))
    {
        return Ok(Some(arg));
    }
    let hits = candidates
        .into_iter()
        .filter(|arg| arg.name.to_ascii_lowercase().starts_with(&typed))
        .collect::<Vec<_>>();
    if hits.len() < 2 {
        return Ok(hits.first().copied());
    }
    let count = |arg: &&Arg| arg.name.chars().filter(char::is_ascii_uppercase).count();
    let fewest = hits.iter().map(count).min().unwrap();
    let short = hits
        .iter()
        .copied()
        .filter(|arg| count(&arg) == fewest)
        .collect::<Vec<_>>();
    if short.len() == 1 {
        Ok(Some(short[0]))
    } else {
        Err(format!(
            "\"{original}\" is ambiguous: {}",
            hits.iter()
                .map(|arg| arg.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}
fn enumeration<'a>(
    values: &[String],
    text: &'a str,
) -> Result<Option<(String, &'a str)>, spec::Error> {
    let Some(token) = next_token(text) else {
        return Ok(None);
    };
    let word = unescape(token.value).to_ascii_lowercase();
    if let Some(value) = values.iter().find(|v| v.eq_ignore_ascii_case(&word)) {
        return Ok(Some((value.clone(), token.rest)));
    }
    let hits = values
        .iter()
        .filter(|v| !word.is_empty() && v.to_ascii_lowercase().starts_with(&word))
        .collect::<Vec<_>>();
    if hits.len() > 1 {
        return Err(spec::Error::new(
            format!(
                "\"{}\" is ambiguous: {}",
                token.value,
                hits.iter()
                    .map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            0,
        ));
    }
    Ok(hits.first().map(|value| ((*value).clone(), token.rest)))
}
pub(super) fn number_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == f64::INFINITY {
        return "Infinity".into();
    }
    if value == f64::NEG_INFINITY {
        return "-Infinity".into();
    }
    if value == 0. {
        return "0".into();
    }
    if value.abs() >= 1e-6 && value.abs() < 1e21 {
        return value.to_string();
    }
    let mut text = serde_json::to_string(&value).unwrap().replace(".0e", "e");
    if let Some(i) = text.find('e') {
        if !text[i + 1..].starts_with('-') {
            text.insert(i + 1, '+');
        }
    }
    text
}
fn read_arg<'a>(
    arg: &Arg,
    text: &'a str,
    names: &BTreeSet<String>,
) -> Result<Option<(String, &'a str)>, spec::Error> {
    match arg.kind.as_str() {
        "rest" => return Ok(Some((text.trim().into(), ""))),
        "spec" => {
            return spec::parse(text, names)
                .map(|r| r.map(|(_, end)| (text[..end].into(), text[end..].trim_start())))
        }
        "color" => return Ok(colors::read(text).map(|color| (color.value(), color.rest))),
        "color-or-enum" => {
            if let Some(color) = colors::read(text) {
                return Ok(Some((color.value(), color.rest)));
            }
            return enumeration(&arg.values, text);
        }
        "enum" => return enumeration(&arg.values, text),
        "enum-list" => {
            let mut rest = text;
            let mut values = Vec::new();
            loop {
                let Some((value, tail)) = enumeration(&arg.values, rest)? else {
                    return Ok(None);
                };
                values.push(value);
                if !tail.starts_with(',') {
                    return Ok(Some((values.join(","), tail)));
                }
                rest = tail[1..].trim_start();
            }
        }
        _ => {}
    }
    let Some(token) = next_token(text) else {
        return Ok(None);
    };
    let value = match arg.kind.as_str() {
        "path" => token.value.into(),
        "string" => {
            if token.quoted {
                unescape(token.value)
            } else {
                token.value.into()
            }
        }
        "bool" => {
            let word = token.value.to_ascii_lowercase();
            if word == "1" || word == "on" || (!word.is_empty() && "true".starts_with(&word)) {
                "true".into()
            } else if word == "0"
                || word == "off"
                || (!word.is_empty() && "false".starts_with(&word))
            {
                "false".into()
            } else {
                return Ok(None);
            }
        }
        "axis" => {
            if re!(r"(?i)^-?[xyz]$").is_match(token.value)
                || re!(r"^[-+.\d]+,[-+.\d]+,[-+.\d]+$").is_match(token.value)
            {
                token.value.into()
            } else {
                return Ok(None);
            }
        }
        "int" | "float" => {
            let pattern = if arg.kind == "int" {
                re!(r"^[-+]?\d+$")
            } else {
                re!(r"^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$")
            };
            if !pattern.is_match(token.value) {
                return Ok(None);
            }
            number_string(token.value.parse::<f64>().unwrap_or(f64::NAN))
        }
        _ => {
            return Err(spec::Error::new(
                format!("unsupported argument type: {}", arg.kind),
                0,
            ))
        }
    };
    Ok(Some((value, token.rest)))
}
pub(super) fn type_name(arg: &Arg) -> String {
    match arg.kind.as_str() {
        "rest" => "the rest of line".into(),
        "spec" => "an atom specification".into(),
        "path" => "a file name".into(),
        "string" => "a text string".into(),
        "bool" => "true or false".into(),
        "axis" => "an axis vector".into(),
        "int" => "an integer".into(),
        "float" => "a number".into(),
        "color" => "a color".into(),
        "enum" => format!("one of {}", arg.values.join(", ")),
        "color-or-enum" => format!("a color or one of {}", arg.values.join(", ")),
        "enum-list" => format!("a list of one of {}", arg.values.join(", ")),
        _ => arg.kind.clone(),
    }
}
fn caret(line: &str, pos: usize, message: &str) -> String {
    format!("{line}\n{}^ {message}", " ".repeat(pos.min(line.len())))
}
pub(super) fn bind(
    command: &Command,
    line: &str,
    names: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, String> {
    let full = line.trim();
    let mut rest = after_words(full, command.name.split_whitespace().count());
    let mut args = BTreeMap::new();
    let mut read = |arg: &Arg, rest: &mut &str| -> Result<bool, String> {
        let start = full.len() - rest.len();
        let value = read_arg(arg, rest, names)
            .map_err(|error| caret(full, start + error.pos, &error.message))?;
        if let Some((value, tail)) = value {
            args.insert(arg.name.clone(), value);
            *rest = tail;
            Ok(true)
        } else {
            Ok(false)
        }
    };
    for arg in command.args.iter().filter(|arg| arg.role == "required") {
        rest = rest.trim_start();
        if !read(arg, &mut rest)? {
            return Err(caret(
                full,
                full.len() - rest.len(),
                &format!(
                    "Missing or invalid {} argument: expected {}",
                    arg.name,
                    type_name(arg)
                ),
            ));
        }
    }
    for arg in command.args.iter().filter(|arg| arg.role == "optional") {
        rest = rest.trim_start();
        let starts_keyword = next_token(rest).is_some_and(|token| {
            !token.quoted
                && token.value.starts_with(char::is_alphabetic)
                && !matches!(keyword(token.value, &command.args), Ok(None))
        });
        if rest.is_empty() || starts_keyword {
            break;
        }
        if !read(arg, &mut rest)? && arg.kind != "spec" {
            break;
        }
    }
    let mut given = BTreeSet::new();
    loop {
        rest = rest.trim_start();
        let Some(word) = next_token(rest) else { break };
        let pos = full.len() - rest.len();
        let arg = keyword(word.value, &command.args)
            .map_err(|error| caret(full, pos, &error))?
            .ok_or_else(|| {
                caret(
                    full,
                    pos,
                    &format!("Expected a keyword; \"{}\" is not one", word.value),
                )
            })?;
        if !given.insert(&arg.name) {
            return Err(caret(full, pos, &format!("Repeated keyword: {}", arg.name)));
        }
        rest = word.rest;
        let value_at = full.len() - rest.len();
        if !read(arg, &mut rest)? {
            return Err(caret(
                full,
                value_at,
                &format!(
                    "Missing or invalid {} argument: expected {}",
                    arg.name,
                    type_name(arg)
                ),
            ));
        }
    }
    for arg in &command.args {
        args.entry(arg.name.clone())
            .or_insert_with(|| arg.default.clone().unwrap_or_default());
    }
    Ok(args)
}
pub(super) fn usage(command: &Command) -> String {
    std::iter::once(command.name.clone())
        .chain(command.args.iter().map(|arg| match arg.role.as_str() {
            "required" => arg.name.clone(),
            "optional" => format!("[{}]", arg.name),
            _ => format!("[{} {}]", arg.name, type_name(arg)),
        }))
        .collect::<Vec<_>>()
        .join(" ")
}
