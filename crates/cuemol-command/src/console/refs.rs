use super::{node_accepts, Action, CommandSpec, Node, Session};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Word {
    pub value: String,
    pub pattern: String,
}

pub(super) fn words(text: &str) -> Vec<Word> {
    if let Ok(value) = serde_json::from_str::<String>(text) {
        let pattern = value.chars().fold(String::new(), |mut pattern, ch| {
            if matches!(ch, '\\' | '*' | '?') {
                pattern.push('\\');
            }
            pattern.push(ch);
            pattern
        });
        return vec![Word { value, pattern }];
    }
    let mut out = Vec::new();
    let mut word: Option<Word> = None;
    let mut quote = None;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if quote.is_none() && ch.is_whitespace() {
            if let Some(word) = word.take() {
                out.push(word);
            }
            continue;
        }
        let word = word.get_or_insert_with(Word::default);
        if quote == Some(ch) {
            quote = None;
            continue;
        }
        if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
            continue;
        }
        let escaped = ch == '\\'
            && chars.peek().is_some_and(|ch| {
                ch.is_whitespace() || matches!(ch, '\'' | '"' | '*' | '?' | '\\')
            });
        let ch = if escaped { chars.next().unwrap() } else { ch };
        word.value.push(ch);
        if ch == '\\' || (quote.is_some() || escaped) && matches!(ch, '*' | '?') {
            word.pattern.push('\\');
        }
        word.pattern.push(ch);
    }
    if let Some(word) = word {
        out.push(word);
    }
    out
}

fn literal(pattern: &str) -> String {
    let mut chars = pattern.chars();
    let mut value = String::new();
    while let Some(ch) = chars.next() {
        value.push(if ch == '\\' {
            chars.next().unwrap_or(ch)
        } else {
            ch
        });
    }
    value
}

pub(super) fn is_glob(pattern: &str) -> bool {
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            chars.next();
        } else if matches!(ch, '*' | '?') {
            return true;
        }
    }
    false
}

pub(super) fn glob(pattern: &str, value: &str) -> bool {
    let mut tokens = Vec::new();
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        tokens.push(if ch == '\\' {
            (chars.next().unwrap_or('\\'), false)
        } else {
            (ch, matches!(ch, '*' | '?'))
        });
    }
    let mut row = vec![false; tokens.len() + 1];
    row[0] = true;
    for (i, token) in tokens.iter().enumerate() {
        row[i + 1] = row[i] && *token == ('*', true);
    }
    for ch in value.chars() {
        let mut diagonal = row[0];
        row[0] = false;
        for (i, &(token, wildcard)) in tokens.iter().enumerate() {
            let old = row[i + 1];
            row[i + 1] = if wildcard && token == '*' {
                old || row[i]
            } else {
                diagonal && (wildcard || token == ch)
            };
            diagonal = old;
        }
    }
    row[tokens.len()]
}

fn type_part(text: &str) -> (&str, Option<&str>) {
    text.rsplit_once('@')
        .map_or((text, None), |(name, kind)| (name, Some(kind)))
}

fn hint(text: &str) -> (&str, Option<u64>) {
    text.rsplit_once('#')
        .and_then(|(rest, id)| {
            (!rest.is_empty())
                .then(|| id.parse().ok().map(|id| (rest, Some(id))))
                .flatten()
        })
        .unwrap_or((text, None))
}

fn named<'a>(nodes: impl Iterator<Item = &'a Node>, text: &str, pattern: bool) -> Vec<&'a Node> {
    let nodes = nodes.collect::<Vec<_>>();
    if !pattern {
        let exact = nodes
            .iter()
            .copied()
            .filter(|node| node.name == text)
            .collect::<Vec<_>>();
        if !exact.is_empty() {
            return exact;
        }
    }
    let (name, kind) = type_part(text);
    let name = if pattern && name.is_empty() && kind.is_some() {
        "*"
    } else {
        name
    };
    nodes
        .into_iter()
        .filter(|node| {
            kind.is_none_or(|kind| kind == node.class)
                && if pattern {
                    glob(name, &node.name)
                } else {
                    name == node.name
                }
        })
        .collect()
}

fn wildcard_ref(text: &str) -> bool {
    text.split('/').any(|segment| is_glob(type_part(segment).0))
}

pub(super) fn candidates<'a>(
    nodes: &'a [Node],
    text: &str,
    semantic: &str,
    pattern: bool,
) -> Result<Vec<&'a Node>, String> {
    let (text, id_hint) = hint(text);
    if pattern && id_hint.is_some() {
        return Err("A node pattern cannot carry a UID hint".into());
    }
    if !pattern {
        if let Ok(id) = text.strip_prefix('#').unwrap_or(text).parse::<u64>() {
            return Ok(nodes
                .iter()
                .filter(|node| node.id == id && node_accepts(node, semantic))
                .collect());
        }
    }
    let mut found = if let Some((owner, renderer)) = text.split_once('/') {
        named(
            nodes.iter().filter(|node| {
                node.kind != "object"
                    && node_accepts(node, semantic)
                    && node.path.split_once('/').is_some_and(|(object, _)| {
                        if pattern {
                            glob(owner, object)
                        } else {
                            object == owner
                        }
                    })
            }),
            renderer,
            pattern,
        )
    } else {
        let objects = named(
            nodes
                .iter()
                .filter(|node| node.kind == "object" && node_accepts(node, semantic)),
            text,
            pattern,
        );
        if objects.is_empty() {
            named(
                nodes
                    .iter()
                    .filter(|node| node.kind != "object" && node_accepts(node, semantic)),
                text,
                pattern,
            )
        } else {
            objects
        }
    };
    if let Some(found_id) = id_hint.filter(|id| found.iter().any(|node| node.id == *id)) {
        found.retain(|node| node.id == found_id);
    }
    Ok(found)
}

pub(super) fn resolve<'a>(
    nodes: &'a [Node],
    text: &str,
    semantic: &str,
) -> Result<&'a Node, String> {
    let decoded = serde_json::from_str::<String>(text).ok();
    let text = decoded
        .as_deref()
        .unwrap_or_else(|| super::parser::unquote(text));
    let found = candidates(nodes, text, semantic, false)?;
    match found.as_slice() {
        [node] => Ok(*node),
        [] => Err(format!("No {semantic} named {text}")),
        _ => Err(format!(
            "Ambiguous {semantic}: {text}; use {}",
            found
                .iter()
                .map(|node| format!("#{} ({}@{})", node.id, node.path, node.class))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn property(nodes: &[Node], word: &Word) -> Result<Option<Vec<String>>, String> {
    let slash = word.pattern.find('/');
    let dots = word
        .pattern
        .char_indices()
        .filter(|(i, ch)| *ch == '.' && slash.is_none_or(|slash| *i > slash))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if slash.is_some() {
        if let Some(&dot) = dots.first() {
            let prefix = &word.pattern[..dot];
            if wildcard_ref(prefix) {
                return Ok(Some(
                    candidates(nodes, prefix, "node", true)?
                        .iter()
                        .map(|node| format!("#{}.{}", node.id, literal(&word.pattern[dot + 1..])))
                        .collect(),
                ));
            }
        }
    } else {
        for &dot in &dots {
            let prefix = &word.pattern[..dot];
            if !is_glob(prefix) && resolve(nodes, &literal(prefix), "object").is_ok() {
                return Ok(None);
            }
        }
        let mut has_pattern = false;
        for dot in dots.into_iter().rev() {
            let prefix = &word.pattern[..dot];
            if !wildcard_ref(prefix) {
                continue;
            }
            has_pattern = true;
            let found = candidates(nodes, prefix, "object", true)?;
            if !found.is_empty() {
                return Ok(Some(
                    found
                        .iter()
                        .map(|node| format!("#{}.{}", node.id, literal(&word.pattern[dot + 1..])))
                        .collect(),
                ));
            }
        }
        if has_pattern {
            return Ok(Some(Vec::new()));
        }
    }
    Ok(None)
}

pub(super) fn files(session: &Session, pattern: &str) -> Result<Vec<String>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (session, pattern);
        Err("File patterns require the native application".into())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::path::Path::new(pattern);
        let mut paths = vec![if path.is_absolute() {
            std::path::PathBuf::new()
        } else {
            session.cwd().to_owned()
        }];
        for component in path.components() {
            let std::path::Component::Normal(segment) = component else {
                for path in &mut paths {
                    path.push(component.as_os_str());
                }
                continue;
            };
            let pattern = segment.to_string_lossy();
            let mut next = Vec::new();
            for path in paths {
                if is_glob(&pattern) {
                    let mut entries = std::fs::read_dir(&path)
                        .into_iter()
                        .flatten()
                        .filter_map(Result::ok)
                        .collect::<Vec<_>>();
                    entries.sort_by_key(|entry| entry.file_name());
                    for entry in entries {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if (!name.starts_with('.') || pattern.starts_with('.'))
                            && glob(&pattern, &name)
                        {
                            next.push(entry.path());
                        }
                    }
                } else {
                    let candidate = if pattern == "~" && path == session.cwd() {
                        session.resolve_path("~")
                    } else {
                        path.join(literal(&pattern))
                    };
                    if candidate.exists() {
                        next.push(candidate);
                    }
                }
            }
            paths = next;
        }
        Ok(paths
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect())
    }
}

pub(super) fn expand(
    session: &Session,
    nodes: &[Node],
    spec: &CommandSpec,
    bound: &BTreeMap<String, String>,
) -> Result<(Vec<BTreeMap<String, String>>, Vec<Action>), String> {
    let mut result = bound.clone();
    let mut fan: Option<(String, Vec<String>)> = None;
    let mut warnings = Vec::new();
    for (key, semantic) in &spec.semantics {
        let is_file = semantic == "path" && spec.tool.starts_with("load");
        if !(is_file
            || semantic == "property path"
            || semantic == "node"
            || semantic == "renderer"
            || (semantic == "object" || semantic.ends_with(" object")))
        {
            continue;
        }
        let Some(raw) = bound
            .get(key)
            .filter(|value| !value.is_empty() && value.trim() != "null")
        else {
            continue;
        };
        let words = words(raw);
        let mut values = Vec::new();
        let mut many = words.len() > 1;
        for word in &words {
            let expanded = if is_file && is_glob(&word.pattern) {
                Some(files(session, &word.pattern)?)
            } else if semantic == "property path" {
                property(nodes, word)?
            } else if !is_file && wildcard_ref(&word.pattern) {
                Some(
                    candidates(nodes, &word.pattern, semantic, true)?
                        .iter()
                        .map(|node| format!("#{}", node.id))
                        .collect(),
                )
            } else {
                None
            };
            if let Some(expanded) = expanded {
                many = true;
                if expanded.is_empty() {
                    warnings.push(Action::Output(format!("No matches found: {}", word.value)));
                }
                values.extend(expanded);
            } else {
                values.push(word.value.clone());
            }
        }
        if many && is_file {
            values.retain(|path| {
                let scene = ["qsc", "pse"].contains(
                    &std::path::Path::new(path)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .as_str(),
                );
                if scene {
                    warnings.push(Action::Output(format!(
                        "A scene file opens on its own: {path}"
                    )));
                }
                !scene
            });
        }
        if many {
            if let Some((other, _)) = &fan {
                return Err(format!(
                    "Only one argument may name several targets: {other} and {key}"
                ));
            }
            fan = Some((key.clone(), values));
        } else if let Some(value) = values.pop() {
            result.insert(key.clone(), serde_json::json!(value).to_string());
        }
    }
    Ok((
        if let Some((key, values)) = fan {
            values
                .into_iter()
                .map(|value| {
                    let mut line = result.clone();
                    line.insert(key.clone(), serde_json::json!(value).to_string());
                    line
                })
                .collect()
        } else {
            vec![result]
        },
        warnings,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_desktop_core::DesktopAppState;
    use cuemol_qsys::Camera;

    fn apply(state: &mut DesktopAppState, session: &mut Session, source: &str) {
        let mut run = super::super::Submission::new(source, super::super::Dialect::CueMol).unwrap();
        let mut camera = Camera::new();
        while let Some(action) = run.next(session, state, &camera).unwrap() {
            if let Action::Tool { name, args } = action {
                crate::call_tool(state, &mut camera, &name, &args).unwrap();
            }
        }
    }

    #[test]
    fn native_lists_patterns_and_typed_paths_resolve_current_nodes() {
        let mut state = DesktopAppState::new();
        for name in [
            "one.pdb",
            "two.pdb",
            "literal*",
            "literalOther",
            "with space",
        ] {
            state.open_bytes("fixture.pdb", b"ATOM      1  CA  GLY A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n").unwrap();
            let id = state.active_object_summaries().last().unwrap().uid;
            state
                .scene_manager_mut()
                .object_mut(id)
                .unwrap()
                .set_name(name);
        }
        let mut session = Session::default();
        apply(
            &mut state,
            &mut session,
            "hide one.pdb two.pdb; hide literal\\*; hide \"with space\"",
        );
        assert_eq!(
            state
                .active_object_summaries()
                .iter()
                .map(|object| object.visible)
                .collect::<Vec<_>>(),
            [false, false, false, true, false]
        );
        apply(
            &mut state,
            &mut session,
            "set *.pdb.visible, true; show \"literal*\"; show with\\ space",
        );
        assert!(state
            .active_object_summaries()
            .iter()
            .all(|object| object.visible));
        apply(
            &mut state,
            &mut session,
            "create_renderer *.pdb, simple, name=wire; hide */wire@simple",
        );
        let inventory = super::super::nodes(&state).unwrap();
        let wires = inventory
            .iter()
            .filter(|node| node.name == "wire")
            .collect::<Vec<_>>();
        assert_eq!(wires.len(), 2);
        assert!(resolve(&inventory, "wire@simple", "renderer").is_err());
        assert_eq!(
            resolve(
                &inventory,
                &format!("wire@simple#{}", wires[0].id),
                "renderer"
            )
            .unwrap()
            .id,
            wires[0].id
        );
        assert!(session
            .plan(&state, "superpose one* two*, one* two*")
            .unwrap_err()
            .contains("Only one"));
        assert!(session
            .plan(&state, "hide missing*")
            .unwrap()
            .iter()
            .any(|action| matches!(action, Action::Output(value) if value.contains("No matches"))));
    }

    #[test]
    fn native_paths_keep_working_directory_metacharacters_literal_and_preserve_null() {
        let directory = tempfile::tempdir().unwrap();
        let literal = directory.path().join("data*set");
        let sibling = directory.path().join("dataXset");
        std::fs::create_dir(&literal).unwrap();
        std::fs::create_dir(&sibling).unwrap();
        std::fs::write(literal.join("one.pdb"), b"").unwrap();
        std::fs::write(sibling.join("other.pdb"), b"").unwrap();
        let mut session = Session::default();
        session.cwd = literal.clone();
        assert_eq!(
            files(&session, "*.pdb").unwrap(),
            [literal.join("one.pdb").to_string_lossy()]
        );
        let state = DesktopAppState::new();
        let actions = session
            .plan(
                &state,
                "set_node_prop nodeId=null, nodeType=scene, prop=bgcolor, value=white",
            )
            .unwrap();
        assert!(
            matches!(actions.as_slice(), [Action::Tool { args, .. }] if args["nodeId"].is_null() && args["nodeType"] == "scene")
        );
    }

    #[test]
    fn native_file_lists_preserve_spaces_and_do_not_open_scenes_in_patterns() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["first.pdb", "with space.pdb", ".hidden.pdb", "skip.qsc"] {
            std::fs::write(directory.path().join(name), "").unwrap();
        }
        let state = DesktopAppState::new();
        let mut session = Session::default();
        session.cwd = directory.path().to_owned();
        let actions = session.plan(&state, "load *.pdb skip.qsc").unwrap();
        let paths = actions
            .iter()
            .filter_map(|action| {
                if let Action::Tool { args, .. } = action {
                    args["path"].as_str()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 2);
        assert!(paths[1].ends_with("with space.pdb"));
        assert!(actions
            .iter()
            .any(|action| matches!(action, Action::Output(value) if value.contains("scene file"))));
    }
}
