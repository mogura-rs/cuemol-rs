use crate::{
    clear_xml_formatting_whitespace, parse_ldom2_xml, write_ldom2_xml, CueMolError,
    DesktopAppState, GuiStyleSummary, LDom2Node, LDom2Tree, LoadedStyle, Result, Uid,
};
use cuemol_qsys::{StyleManager, StyleSet};
use std::path::{Path, PathBuf};

pub(crate) struct PreparedSceneStyles {
    pub nodes: Vec<LDom2Node>,
    writes: Vec<(PathBuf, String)>,
}

impl PreparedSceneStyles {
    pub fn write_sources(&self) -> Result<()> {
        for (path, xml) in &self.writes {
            std::fs::write(path, xml)?;
        }
        Ok(())
    }

    pub fn apply(self, scene: &mut cuemol_molstr::qsys::MolScene) {
        for (style, node) in scene.loaded_styles_mut().iter_mut().zip(self.nodes) {
            style.node = node;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuiStyleTarget {
    Global(u64),
    Scene { scene_uid: Uid, runtime_id: u64 },
}

impl GuiStyleTarget {
    pub fn runtime_id(self) -> u64 {
        match self {
            Self::Global(id) => id,
            Self::Scene { runtime_id, .. } => runtime_id,
        }
    }

    pub fn scope(self) -> Uid {
        match self {
            Self::Global(_) => Uid::new(0),
            Self::Scene { scene_uid, .. } => scene_uid,
        }
    }
}

fn clean_document(tree: &mut LDom2Tree) -> Result<()> {
    if tree.top().tag_name() != "styles" {
        return Err(CueMolError::invalid_argument(
            "style XML root must be <styles>",
        ));
    }
    for attr in ["src", "alt_src", "readonly", "modified"] {
        tree.top_mut().remove_child(attr);
    }
    clear_xml_formatting_whitespace(tree.top_mut());
    Ok(())
}

pub(crate) fn document(style: &LoadedStyle) -> Result<LDom2Tree> {
    let mut tree = if style.node.children().iter().any(|child| !child.is_attr()) {
        LDom2Tree::from(style.node.clone())
    } else {
        parse_ldom2_xml(&style.data)?
    };
    clean_document(&mut tree)?;
    if let Some(id) = style.node.find_child("id").filter(|node| node.is_attr()) {
        tree.top_mut().set_str_attr("id", id.value());
    }
    Ok(tree)
}

pub(crate) fn parse_import(xml: &str) -> Result<(LDom2Node, String)> {
    let mut tree = parse_ldom2_xml(xml)?;
    clean_document(&mut tree)?;
    if tree.top().str_attr("id").trim().is_empty() {
        return Err(CueMolError::invalid_argument("style XML must have an id"));
    }
    Ok((tree.top().clone(), write_ldom2_xml(&tree)))
}

fn normalize_palette(styles: &StyleManager, scope: u64, mut tree: LDom2Tree) -> Result<LDom2Tree> {
    let set = StyleSet::from_node_with_colors(0, "", tree.top(), |name| {
        styles.get_color(name, scope).cloned()
    })?;
    for node in tree
        .top_mut()
        .children_mut()
        .iter_mut()
        .filter(|node| !node.is_attr() && node.tag_name() == "color")
    {
        let color = set
            .get_color(node.str_attr("id").as_str())
            .ok_or_else(|| CueMolError::parse("palette color was not parsed"))?;
        node.set_value(color.script_string());
        node.retain_children(|child| {
            !child.is_attr()
                || !matches!(
                    child.tag_name(),
                    "set_h"
                        | "set_s"
                        | "set_b"
                        | "set_a"
                        | "mod_h"
                        | "mod_s"
                        | "mod_b"
                        | "mod_a"
                        | "material"
                )
        });
        if !color.material().is_empty() {
            node.set_str_attr("material", color.material());
        }
    }
    Ok(tree)
}

pub(crate) fn replace_content(
    style: &mut LoadedStyle,
    mut tree: LDom2Tree,
    read_only: bool,
    edited: bool,
) -> Result<()> {
    let mut before = document(style)?;
    clean_document(&mut tree)?;
    before
        .top_mut()
        .set_str_attr("id", tree.top().str_attr("id").as_str());
    let modified = edited
        || before.top() != tree.top()
        || style.node.str_attr("modified").equals_ignore_case("true");
    let mut node = if style.node.str_attr("src").is_empty() {
        tree.top().clone()
    } else {
        let mut node = style.node.clone();
        node.retain_children(LDom2Node::is_attr);
        node.set_str_attr("id", tree.top().str_attr("id").as_str());
        node
    };
    for (key, enabled) in [("readonly", read_only), ("modified", modified)] {
        if enabled {
            node.set_str_attr(key, "true");
        } else {
            node.remove_child(key);
        }
    }
    style.node = node;
    style.data = write_ldom2_xml(&tree);
    Ok(())
}

impl DesktopAppState {
    pub(crate) fn normalized_style_document(&self, scene: Uid, xml: &str) -> Result<LDom2Tree> {
        let scene_state = self
            .scene_manager()
            .scene(scene)
            .ok_or_else(|| CueMolError::not_found("scene"))?;
        normalize_palette(
            &self.scene_styles_in_scope(scene_state, scene.get()),
            scene.get(),
            parse_ldom2_xml(xml)?,
        )
    }

    pub(crate) fn normalize_loaded_scene_styles(&mut self, scene: Uid) -> Result<()> {
        let mut styles = self.global_styles();
        let loaded = self
            .scene_manager()
            .scene(scene)
            .ok_or_else(|| CueMolError::not_found("scene"))?
            .loaded_styles();
        let mut replacements = Vec::with_capacity(loaded.len());
        for (index, style) in loaded.iter().enumerate() {
            let tree = normalize_palette(&styles, scene.get(), document(style)?)?;
            let data = write_ldom2_xml(&tree);
            let name = if tree.top().str_attr("id").is_empty() {
                format!("scene-style-{index}")
            } else {
                tree.top().str_attr("id").to_string()
            };
            if styles.load_style_set_from_node(scene.get(), name, tree.top(), false) == 0 {
                return Err(CueMolError::parse("loaded scene style is invalid"));
            }
            replacements.push((tree, data));
        }
        let loaded = self
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .loaded_styles_mut();
        for (style, (tree, data)) in loaded.iter_mut().zip(replacements) {
            if style.node.str_attr("src").is_empty() {
                let mut node = tree.top().clone();
                for attribute in style.node.children().iter().filter(|node| node.is_attr()) {
                    node.set_str_attr(attribute.tag_name(), attribute.value());
                }
                style.node = node;
            }
            style.data = data;
        }
        Ok(())
    }

    pub(crate) fn prepare_scene_styles(
        &self,
        scene: Uid,
        embed_all: bool,
    ) -> Result<PreparedSceneStyles> {
        let mut prepared = PreparedSceneStyles {
            nodes: Vec::new(),
            writes: Vec::new(),
        };
        let styles = self
            .scene_manager()
            .scene(scene)
            .ok_or_else(|| CueMolError::not_found("scene"))?
            .loaded_styles();
        for style in styles {
            let source = style.node.str_attr("src");
            let mut node = if embed_all || source.is_empty() {
                let mut node = document(style)?.top().clone();
                for attr in style.node.children().iter().filter(|attr| {
                    attr.is_attr() && !matches!(attr.tag_name(), "src" | "alt_src" | "modified")
                }) {
                    node.set_str_attr(attr.tag_name(), attr.value());
                }
                node
            } else {
                let path = crate::absolute_scene_path(
                    &self.resolve_scene_path(scene, Path::new(source.as_str()))?,
                )?;
                let mut node = style.node.clone();
                if !node.bool_attr("readonly") {
                    prepared
                        .writes
                        .push((path.clone(), write_ldom2_xml(&document(style)?)));
                    if source.as_str() != path.to_string_lossy() {
                        node.remove_child("modified");
                    }
                }
                node.set_str_attr("src", path.to_string_lossy());
                node
            };
            if embed_all || source.is_empty() {
                node.remove_child("modified");
            }
            prepared.nodes.push(node);
        }
        Ok(prepared)
    }

    pub fn import_scene_style(&mut self, scene: Uid, xml: &str, read_only: bool) -> Result<usize> {
        let (node, data) = parse_import(xml)?;
        self.insert_imported_style(scene, node, data, read_only)
    }

    pub(crate) fn insert_imported_style(
        &mut self,
        scene: Uid,
        node: LDom2Node,
        data: String,
        read_only: bool,
    ) -> Result<usize> {
        let name = node.str_attr("id").to_string();
        let tree = self.normalized_style_document(scene, &data)?;
        let mut node = tree.top().clone();
        let data = write_ldom2_xml(&tree);
        let scene = self
            .scene_manager_mut()
            .scene_mut(scene)
            .ok_or_else(|| CueMolError::not_found("scene"))?;
        if scene
            .loaded_styles()
            .iter()
            .any(|style| style.node.str_attr("id").as_str() == name)
        {
            return Err(CueMolError::invalid_argument(format!(
                "style already exists: {name}"
            )));
        }
        if read_only {
            node.set_str_attr("readonly", "true");
        }
        Ok(scene.add_loaded_style(LoadedStyle::new(node, data)))
    }

    pub fn resolve_style_color(
        &self,
        scope: Uid,
        expression: &str,
    ) -> Result<cuemol_gfx::SolidColor> {
        let styles = if scope.get() == 0 {
            self.global_styles()
        } else {
            let scene = self
                .scene_manager()
                .scene(scope)
                .ok_or_else(|| CueMolError::not_found("scene"))?;
            self.scene_styles_in_scope(scene, scope.get())
        };
        if let Some(color) = styles.get_color(expression.trim(), scope.get()) {
            return Ok(color.clone());
        }
        cuemol_gfx::parse_color(expression)?
            .resolve(|name| styles.get_color(name, scope.get()).cloned(), None)
            .ok_or_else(|| CueMolError::invalid_argument(format!("unresolved color: {expression}")))
    }

    pub fn find_style_target(&self, scene: Uid, name: &str) -> Option<GuiStyleTarget> {
        self.scene_manager()
            .scene(scene)
            .and_then(|scene| {
                scene
                    .loaded_styles()
                    .iter()
                    .rev()
                    .find(|style| style.node.str_attr("id").as_str() == name)
            })
            .map(|style| GuiStyleTarget::Scene {
                scene_uid: scene,
                runtime_id: style.runtime_id(),
            })
            .or_else(|| {
                self.script_styles
                    .style_sets(0)
                    .iter()
                    .find(|style| style.name() == name)
                    .map(|style| GuiStyleTarget::Global(style.uid()))
            })
    }

    pub fn style_summary(&self, target: GuiStyleTarget) -> Result<GuiStyleSummary> {
        self.style_summaries(target.scope())
            .into_iter()
            .find(|style| style.target == target)
            .ok_or_else(|| CueMolError::not_found("style set"))
    }

    pub fn style_name(&self, target: GuiStyleTarget) -> Result<String> {
        match target {
            GuiStyleTarget::Global(id) => self
                .script_styles
                .style_sets(0)
                .iter()
                .find(|set| set.uid() == id)
                .map(|set| set.name().to_string()),
            GuiStyleTarget::Scene {
                scene_uid,
                runtime_id,
            } => self
                .scene_manager()
                .scene(scene_uid)
                .and_then(|scene| {
                    scene
                        .loaded_styles()
                        .iter()
                        .find(|style| style.runtime_id() == runtime_id)
                })
                .map(|style| style.node.str_attr("id").to_string()),
        }
        .ok_or_else(|| CueMolError::not_found("style set"))
    }

    pub fn set_style_read_only(&mut self, target: GuiStyleTarget, read_only: bool) -> Result<()> {
        let summary = self.style_summary(target)?;
        if read_only && !summary.read_only && summary.modified {
            return Err(CueMolError::invalid_argument(
                "modified styles cannot be made read-only",
            ));
        }
        match target {
            GuiStyleTarget::Global(id) => self
                .script_styles
                .get_style_set_mut(id)
                .unwrap()
                .set_read_only(read_only),
            GuiStyleTarget::Scene { scene_uid, .. } => {
                let node = &mut self
                    .scene_manager_mut()
                    .scene_mut(scene_uid)
                    .unwrap()
                    .loaded_styles_mut()[summary.local_index.unwrap()]
                .node;
                if read_only {
                    node.set_str_attr("readonly", "true");
                } else {
                    node.remove_child("readonly");
                }
            }
        }
        Ok(())
    }

    pub fn style_xml(&self, target: GuiStyleTarget) -> Result<String> {
        match target {
            GuiStyleTarget::Global(id) => self
                .script_styles
                .style_sets(0)
                .iter()
                .find(|set| set.uid() == id)
                .map(StyleSet::to_xml)
                .ok_or_else(|| CueMolError::not_found("global style set")),
            GuiStyleTarget::Scene {
                scene_uid,
                runtime_id,
            } => {
                let style = self
                    .scene_manager()
                    .scene(scene_uid)
                    .and_then(|scene| {
                        scene
                            .loaded_styles()
                            .iter()
                            .find(|style| style.runtime_id() == runtime_id)
                    })
                    .ok_or_else(|| CueMolError::not_found("scene style set"))?;
                Ok(write_ldom2_xml(&document(style)?))
            }
        }
    }

    pub fn set_style_properties(
        &mut self,
        target: GuiStyleTarget,
        name: &str,
        read_only: bool,
    ) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CueMolError::invalid_argument("style name is required"));
        }
        let summary = self.style_summary(target)?;
        if read_only && !summary.read_only && summary.modified {
            return Err(CueMolError::invalid_argument(
                "modified styles cannot be made read-only",
            ));
        }
        let xml = self.style_xml(target)?;
        match target {
            GuiStyleTarget::Global(id) => {
                self.replace_global_style(id, name, read_only, &xml, false)
            }
            GuiStyleTarget::Scene { scene_uid, .. } => self.update_scene_style(
                scene_uid,
                summary.local_index.unwrap(),
                name,
                read_only,
                &xml,
            ),
        }
    }

    pub fn edit_style(&mut self, target: GuiStyleTarget, xml: &str) -> Result<()> {
        let summary = self.style_summary(target)?;
        if summary.read_only {
            return Err(CueMolError::invalid_argument("style set is read-only"));
        }
        match target {
            GuiStyleTarget::Global(id) => {
                let name = self.style_name(target)?;
                self.replace_global_style(id, &name, false, xml, true)
            }
            GuiStyleTarget::Scene { scene_uid, .. } => {
                self.edit_scene_style(scene_uid, summary.local_index.unwrap(), xml)
            }
        }
    }

    fn replace_global_style(
        &mut self,
        id: u64,
        name: &str,
        read_only: bool,
        xml: &str,
        edited: bool,
    ) -> Result<()> {
        let existing = self.script_styles.has_style_set(name, 0);
        if existing != 0 && existing != id {
            return Err(CueMolError::invalid_argument(format!(
                "style already exists: {name}"
            )));
        }
        let old = self
            .script_styles
            .style_sets(0)
            .iter()
            .find(|set| set.uid() == id)
            .ok_or_else(|| CueMolError::not_found("global style set"))?;
        let mut replacement = StyleSet::from_xml_with_colors(id, name, xml, |key| {
            self.script_styles.get_color(key, 0).cloned()
        })?;
        replacement.set_source(old.source());
        replacement.set_read_only(read_only);
        replacement.set_modified(edited || old.modified());
        *self.script_styles.get_style_set_mut(id).unwrap() = replacement;
        Ok(())
    }

    pub fn save_style_file(
        &mut self,
        target: GuiStyleTarget,
        scene: Uid,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        let summary = self.style_summary(target)?;
        let path = path.as_ref();
        match target {
            GuiStyleTarget::Scene { scene_uid, .. } => {
                self.save_scene_style_file(scene_uid, summary.local_index.unwrap(), path)
            }
            GuiStyleTarget::Global(id) => {
                let resolved = crate::absolute_scene_path(&self.resolve_scene_path(scene, path)?)?;
                std::fs::write(&resolved, self.style_xml(target)?)?;
                let set = self.script_styles.get_style_set_mut(id).unwrap();
                if set.source() != resolved.to_string_lossy() {
                    set.set_source(resolved.to_string_lossy());
                    set.set_modified(false);
                }
                Ok(())
            }
        }
    }

    pub fn reload_style_file(&mut self, target: GuiStyleTarget, scene: Uid) -> Result<()> {
        let summary = self.style_summary(target)?;
        match target {
            GuiStyleTarget::Scene { scene_uid, .. } => {
                self.reload_scene_style_file(scene_uid, summary.local_index.unwrap())
            }
            GuiStyleTarget::Global(id) => {
                if summary.source.is_empty() {
                    return Err(CueMolError::invalid_argument(
                        "style is not linked to a file",
                    ));
                }
                let data = std::fs::read_to_string(
                    self.resolve_scene_path(scene, Path::new(&summary.source))?,
                )?;
                let tree = parse_ldom2_xml(&data)?;
                if tree.top().str_attr("id").is_empty() {
                    return Err(CueMolError::invalid_argument(
                        "external style XML must have an id",
                    ));
                }
                self.replace_global_style(
                    id,
                    tree.top().str_attr("id").as_str(),
                    summary.read_only,
                    &data,
                    false,
                )?;
                self.script_styles
                    .get_style_set_mut(id)
                    .unwrap()
                    .set_modified(false);
                Ok(())
            }
        }
    }

    pub fn delete_style(&mut self, target: GuiStyleTarget) -> Result<bool> {
        let summary = self.style_summary(target)?;
        match target {
            GuiStyleTarget::Global(id) => Ok(self.script_styles.destroy_style_set(0, id)),
            GuiStyleTarget::Scene { scene_uid, .. } => {
                self.destroy_scene_style(scene_uid, summary.local_index.unwrap())
            }
        }
    }

    /// Apply a content edit while retaining the style's file link and identity.
    pub fn edit_scene_style(&mut self, scene: Uid, index: usize, xml: &str) -> Result<()> {
        let mut tree = self.normalized_style_document(scene, xml)?;
        let style = self
            .scene_manager_mut()
            .scene_mut(scene)
            .and_then(|scene| scene.loaded_styles_mut().get_mut(index))
            .ok_or_else(|| CueMolError::not_found(format!("style {index}")))?;
        if style.node.str_attr("readonly").equals_ignore_case("true") {
            return Err(CueMolError::invalid_argument("style set is read-only"));
        }
        tree.top_mut()
            .set_str_attr("id", style.node.str_attr("id").as_str());
        replace_content(style, tree, false, true)
    }
}
