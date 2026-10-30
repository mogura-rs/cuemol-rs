use super::{catalog, node_accepts, nodes, parser, Completion, Session, BUILTINS};
use cuemol_core::Uid;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use std::collections::BTreeMap;

fn decoded(text: &str) -> String {
    serde_json::from_str(text).unwrap_or_else(|_| {
        super::refs::words(text)
            .first()
            .map(|word| word.value.clone())
            .unwrap_or_default()
    })
}

fn last_word(text: &str) -> &str {
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (offset, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if quote == Some(ch) {
            quote = None;
        } else if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if quote.is_none() && ch.is_whitespace() {
            start = offset + ch.len_utf8();
        }
    }
    &text[start..]
}

fn quote(text: &str) -> String {
    if text.contains([',', ';', ' ', '"', '\'', '\\', '*', '?']) {
        serde_json::to_string(text).unwrap()
    } else {
        text.into()
    }
}

pub(super) struct Context<'a> {
    pub source: &'a str,
    pub command: &'a str,
    pub rest: &'a str,
    pub current: &'a str,
    pub typed: &'a str,
    pub key: String,
    pub bound: BTreeMap<String, String>,
}

impl<'a> Context<'a> {
    pub fn new(source: &'a str, params: &[String]) -> Self {
        let line = parser::split_groups(source, ';', true)
            .ok()
            .and_then(|parts| parts.last().copied())
            .unwrap_or(source);
        let line = line.rsplit('\n').next().unwrap_or(line).trim_start();
        let (command, rest) = parser::head(line);
        let parts = parser::split_groups(rest, ',', true).unwrap_or_default();
        let current = parts.last().copied().unwrap_or("");
        let previous = parts[..parts.len().saturating_sub(1)].join(",");
        let bound = parser::bind_pymol_arguments(params, &previous, false).unwrap_or_default();
        let (key, typed) = current
            .split_once('=')
            .filter(|(key, _)| params.iter().any(|p| p == key.trim()))
            .map_or_else(
                || {
                    (
                        params
                            .iter()
                            .find(|p| !bound.contains_key(*p))
                            .cloned()
                            .unwrap_or_default(),
                        current,
                    )
                },
                |(key, value)| (key.trim().into(), value.trim_start()),
            );
        Self {
            source,
            command,
            rest,
            current,
            typed,
            key,
            bound,
        }
    }

    pub fn offers(
        &self,
        values: impl IntoIterator<Item = String>,
        suffix: &str,
        list: bool,
    ) -> Vec<Completion> {
        let mut typed = self.typed;
        if list {
            typed = last_word(typed);
        }
        let pattern = decoded(typed);
        let offset = if typed.is_empty() {
            self.source.len()
        } else {
            self.source.trim_end().len().saturating_sub(typed.len())
        };
        let prefix = &self.source[..offset];
        let mut values = values.into_iter().collect::<Vec<_>>();
        values.sort();
        values.dedup();
        values
            .into_iter()
            .filter(|value| parser::shortcut_matches(&pattern, value))
            .map(|value| Completion {
                replacement: format!(
                    "{prefix}{}{}",
                    quote(&value),
                    if value.ends_with('.') { "" } else { suffix }
                ),
                display: value,
            })
            .collect()
    }

    pub fn parameters(&self, params: &[String]) -> Vec<Completion> {
        if self.current.contains('=') {
            return Vec::new();
        }
        let prefix = &self.source[..if self.current.is_empty() {
            self.source.len()
        } else {
            self.source
                .trim_end()
                .len()
                .saturating_sub(self.current.len())
        }];
        params
            .iter()
            .filter(|name| !self.bound.contains_key(*name) && name.starts_with(self.current))
            .map(|name| Completion {
                replacement: format!("{prefix}{name}="),
                display: format!("{name}="),
            })
            .collect()
    }

    pub fn files(&self, session: &Session, directories: bool, list: bool) -> Vec<Completion> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (session, directories, list);
            Vec::new()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let raw = if list {
                last_word(self.typed)
            } else {
                self.typed
            };
            let typed = decoded(raw);
            let offset = if raw.is_empty() {
                self.source.len()
            } else {
                self.source.trim_end().len().saturating_sub(raw.len())
            };
            let prefix = &self.source[..offset];
            if let Some(pattern) = typed
                .strip_prefix('$')
                .filter(|pattern| !pattern.contains('/'))
            {
                return std::env::vars_os()
                    .filter_map(|(name, _)| name.into_string().ok())
                    .filter(|name| name.starts_with(pattern))
                    .map(|name| Completion {
                        display: format!("${name}"),
                        replacement: format!("{prefix}${name}"),
                    })
                    .collect();
            }
            let cut = typed.rfind(['/', '\\']).map_or(0, |i| i + 1);
            let (dir, stem) = typed.split_at(cut);
            let directory = session.resolve_path(if dir.is_empty() { "." } else { dir });
            let entries = std::fs::read_dir(directory)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    let is_dir = entry.path().is_dir();
                    ((!directories || is_dir) && (!name.starts_with('.') || stem.starts_with('.')))
                        .then_some((name, is_dir))
                })
                .collect::<Vec<_>>();
            let exact = entries.iter().any(|(name, _)| name.starts_with(stem));
            let mut result = entries
                .into_iter()
                .filter(|(name, _)| {
                    if exact {
                        name.starts_with(stem)
                    } else {
                        name.to_lowercase().starts_with(&stem.to_lowercase())
                    }
                })
                .map(|(name, is_dir)| {
                    let value = format!("{dir}{name}{}", if is_dir { "/" } else { "" });
                    Completion {
                        replacement: format!(
                            "{prefix}{}{}",
                            quote(&value),
                            if list && !is_dir { " " } else { "" }
                        ),
                        display: value,
                    }
                })
                .collect::<Vec<_>>();
            result.sort_by(|a, b| a.display.cmp(&b.display));
            result
        }
    }
}

fn property_values(state: &DesktopAppState, target: &str) -> BTreeMap<String, String> {
    let inventory = nodes(state).unwrap_or_default();
    let target = decoded(target);
    if let Ok(node) = super::resolve_node(&inventory, &target, "node") {
        return crate::properties::node_property_values(
            state,
            &Camera::new(),
            node.kind,
            Some(Uid::new(node.id)),
        )
        .unwrap_or_default();
    }
    if matches!(target.as_str(), "scene" | "view" | "") {
        return crate::properties::node_property_values(
            state,
            &Camera::new(),
            if target == "view" { "view" } else { "scene" },
            None,
        )
        .unwrap_or_default();
    }
    BTreeMap::new()
}

fn property_paths(state: &DesktopAppState, pattern: &str) -> Vec<String> {
    let pattern = decoded(pattern);
    for (at, _) in pattern.match_indices('.') {
        let prefix = &pattern[..at];
        let properties = property_values(state, prefix);
        if !properties.is_empty() {
            return properties
                .keys()
                .map(|prop| format!("{prefix}.{prop}"))
                .collect();
        }
    }
    let mut candidates = property_values(state, "scene")
        .into_keys()
        .collect::<Vec<_>>();
    candidates.push("view.".into());
    candidates.extend(
        nodes(state)
            .unwrap_or_default()
            .iter()
            .map(|node| format!("{}.", node.path)),
    );
    candidates
}

pub(super) fn native(session: &Session, state: &DesktopAppState, source: &str) -> Vec<Completion> {
    let initial = Context::new(source, &[]);
    let line = source.rsplit('\n').next().unwrap_or(source).trim_start();
    if !line.contains(char::is_whitespace) && !line.starts_with('@') {
        return initial
            .offers(
                catalog()
                    .keys()
                    .cloned()
                    .chain(BUILTINS.iter().map(|s| s.to_string())),
                " ",
                false,
            )
            .into_iter()
            .filter(|c| parser::shortcut_matches(line, &c.display))
            .map(|mut c| {
                c.replacement = format!(
                    "{}{name} ",
                    &source[..source.len() - line.len()],
                    name = c.display
                );
                c
            })
            .collect();
    }
    if initial.command == "help" {
        let mut values = catalog()
            .keys()
            .cloned()
            .chain(BUILTINS.iter().map(|s| s.to_string()))
            .collect::<Vec<_>>();
        values.extend(catalog().values().map(|spec| spec.group.clone()));
        values.push("syntax".into());
        return initial.offers(values, "", false);
    }
    if let Some(path) = line.strip_prefix('@') {
        let context = Context {
            typed: path.trim_start(),
            current: path.trim_start(),
            ..initial
        };
        return context.files(session, false, false);
    }
    if matches!(initial.command, "cd" | "ls" | "run" | "open_log") {
        let params = vec![
            if matches!(initial.command, "cd" | "ls") {
                "dir"
            } else {
                "file"
            }
            .into(),
            "mode".into(),
        ];
        let context = Context::new(source, &params);
        return context.files(session, initial.command == "cd", false);
    }
    let Ok(name) = parser::lookup(initial.command, catalog().keys().map(String::as_str)) else {
        return initial.files(session, false, false);
    };
    let spec = &catalog()[name];
    let context = Context::new(source, &spec.params);
    let key = context.key.as_str();
    let semantic = spec.semantics.get(key).map(String::as_str).unwrap_or("");
    let mut results = context.parameters(&spec.params);
    if semantic == "path" {
        results.extend(context.files(session, false, spec.tool.starts_with("load")));
        return results;
    }
    let mut candidates = Vec::new();
    let list = semantic == "node"
        || semantic == "renderer"
        || semantic == "object"
        || semantic.ends_with(" object")
        || semantic == "property path";
    if list && semantic != "property path" {
        candidates.extend(
            nodes(state)
                .unwrap_or_default()
                .into_iter()
                .filter(|node| node_accepts(node, semantic))
                .map(|node| node.path),
        );
        if semantic == "node" {
            candidates.extend(["scene".into(), "view".into()]);
        }
    } else if semantic == "property path" {
        candidates.extend(property_paths(state, context.typed));
    } else if semantic == "scene" {
        candidates.extend(state.scene_summaries().into_iter().map(|scene| scene.name));
    } else if key == "prop" {
        candidates.extend(
            property_values(
                state,
                context
                    .bound
                    .get("nodeId")
                    .map(String::as_str)
                    .unwrap_or("scene"),
            )
            .into_keys(),
        );
    } else if key.to_ascii_lowercase().contains("color") {
        candidates.extend(state.active_style_names("color"));
    } else if matches!(key, "selection" | "sel") {
        candidates.extend(
            ["*", "none", "protein", "nucleic", "water"]
                .into_iter()
                .map(str::to_string),
        );
        candidates.extend(state.active_style_names("sel"));
    } else if matches!(key, "rendererType" | "type") {
        if let Some(scene) = state.active_scene() {
            let inventory = nodes(state).unwrap_or_default();
            let target = context
                .bound
                .get("objId")
                .and_then(|name| super::resolve_node(&inventory, name, "object").ok());
            let types = match target.map(|n| n.class.as_str()) {
                Some("DensityMap" | "ElePotMap") => state.density_map_renderer_setup_options(),
                Some("MolSurfObj") => state.surface_renderer_setup_options(),
                Some("LWObject") => return context.offers(["lwrend".into()], ", ", false),
                _ => state
                    .molecule_renderer_setup_options(scene)
                    .unwrap_or_default(),
            };
            candidates.extend(types.into_iter().filter(|t| !t.preset).map(|t| t.name));
        }
    }
    let schema = super::tool_schema(&spec.tool);
    candidates.extend(
        schema["properties"][key]["enum"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
            .map(str::to_string),
    );
    if schema["properties"][key]["type"] == "boolean"
        || schema["properties"][key]["type"]
            .as_array()
            .is_some_and(|types| types.iter().any(|t| t == "boolean"))
    {
        candidates.extend(["true".into(), "false".into()]);
    }
    let suffix = if list {
        " "
    } else if matches!(key, "selection" | "sel") {
        ""
    } else if spec
        .params
        .iter()
        .any(|p| p != key && !context.bound.contains_key(p))
    {
        ", "
    } else {
        ""
    };
    results.extend(context.offers(candidates, suffix, list));
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_uses_argument_roles_and_does_not_modify_the_scene() {
        let state = super::super::tests::state();
        let session = Session::default();
        let saved = state.clone();
        assert!(native(&session, &state, "set protein.vis")
            .iter()
            .any(|c| c.display == "protein.visible"));
        assert!(native(&session, &state, "set_node_prop protein, vis")
            .iter()
            .any(|c| c.display == "visible"));
        assert!(native(&session, &state, "create_renderer protein, rib")
            .iter()
            .any(|c| c.display == "ribbon"));
        assert!(native(&session, &state, "set_view fit")
            .iter()
            .any(|c| c.replacement == "set_view fitSlab="));
        assert!(native(&session, &state, "set_view fitSlab=t")
            .iter()
            .any(|c| c.display == "true"));
        assert_eq!(
            state.active_object_summaries(),
            saved.active_object_summaries()
        );
    }

    #[test]
    fn filenames_complete_quotes_case_and_directories_without_listing_hidden_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Protein one.pdb"), b"").unwrap();
        std::fs::write(dir.path().join(".hidden.pdb"), b"").unwrap();
        std::fs::create_dir(dir.path().join("models")).unwrap();
        let mut session = Session::default();
        session.cwd = dir.path().into();
        let state = DesktopAppState::new();
        let entries = native(&session, &state, "load \"protein");
        assert!(entries
            .iter()
            .any(|c| c.replacement == "load \"Protein one.pdb\" "));
        assert!(!native(&session, &state, "load ")
            .iter()
            .any(|c| c.display.contains("hidden")));
        assert!(native(&session, &state, "cd m")
            .iter()
            .any(|c| c.replacement == "cd models/"));
        assert!(!native(&session, &state, "cd P")
            .iter()
            .any(|c| c.display.ends_with("pdb")));
        assert!(native(&session, &state, "@pro")
            .iter()
            .any(|c| c.replacement == "@\"Protein one.pdb\""));
        assert!(native(&session, &state, "load first.pdb \"Protein o")
            .iter()
            .any(|c| c.replacement == "load first.pdb \"Protein one.pdb\" "));
    }
}
