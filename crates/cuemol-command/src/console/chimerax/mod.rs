macro_rules! re {
    ($pattern:literal) => {{
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($pattern).expect("ChimeraX parser pattern"))
    }};
}

mod colors;
mod completion;
pub(super) use completion::complete;
mod analysis;
mod auto_style;
mod display;
mod execute;
mod files;
mod paint;
mod palette;
mod representation;
mod surface;
mod view;
pub use execute::Deferred;
pub(crate) use execute::State;
pub(super) use execute::{plan, resume};
mod spec;
mod syntax;
pub(super) use syntax::split_lines;

use cuemol_core::Uid;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
struct Arg {
    name: String,
    role: String,
    kind: String,
    #[serde(default)]
    default: Option<String>,
    #[serde(default)]
    values: Vec<String>,
    #[serde(default)]
    ignored: bool,
}
#[derive(Debug, Deserialize)]
struct Command {
    name: String,
    args: Vec<Arg>,
    summary: String,
}
fn catalog() -> &'static [Command] {
    static CATALOG: OnceLock<Vec<Command>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("catalog.json")).expect("ChimeraX commands")
    })
}
#[derive(Debug, Clone)]
struct Model {
    id: Uid,
    number: u32,
    name: String,
    molecule: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn grammar_keeps_registration_order_keywords_colors_and_windows_paths() {
        let names = BTreeSet::new();
        for (input, expected) in [
            ("c", "cartoon"),
            ("col", "color"),
            ("surface h", "surface hidePatches"),
            ("view n test", "view name"),
        ] {
            assert_eq!(syntax::resolve(input, catalog()).unwrap().0.name, expected);
        }
        let (command, _) = syntax::resolve("color red target a", catalog()).unwrap();
        let bound = syntax::bind(command, "color red target a", &names).unwrap();
        assert_eq!(bound["color"], "#ff0000");
        assert_eq!(bound["objects"], "");
        assert_eq!(bound["target"], "a");
        let (command, _) = syntax::resolve("open x", catalog()).unwrap();
        assert_eq!(
            syntax::bind(command, r#"open "C:\Users\x.pdb""#, &names).unwrap()["names"],
            r"C:\Users\x.pdb"
        );
        assert_eq!(syntax::split_lines("# comment\ncolor #1 red; # ignore; hide\nalias paint color $1 red; show $1\nopen C:\\molecule.pdb"),vec!["color #1 red","alias paint color $1 red; show $1","open C:\\molecule.pdb"]);
        for (input, expected) in [
            ("gray(50%)", "#808080"),
            ("rgba(255,0,0,0.5)", "#ff0000@0.5"),
            ("hsl(120deg,100%,50%)", "#00ff00"),
            ("#ffff00008000", "#ff0080"),
            ("spring green target a", "#00ff7f"),
        ] {
            assert_eq!(colors::read(input).unwrap().value(), expected);
        }
    }
}

#[cfg(test)]
mod reference_tests;

pub(super) fn help(topic: &str) -> Result<String, String> {
    if topic.trim().is_empty() {
        let mut commands = catalog().iter().collect::<Vec<_>>();
        commands.sort_by_key(|c| &c.name);
        Ok(commands
            .into_iter()
            .map(|c| format!("{:<22} {}", c.name, c.summary))
            .collect::<Vec<_>>()
            .join("\n"))
    } else {
        usage(topic)
    }
}

#[cfg(test)]
mod execution_tests;

pub(super) fn usage(topic: &str) -> Result<String, String> {
    if topic.trim().is_empty() {
        return Ok(catalog()
            .iter()
            .map(syntax::usage)
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let command = syntax::resolve(topic, catalog())?.0;
    Ok(std::iter::once(syntax::usage(command))
        .chain(
            command
                .args
                .iter()
                .filter(|a| a.role != "required")
                .map(|arg| {
                    format!(
                        "  {}: {}{}",
                        arg.name,
                        syntax::type_name(arg),
                        if arg.ignored {
                            " (not supported yet; ignored)"
                        } else {
                            ""
                        }
                    )
                }),
        )
        .collect::<Vec<_>>()
        .join("\n"))
}
