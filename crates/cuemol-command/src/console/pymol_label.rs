#[derive(Debug, PartialEq)]
enum Token {
    Text(String),
    Field(String),
    Op(char),
}

fn tokens(source: &str) -> Result<Vec<Token>, String> {
    let mut chars = source.char_indices().peekable();
    let mut result = Vec::new();
    while let Some((at, ch)) = chars.next() {
        match ch {
            ch if ch.is_whitespace() => {}
            '\'' | '"' => {
                let mut value = String::new();
                let mut closed = false;
                for (_, next) in chars.by_ref() {
                    if next == ch {
                        closed = true;
                        break;
                    }
                    value.push(next);
                }
                if !closed {
                    return Err("Unclosed label string".into());
                }
                result.push(Token::Text(value));
            }
            '+' | '%' | '(' | ')' | ',' => result.push(Token::Op(ch)),
            ch if ch.is_ascii_alphabetic() || ch == '_' => {
                let mut value = ch.to_string();
                while chars
                    .peek()
                    .is_some_and(|(_, ch)| ch.is_ascii_alphanumeric() || *ch == '_')
                {
                    value.push(chars.next().unwrap().1);
                }
                result.push(Token::Field(value));
            }
            _ => return Err(format!("Unsupported label character {ch} at {}", at + 1)),
        }
    }
    Ok(result)
}

fn field(name: &str) -> Result<&str, String> {
    Ok(match name {
        "name" | "resn" | "resi" | "chain" | "elem" | "alt" | "bfac" | "occ" | "aid"
        | "molname" => name,
        "resv" => "resi",
        "b" => "bfac",
        "q" => "occ",
        "ID" => "aid",
        "model" => "molname",
        _ => return Err(format!("Unsupported atom attribute in label: {name}")),
    })
}

fn literal(text: &str) -> String {
    text.replace('{', "{{").replace('}', "}}")
}

fn conversion(spec: &str, name: &str) -> Result<String, String> {
    let mut rest = spec.strip_prefix('%').ok_or("Invalid label conversion")?;
    let left = rest.starts_with('-');
    rest = rest.trim_start_matches('-');
    let zero = rest.starts_with('0');
    rest = rest.strip_prefix('0').unwrap_or(rest);
    let width_end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    let width = &rest[..width_end];
    rest = &rest[width_end..];
    let precision = if let Some(next) = rest.strip_prefix('.') {
        let end = next
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(next.len());
        if end == 0 {
            return Err(format!("Invalid label precision: {spec}"));
        }
        rest = &next[end..];
        Some(&next[..end])
    } else {
        None
    };
    if rest.len() != 1 || !"sdifeg".contains(rest) {
        return Err(format!("Unsupported label format: {spec}"));
    }
    let numeric = matches!(name, "bfac" | "occ" | "aid");
    if !numeric && rest != "s" && (precision.is_some() || zero) {
        return Err(format!(
            "Label format {spec} needs a number; {name} is text"
        ));
    }
    let mut output = String::new();
    if !width.is_empty() {
        output.push_str(&format!(
            "{}{width}",
            if zero && numeric && !left {
                "0"
            } else if left {
                "<"
            } else {
                ">"
            }
        ));
    }
    if numeric {
        if matches!(rest, "d" | "i") {
            output.push_str(if name == "aid" { "d" } else { ".0f" });
        } else if rest != "s" {
            if let Some(precision) = precision {
                output.push_str(&format!(".{precision}"));
            }
            output.push_str(rest);
        }
    }
    Ok(if output.is_empty() {
        format!("{{{name}}}")
    } else {
        format!("{{{name}:{output}}}")
    })
}

fn percent(text: &str, fields: &[&str]) -> Result<String, String> {
    let mut result = String::new();
    let mut chars = text.char_indices().peekable();
    let mut index = 0;
    while let Some((start, ch)) = chars.next() {
        if ch != '%' {
            result.push_str(&literal(&ch.to_string()));
            continue;
        }
        if chars.peek().is_some_and(|(_, ch)| *ch == '%') {
            chars.next();
            result.push('%');
            continue;
        }
        let mut end = None;
        for (at, ch) in chars.by_ref() {
            if ch.is_ascii_alphabetic() {
                end = Some(at + ch.len_utf8());
                break;
            }
            if !ch.is_ascii_digit() && ch != '-' && ch != '.' {
                break;
            }
        }
        let end = end.ok_or("Invalid percent conversion in label")?;
        let name = fields
            .get(index)
            .ok_or("More percent conversions than label values")?;
        result.push_str(&conversion(&text[start..end], field(name)?)?);
        index += 1;
    }
    if index != fields.len() {
        return Err("More label values than percent conversions".into());
    }
    Ok(result)
}

pub(super) fn translate(source: &str) -> Result<String, String> {
    let tokens = tokens(source.trim())?;
    if tokens.is_empty() {
        return Ok(String::new());
    }
    if let [Token::Text(text), Token::Op('%'), rest @ ..] = tokens.as_slice() {
        let mut fields = Vec::new();
        match rest {
            [Token::Field(name)] => fields.push(name.as_str()),
            [Token::Op('('), inner @ .., Token::Op(')')] => {
                for (index, token) in inner.iter().enumerate() {
                    match token {
                        Token::Field(name) if index % 2 == 0 => fields.push(name.as_str()),
                        Token::Op(',') if index % 2 == 1 => {}
                        _ => {
                            return Err(
                                "Label values must be atom attributes separated by commas".into()
                            )
                        }
                    }
                }
            }
            _ => return Err("Use an atom attribute or (a, b, ...) after %".into()),
        }
        return percent(text, &fields);
    }
    if let [Token::Text(text)] = tokens.as_slice() {
        let has_field = text.split('{').skip(1).any(|part| {
            part.split_once('}').is_some_and(|(key, _)| {
                key.chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_alphabetic())
                    && key
                        .split(':')
                        .next()
                        .unwrap_or("")
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
            })
        });
        return Ok(if has_field {
            text.clone()
        } else {
            literal(text)
        });
    }
    let mut result = String::new();
    let mut rest = tokens.as_slice();
    loop {
        rest = match rest {
            [Token::Text(text), tail @ ..] => {
                result.push_str(&literal(text));
                tail
            }
            [Token::Field(name), Token::Op('('), Token::Field(arg), Token::Op(')'), tail @ ..]
                if name == "str" =>
            {
                result.push_str(&format!("{{{}}}", field(arg)?));
                tail
            }
            [Token::Field(name), tail @ ..] => {
                result.push_str(&format!("{{{}}}", field(name)?));
                tail
            }
            _ => {
                return Err("A label term must be an attribute, a string or str(attribute)".into())
            }
        };
        match rest {
            [] => return Ok(result),
            [Token::Op('+'), tail @ ..] => rest = tail,
            _ => return Err("Separate label terms with +".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_label_expressions_keep_fields_precision_and_literals() {
        for (source, expected) in [
            (r#""%s%s" % (resn, resi)"#, "{resn}{resi}"),
            (r#""%.2f" % b"#, "{bfac:.2f}"),
            (r#""%04d: %-5s %%" % (ID, name)"#, "{aid:04d}: {name:<5} %"),
            (r#"chain + ":" + resn + str(resi)"#, "{chain}:{resn}{resi}"),
            (r#""{resn}{resi}""#, "{resn}{resi}"),
            (r#""{}""#, "{{}}"),
            ("", ""),
        ] {
            assert_eq!(translate(source).unwrap(), expected, "{source}");
            cuemol_molstr::label_format::LabelFormat::new(expected).unwrap();
        }
        for source in [
            "name * 2",
            "os.system('x')",
            r#""%s%s" % name"#,
            r#""%03d" % resi"#,
            "name +",
        ] {
            assert!(translate(source).is_err(), "{source}");
        }
    }
}
