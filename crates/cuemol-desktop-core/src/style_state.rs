use crate::{parse_ldom2_xml, write_ldom2_xml, LoadedStyle, Result};
use crate::{CueMolError, DesktopAppState, GuiStyleTarget, MolScene, StyleManager};
use cuemol_qsys::StyleSet;

fn loaded_name(style: &cuemol_molstr::LoadedStyle, index: usize) -> String {
    let name = style.node.str_attr("id");
    if name.is_empty() {
        format!("scene-style-{index}")
    } else {
        name.to_string()
    }
}

impl DesktopAppState {
    pub(crate) fn runtime_style_set_id(
        &self,
        styles: &StyleManager,
        target: GuiStyleTarget,
    ) -> Result<u64> {
        let scope = target.scope().get();
        let uid = match target {
            GuiStyleTarget::Global(id) => id,
            GuiStyleTarget::Scene {
                scene_uid,
                runtime_id,
            } => {
                let (index, loaded) = self
                    .scene_manager()
                    .scene(scene_uid)
                    .and_then(|scene| {
                        scene
                            .loaded_styles()
                            .iter()
                            .enumerate()
                            .find(|(_, loaded)| loaded.runtime_id() == runtime_id)
                    })
                    .ok_or_else(|| CueMolError::not_found("scene style"))?;
                self.script_style_bindings
                    .get(&scope)
                    .and_then(|bindings| bindings.get(&runtime_id))
                    .copied()
                    .unwrap_or_else(|| styles.has_style_set(&loaded_name(loaded, index), scope))
            }
        };
        styles
            .style_sets(scope)
            .iter()
            .find(|set| set.uid() == uid)
            .map(StyleSet::uid)
            .ok_or_else(|| CueMolError::not_found("style set"))
    }

    pub(crate) fn global_styles(&self) -> StyleManager {
        self.script_styles.global_scope()
    }

    pub(crate) fn scene_styles_in_scope(&self, scene: &MolScene, scope: u64) -> StyleManager {
        let mut styles = self.build_script_styles();
        if scope == 0 {
            styles.project_scene_scope(scene.uid().get());
        }
        styles
    }

    pub(crate) fn scene_styles(&self, scene: &MolScene) -> StyleManager {
        self.scene_styles_in_scope(scene, cuemol_molvis::DEFAULT_STYLE_SCOPE)
    }

    pub(crate) fn build_script_styles(&self) -> StyleManager {
        let mut styles = self.script_styles.clone();
        for scene in self.scene_manager().scenes() {
            let scope = scene.uid().get();
            let previous = styles.style_sets(scope).to_vec();
            let mut rebuilt = previous
                .iter()
                .filter(|set| {
                    !self
                        .script_style_bindings
                        .get(&scope)
                        .is_some_and(|bindings| bindings.values().any(|uid| *uid == set.uid()))
                })
                .cloned()
                .collect::<Vec<_>>();
            for (index, loaded) in scene.loaded_styles().iter().enumerate() {
                let name = loaded_name(loaded, index);
                let uid = self
                    .script_style_bindings
                    .get(&scope)
                    .and_then(|bindings| bindings.get(&loaded.runtime_id()))
                    .copied()
                    .or_else(|| {
                        previous
                            .iter()
                            .find(|set| {
                                set.name() == name
                                    && !self.script_style_bindings.get(&scope).is_some_and(
                                        |bindings| bindings.values().any(|uid| *uid == set.uid()),
                                    )
                            })
                            .map(StyleSet::uid)
                    })
                    .unwrap_or(0);
                let old = previous.iter().find(|set| set.uid() == uid);
                let mut set = if let Some(old) = old.filter(|old| old.to_xml() == loaded.data) {
                    old.clone()
                } else {
                    let Ok(tree) = parse_ldom2_xml(&loaded.data) else {
                        continue;
                    };
                    let colors = |name: &str| styles.resolve_color(name, scope).ok().flatten();
                    let parsed = if let Some(old) = old {
                        StyleSet::from_node_editing(uid, &name, tree.top(), old, colors)
                    } else {
                        StyleSet::from_node_with_colors(uid, &name, tree.top(), colors)
                    };
                    let Ok(set) = parsed else {
                        continue;
                    };
                    set
                };
                if set.name() != name {
                    set.set_name(&name);
                }
                set.set_source(loaded.node.str_attr("src").to_string());
                set.set_read_only(loaded.node.str_attr("readonly").equals_ignore_case("true"));
                set.set_modified(loaded.node.str_attr("modified").equals_ignore_case("true"));
                rebuilt.retain(|set| set.name() != name);
                rebuilt.insert(0, set);
                styles.replace_scope_style_sets(scope, rebuilt.clone());
                rebuilt = styles.style_sets(scope).to_vec();
            }
            styles.replace_scope_style_sets(scope, rebuilt);
        }
        if !styles.same_definition_state(&self.script_styles, true) {
            styles.invalidate_color_cache();
        }
        styles
    }

    pub(crate) fn sync_script_styles(&mut self, previous: &StyleManager) -> Result<()> {
        for scene_uid in self.scene_manager().scene_uids() {
            let scope = scene_uid.get();
            let before = previous.style_sets(scope);
            let after = self.script_styles.style_sets(scope);
            if before != after {
                let scene = self.scene_manager().scene(scene_uid).unwrap();
                let bindings = self.script_style_bindings.get(&scope);
                let loaded =
                    after
                        .iter()
                        .rev()
                        .map(|set| {
                            let old = before.iter().find(|old| old.uid() == set.uid());
                            let original = scene.loaded_styles().iter().enumerate().find_map(
                                |(index, loaded)| {
                                    let bound = bindings
                                        .and_then(|values| values.get(&loaded.runtime_id()))
                                        .copied();
                                    (bound == Some(set.uid())
                                        || (bound.is_none()
                                            && loaded_name(loaded, index) == set.name()))
                                    .then_some(loaded)
                                },
                            );
                            if old == Some(set) {
                                if let Some(loaded) = original {
                                    return Ok((loaded.clone(), set.uid()));
                                }
                            }
                            let mut data = set.to_xml();
                            if original.is_some_and(|loaded| loaded.node.str_attr("id").is_empty())
                                && old.is_some_and(|old| old.name() == set.name())
                            {
                                let mut tree = parse_ldom2_xml(&data)?;
                                tree.top_mut().remove_child("id");
                                data = write_ldom2_xml(&tree);
                            }
                            let mut node = if set.source().is_empty() {
                                parse_ldom2_xml(&data)?.top().clone()
                            } else {
                                let mut node = crate::LDom2Node::new();
                                node.set_tag_name("styles");
                                node.set_str_attr("id", set.name());
                                node.set_str_attr("src", set.source());
                                node
                            };
                            if let Some(original) = original {
                                for attr in original.node.children().iter().filter(|child| {
                                    child.is_attr()
                                        && !matches!(
                                            child.tag_name(),
                                            "id" | "src" | "readonly" | "modified"
                                        )
                                }) {
                                    if node.find_child(attr.tag_name()).is_none() {
                                        node.append_child_node(attr.clone());
                                    }
                                }
                            }
                            if set.read_only() {
                                node.set_str_attr("readonly", "true");
                            }
                            if set.modified() {
                                node.set_str_attr("modified", "true");
                            }
                            let loaded = if let Some(original) = original {
                                let mut loaded = original.clone();
                                loaded.node = node;
                                loaded.data = data;
                                loaded
                            } else {
                                LoadedStyle::new(node, data)
                            };
                            Ok((loaded, set.uid()))
                        })
                        .collect::<Result<Vec<_>>>()?;
                self.script_style_bindings.insert(
                    scope,
                    loaded
                        .iter()
                        .map(|(loaded, uid)| (loaded.runtime_id(), *uid))
                        .collect(),
                );
                *self
                    .scene_manager_mut()
                    .scene_mut(scene_uid)
                    .unwrap()
                    .loaded_styles_mut() = loaded.into_iter().map(|(loaded, _)| loaded).collect();
            }
        }
        self.remember_script_style_bindings();
        Ok(())
    }

    pub(crate) fn remember_script_style_bindings(&mut self) {
        self.script_style_bindings = self
            .scene_manager()
            .scenes()
            .iter()
            .map(|scene| {
                let scope = scene.uid().get();
                let bindings = scene
                    .loaded_styles()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, loaded)| {
                        let uid = self
                            .script_style_bindings
                            .get(&scope)
                            .and_then(|bindings| bindings.get(&loaded.runtime_id()))
                            .copied()
                            .unwrap_or_else(|| {
                                self.script_styles
                                    .has_style_set(&loaded_name(loaded, index), scope)
                            });
                        (uid != 0).then_some((loaded.runtime_id(), uid))
                    })
                    .collect();
                (scope, bindings)
            })
            .collect();
    }
}
