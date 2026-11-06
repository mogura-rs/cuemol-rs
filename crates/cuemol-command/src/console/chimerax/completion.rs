use super::{catalog, colors, execute, spec, syntax, Arg};
use crate::console::{Completion, Session};
use cuemol_desktop_core::DesktopAppState;
use std::collections::BTreeSet;

fn values(arg: &Arg, state: &DesktopAppState) -> Vec<String> {
    match arg.kind.as_str() {
        "spec" => spec::selector_names(&execute::selection_names(state)),
        "color" => colors::tables().named.keys().cloned().collect(),
        "color-or-enum" => colors::tables()
            .named
            .keys()
            .cloned()
            .chain(arg.values.iter().cloned())
            .collect(),
        "bool" => vec!["true".into(), "false".into()],
        "axis" => vec!["x".into(), "y".into(), "z".into()],
        _ => arg.values.clone(),
    }
}
pub(crate) fn complete(session: &Session, state: &DesktopAppState, text: &str) -> Vec<Completion> {
    let trimmed = text.trim_start();
    if !trimmed.contains(char::is_whitespace) {
        let prefix = &text[..text.len() - trimmed.len()];
        return catalog()
            .iter()
            .filter_map(|c| c.name.split_whitespace().next())
            .filter(|name| {
                name.to_ascii_lowercase()
                    .starts_with(&trimmed.to_ascii_lowercase())
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|name| Completion {
                replacement: format!("{prefix}{name} "),
                display: name.into(),
            })
            .collect();
    }
    let Ok((command, _)) = syntax::resolve(text, catalog()) else {
        return Vec::new();
    };
    let at = text.rfind(' ').map_or(0, |i| i + 1);
    let pattern = &text[at..];
    let before = text[..at].split_whitespace().last().unwrap_or("");
    let previous = syntax::keyword(before, &command.args).ok().flatten();
    let args = if let Some(previous) = previous {
        vec![previous]
    } else {
        command.args.iter().collect()
    };
    let mut candidates = args
        .iter()
        .flat_map(|a| values(a, state))
        .collect::<BTreeSet<_>>();
    if previous.is_none() {
        candidates.extend(
            command
                .args
                .iter()
                .filter(|a| a.role != "required")
                .map(|a| a.name.clone()),
        );
    }
    let mut result = candidates
        .into_iter()
        .filter(|v| {
            v.to_ascii_lowercase()
                .starts_with(&pattern.to_ascii_lowercase())
        })
        .map(|value| Completion {
            replacement: format!("{}{value} ", &text[..at]),
            display: value,
        })
        .collect::<Vec<_>>();
    if args.iter().any(|a| a.kind == "path") {
        result.extend(files(session, &text[..at], pattern));
    }
    result.sort_by(|a, b| a.replacement.cmp(&b.replacement));
    result.dedup_by(|a, b| a.replacement == b.replacement);
    result
}
fn files(session: &Session, prefix: &str, pattern: &str) -> Vec<Completion> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (session, prefix, pattern);
        Vec::new()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let typed = pattern.trim_start_matches(['\'', '"']);
        let cut = typed.rfind(['/', '\\']).map_or(0, |i| i + 1);
        let (dir, stem) = typed.split_at(cut);
        let entries =
            std::fs::read_dir(session.resolve_path(if dir.is_empty() { "." } else { dir }))
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    (!name.starts_with('.') || stem.starts_with('.'))
                        .then(|| (name, entry.path().is_dir()))
                })
                .collect::<Vec<_>>();
        let exact = entries.iter().any(|(name, _)| name.starts_with(stem));
        entries
            .into_iter()
            .filter(|(name, _)| {
                if exact {
                    name.starts_with(stem)
                } else {
                    name.to_lowercase().starts_with(&stem.to_lowercase())
                }
            })
            .map(|(name, is_dir)| {
                let path = format!("{dir}{name}{}", if is_dir { "/" } else { "" });
                let quoted = if path.contains(char::is_whitespace) {
                    if path.contains('"') {
                        format!("'{path}'")
                    } else {
                        format!("\"{path}\"")
                    }
                } else {
                    path.clone()
                };
                Completion {
                    replacement: format!("{prefix}{quoted}{}", if is_dir { "" } else { " " }),
                    display: path,
                }
            })
            .collect()
    }
}
