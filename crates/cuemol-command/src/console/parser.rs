use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLine {
    pub text: String,
    pub quiet: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub name: Option<String>,
    pub value: String,
}

pub fn split_groups(text: &str, separator: char, partial: bool) -> Result<Vec<&str>, String> {
    Ok(split_groups_raw(text, separator, partial)?
        .into_iter()
        .map(str::trim)
        .collect())
}

pub(super) fn split_groups_raw(
    text: &str,
    separator: char,
    partial: bool,
) -> Result<Vec<&str>, String> {
    let mut parts = Vec::new();
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut start = 0;
    for (offset, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(end) = quote {
            if ch == end {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' => {
                if stack.pop() != Some(ch) && !partial {
                    return Err(format!(
                        "Unmatched {ch} at character {}",
                        text[..offset].chars().count() + 1
                    ));
                }
            }
            _ if ch == separator && stack.is_empty() => {
                parts.push(&text[start..offset]);
                start = offset + ch.len_utf8();
            }
            _ => {}
        }
    }
    if !partial && (quote.is_some() || !stack.is_empty()) {
        return Err("Unclosed quote or bracket".into());
    }
    parts.push(&text[start..]);
    Ok(parts)
}

pub fn split_commands(source: &str) -> Result<Vec<CommandLine>, String> {
    let mut result = Vec::new();
    let mut pending = String::new();
    for raw in source.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some(continued) = line.strip_suffix('\\') {
            pending.push_str(continued);
            continue;
        }
        pending.push_str(line);
        let line = pending.trim();
        if !line.is_empty() && !line.starts_with('#') {
            for part in split_groups(line, ';', false)? {
                let (text, quiet) = part
                    .strip_prefix("_ ")
                    .map_or((part, false), |text| (text, true));
                if !text.is_empty() {
                    result.push(CommandLine {
                        text: text.trim().into(),
                        quiet,
                    });
                }
            }
        }
        pending.clear();
    }
    if !pending.trim().is_empty() && !pending.trim_start().starts_with('#') {
        for part in split_groups(&pending, ';', false)? {
            let (text, quiet) = part
                .strip_prefix("_ ")
                .map_or((part, false), |text| (text, true));
            if !text.is_empty() {
                result.push(CommandLine {
                    text: text.trim().into(),
                    quiet,
                });
            }
        }
    }
    Ok(result)
}

pub fn head(line: &str) -> (&str, &str) {
    let line = line.trim();
    line.find(char::is_whitespace)
        .map_or((line, ""), |at| (&line[..at], line[at..].trim()))
}

pub fn parse_arguments(text: &str, legacy: bool) -> Result<Vec<Argument>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    split_groups(text, ',', false)?
        .into_iter()
        .map(|part| {
            let named = (!legacy)
                .then(|| part.split_once('='))
                .flatten()
                .filter(|(name, _)| {
                    !name.trim().is_empty()
                        && name
                            .trim()
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                });
            Ok(if let Some((name, value)) = named {
                Argument {
                    name: Some(name.trim().into()),
                    value: value.trim().into(),
                }
            } else {
                Argument {
                    name: None,
                    value: part.trim().into(),
                }
            })
        })
        .collect()
}

pub fn bind_arguments(
    params: &[String],
    args: &[Argument],
) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let mut position = 0;
    let mut saw_named = false;
    for arg in args {
        let key = if let Some(name) = &arg.name {
            saw_named = true;
            if !params.contains(name) {
                return Err(format!("Unknown argument: {name}"));
            }
            name.clone()
        } else {
            if saw_named {
                return Err("A positional argument follows a named one".into());
            }
            let key = params
                .get(position)
                .ok_or_else(|| "Too many arguments".to_string())?
                .clone();
            position += 1;
            key
        };
        if out.insert(key.clone(), arg.value.clone()).is_some() {
            return Err(format!("Argument supplied twice: {key}"));
        }
    }
    Ok(out)
}

pub fn bind_pymol_arguments(
    params: &[String],
    text: &str,
    legacy: bool,
) -> Result<BTreeMap<String, String>, String> {
    let mut expanded = Vec::new();
    for argument in parse_arguments(text, false)? {
        if legacy
            && argument
                .name
                .as_ref()
                .is_some_and(|name| !params.contains(name))
        {
            expanded.push(Argument {
                name: None,
                value: argument.name.unwrap(),
            });
            expanded.push(Argument {
                name: None,
                value: argument.value,
            });
        } else {
            expanded.push(argument);
        }
    }
    let mut result = BTreeMap::new();
    for (index, argument) in expanded.into_iter().enumerate() {
        let key = if let Some(name) = argument.name {
            if !params.contains(&name) {
                return Err(format!("Unknown argument: {name}"));
            }
            name
        } else {
            params.get(index).ok_or("Too many arguments")?.clone()
        };
        if !argument.value.is_empty() {
            result.insert(key, argument.value);
        }
    }
    Ok(result)
}

pub fn unquote(text: &str) -> &str {
    let text = text.trim();
    if text.len() >= 2
        && (text.starts_with('"') && text.ends_with('"')
            || text.starts_with('\'') && text.ends_with('\''))
    {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

pub fn shortcut_matches(word: &str, name: &str) -> bool {
    if name.starts_with(word) {
        return true;
    }
    let parts = name.split('_').collect::<Vec<_>>();
    if parts.len() < 2 {
        return false;
    }
    let leading = &parts[..parts.len() - 1];
    let longest = leading.iter().map(|p| p.len()).max().unwrap_or(0);
    (1..=longest).any(|length| {
        let mut short = leading
            .iter()
            .map(|part| part.chars().take(length).collect::<String>())
            .collect::<Vec<_>>();
        short.push(parts.last().unwrap().to_string());
        short.join("_").starts_with(word)
    })
}

pub fn lookup<'a>(word: &str, names: impl IntoIterator<Item = &'a str>) -> Result<&'a str, String> {
    let mut matches = Vec::new();
    for name in names {
        if name == word {
            return Ok(name);
        }
        if shortcut_matches(word, name) {
            matches.push(name);
        }
    }
    matches.sort_unstable();
    match matches.as_slice() {
        [name] => Ok(name),
        [] => Err(format!("Unknown command: {word}")),
        _ => Err(format!("Ambiguous command: {}", matches.join(", "))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_keep_quoted_delimiters_and_join_continuations() {
        assert!(split_commands("# unfinished comment \\")
            .unwrap()
            .is_empty());
        let lines = split_commands("# comment\nselect mol, (chain A, chain B); set bgcolor, '#204080'\n_ rename mol, \\\n'日本語; molecule'\n").unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "select mol, (chain A, chain B)");
        assert_eq!(
            lines[2],
            CommandLine {
                text: "rename mol, '日本語; molecule'".into(),
                quiet: true
            }
        );
        let args = parse_arguments(head(&lines[0].text).1, false).unwrap();
        assert_eq!(args[1].value, "(chain A, chain B)");
        assert!(split_commands("select mol, (chain A").is_err());
        assert!(parse_arguments("mol, 'open", false).is_err());
    }

    #[test]
    fn arguments_reject_unknown_duplicate_and_excess_values() {
        let params = vec!["object".into(), "value".into()];
        let args = parse_arguments("'two, atoms', value='a;b'", false).unwrap();
        let bound = bind_arguments(&params, &args).unwrap();
        assert_eq!(unquote(&bound["object"]), "two, atoms");
        assert_eq!(unquote(&bound["value"]), "a;b");
        for text in [
            "mol, value=red, value=blue",
            "typo=red",
            "a,b,c",
            "object=mol,red",
        ] {
            assert!(bind_arguments(&params, &parse_arguments(text, false).unwrap()).is_err());
        }
        assert!(parse_arguments("foo=chain A", true).unwrap()[0]
            .name
            .is_none());
        assert_eq!(
            lookup("s_n", ["set", "set_name", "show"]).unwrap(),
            "set_name"
        );
        assert_eq!(lookup("set", ["set", "set_name"]).unwrap(), "set");
        assert!(lookup("s", ["set", "show"]).is_err());
    }

    #[test]
    fn pymol_legacy_equals_and_mixed_arguments_match_published_binding_rules() {
        let params = vec!["name".into(), "selection".into(), "enable".into()];
        let args = bind_pymol_arguments(&params, "chosen=chain A, enable=0", true).unwrap();
        assert_eq!(args["name"], "chosen");
        assert_eq!(args["selection"], "chain A");
        assert_eq!(args["enable"], "0");
        let args = bind_pymol_arguments(&params, "name=first, resi 2, name=last", false).unwrap();
        assert_eq!(args["name"], "last");
        assert_eq!(args["selection"], "resi 2");
        assert!(bind_pymol_arguments(&params, "typo=bad", false).is_err());
    }
}
