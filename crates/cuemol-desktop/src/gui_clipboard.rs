use super::*;
use cuemol_command::clipboard::{self, Target};
use cuemol_core::Uid;
use cuemol_desktop_core::GuiPastedNode;
use cuemol_ui::clipboard::Scope;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_group_and_selected_member_removes_each_once_and_undo_restores_both() {
        let mut app = GuiApplication::new();
        let scene_uid = app.state.active_scene().unwrap();
        let object_uid = app
            .state
            .scene_manager_mut()
            .scene_mut(scene_uid)
            .unwrap()
            .add_molecule("molecule", cuemol_molstr::MolCoord::new());
        let group = app
            .state
            .create_renderer_group(object_uid, "group")
            .unwrap();
        let member = app
            .state
            .scene_manager_mut()
            .object_mut(object_uid)
            .unwrap()
            .add_renderer("simple", "lines");
        app.state
            .move_renderer_in_object(object_uid, member, group, "inside")
            .unwrap();
        app.workspace_multiple_selection = [(group, true), (member, false)]
            .into_iter()
            .map(|(renderer_uid, is_group)| NativeWorkspaceTarget::Renderer {
                scene_uid,
                owner: NativePropertyObject::Molecule(object_uid),
                renderer_uid,
                is_group,
            })
            .collect();
        app.clipboard_focus.scope = Some(Scope::Tree);
        app.clipboard_action(&egui::Context::default(), "cut");
        assert!(app.state.renderer_summaries(object_uid).is_empty());
        assert_eq!(app.status, "Cut selection");
        assert_eq!(app.undo_stack.len(), 1);
        app.undo(&mut CameraController::default());
        assert_eq!(app.state.renderer_summaries(object_uid).len(), 2);
        let payload = app.system_clipboard.payload().unwrap();
        let tree =
            cuemol_core::parse_ldom2_xml(std::str::from_utf8(&payload.bytes).unwrap()).unwrap();
        assert_eq!(tree.top().tag_name(), "renderer");
        assert_eq!(tree.top().type_name(), "simple");
    }
}

impl GuiApplication {
    pub(super) fn write_clipboard_payload(
        &mut self,
        payload: Result<crate::GuiClipboardPayload>,
    ) -> bool {
        match payload
            .map_err(|error| error.to_string())
            .and_then(|payload| self.system_clipboard.write_payload(&payload))
        {
            Ok(()) => true,
            Err(error) => {
                self.set_status(format!("Clipboard error: {error}"));
                false
            }
        }
    }

    pub(super) fn clipboard_has_kind(&mut self, kind: &str) -> bool {
        self.system_clipboard.menu_kind() == Some(kind)
    }

    fn clipboard_target(&self, target: NativeWorkspaceTarget) -> Option<Target> {
        Some(match target {
            NativeWorkspaceTarget::Object { owner, .. } => Target::Object(owner.uid()),
            NativeWorkspaceTarget::Renderer {
                owner,
                renderer_uid,
                ..
            } => Target::Renderer {
                object: owner.uid(),
                renderer: renderer_uid,
            },
            NativeWorkspaceTarget::Camera {
                scene_uid,
                camera_index,
            } => Target::Camera {
                scene: scene_uid,
                name: self
                    .state
                    .camera_summaries(scene_uid)
                    .get(camera_index)?
                    .name
                    .clone(),
            },
            NativeWorkspaceTarget::Style {
                scene_uid,
                style_index,
            } => Target::Style {
                scene: scene_uid,
                index: self
                    .state
                    .style_summaries(scene_uid)
                    .get(style_index)?
                    .local_index?,
            },
        })
    }

    pub(super) fn clipboard_action(&mut self, ctx: &egui::Context, action: &str) {
        if self.clipboard_focus.text.is_some() {
            let event = match action {
                "copy" => egui::Event::Copy,
                "cut" => egui::Event::Cut,
                "paste" => match self.system_clipboard.read() {
                    Ok(text) => egui::Event::Paste(text),
                    Err(error) => {
                        self.set_status(format!("Clipboard error: {error}"));
                        return;
                    }
                },
                "select-all" => edit_key(egui::Key::A, false),
                "undo" => edit_key(egui::Key::Z, false),
                "redo" => edit_key(egui::Key::Z, true),
                _ => return,
            };
            self.clipboard_focus.text_event(ctx, event);
            return;
        }
        if !matches!(
            self.clipboard_focus.scope,
            Some(Scope::Tree | Scope::Cameras)
        ) {
            return;
        }
        let selected = if self.workspace_multiple_selection.is_empty() {
            self.current_workspace_target()
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            self.workspace_multiple_selection.clone()
        };
        if action == "select-all" {
            self.workspace_multiple_selection = self.workspace_targets_in_tree_order();
            return;
        }
        if action == "paste" {
            let scene = selected
                .first()
                .map(|target| target.scene_uid())
                .or(self.state.active_scene());
            let Some(scene) = scene else {
                return;
            };
            let (target, kind) = match selected.first().copied() {
                Some(NativeWorkspaceTarget::Object { owner, .. }) => {
                    (Some(owner.uid()), Some("object"))
                }
                Some(NativeWorkspaceTarget::Renderer {
                    owner,
                    renderer_uid,
                    is_group,
                    ..
                }) => (
                    Some(if is_group { renderer_uid } else { owner.uid() }),
                    Some(if is_group { "rendGroup" } else { "object" }),
                ),
                _ => (None, None),
            };
            self.paste_system_clipboard(scene, None, target, kind);
            return;
        }
        if !matches!(action, "copy" | "cut") || selected.is_empty() {
            return;
        }
        let target = if selected.len() == 1 {
            self.clipboard_target(selected[0])
        } else {
            let targets: Vec<_> = selected
                .iter()
                .filter_map(|target| match target {
                    NativeWorkspaceTarget::Renderer {
                        owner,
                        renderer_uid,
                        ..
                    } => Some((owner.uid(), *renderer_uid)),
                    _ => None,
                })
                .collect();
            if targets.len() != selected.len() {
                self.set_status("Multiple copy requires renderers");
                return;
            }
            Some(Target::Renderers(targets))
        };
        let Some(target) = target else {
            return;
        };
        let result = clipboard::copy_payload(&self.state, &target)
            .map_err(|error| error.to_string())
            .and_then(|payload| self.system_clipboard.write_payload(&payload));
        if let Err(error) = result {
            self.set_status(format!("Clipboard error: {error}"));
            return;
        }
        if action == "cut" {
            let undo = self.capture_undo();
            let mut selected = selected;
            selected.sort_by_key(|target| target.delete_rank());
            selected.dedup();
            let scenes = selected
                .iter()
                .map(|target| target.scene_uid())
                .collect::<Vec<_>>();
            for target in selected {
                if let Err(error) = self.destroy_workspace_target(target) {
                    self.restore_gui_undo_state(undo);
                    self.set_error(error);
                    return;
                }
            }
            self.push_undo_snapshot(undo);
            self.modified_scenes.extend(scenes);
            self.selected_object = None;
            self.selected_renderer = None;
            self.workspace_aux_selection = None;
            self.workspace_multiple_selection.clear();
            self.rebuild_snapshot();
        }
        self.set_status(if action == "cut" {
            "Cut selection"
        } else {
            "Copied selection"
        });
    }

    pub(super) fn paste_system_clipboard(
        &mut self,
        scene: Uid,
        expected: Option<&str>,
        target: Option<Uid>,
        target_type: Option<&str>,
    ) {
        let payload = match self.system_clipboard.payload() {
            Ok(payload) => payload,
            Err(error) => {
                self.set_status(format!("Clipboard error: {error}"));
                return;
            }
        };
        if expected.is_some_and(|kind| kind != payload.kind) {
            self.set_status("Clipboard data is incompatible with this target");
            return;
        }
        let undo = self.capture_undo();
        let active = self.state.active_scene();
        if !self.state.set_active_scene(Some(scene)) {
            self.set_status("Clipboard target scene no longer exists");
            return;
        }
        let result = self.state.paste_clipboard_bytes(
            &payload.kind,
            &payload.form,
            &payload.bytes,
            target,
            target_type,
        );
        self.state.set_active_scene(active);
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.restore_gui_undo_state(undo);
                self.set_error(error);
                return;
            }
        };
        if self.state.same_undo_state(&undo.state) {
            self.set_status("No compatible clipboard items");
            return;
        }
        self.push_undo_snapshot(undo);
        self.modified_scenes.insert(scene);
        self.workspace_multiple_selection.clear();
        match result {
            GuiPastedNode::Object(id) => {
                let owner = if self.state.scene_manager().object(id).is_some() {
                    NativePropertyObject::Molecule(id)
                } else if self.state.density_map_summary(id).is_some() {
                    NativePropertyObject::DensityMap(id)
                } else if self.state.surface_summary(id).is_some() {
                    NativePropertyObject::Surface(id)
                } else {
                    NativePropertyObject::Lightweight(id)
                };
                self.apply_workspace_target(NativeWorkspaceTarget::Object {
                    scene_uid: scene,
                    owner,
                });
            }
            GuiPastedNode::Renderers { uids, group } => {
                if let Some(id) = group.or_else(|| uids.first().copied()) {
                    if let Some(object) = target.and_then(|target| {
                        if target_type == Some("rendGroup") {
                            self.workspace_targets_in_tree_order()
                                .into_iter()
                                .find_map(|item| match item {
                                    NativeWorkspaceTarget::Renderer {
                                        owner,
                                        renderer_uid,
                                        ..
                                    } if renderer_uid == target => Some(owner),
                                    _ => None,
                                })
                        } else {
                            Some(if self.state.scene_manager().object(target).is_some() {
                                NativePropertyObject::Molecule(target)
                            } else if self.state.density_map_summary(target).is_some() {
                                NativePropertyObject::DensityMap(target)
                            } else if self.state.surface_summary(target).is_some() {
                                NativePropertyObject::Surface(target)
                            } else {
                                NativePropertyObject::Lightweight(target)
                            })
                        }
                    }) {
                        self.apply_workspace_target(NativeWorkspaceTarget::Renderer {
                            scene_uid: scene,
                            owner: object,
                            renderer_uid: id,
                            is_group: group.is_some(),
                        });
                    }
                }
            }
            GuiPastedNode::Camera(name) => {
                if let Some(camera_index) = self
                    .state
                    .camera_summaries(scene)
                    .iter()
                    .position(|camera| camera.name == name)
                {
                    self.apply_workspace_target(NativeWorkspaceTarget::Camera {
                        scene_uid: scene,
                        camera_index,
                    });
                }
            }
            GuiPastedNode::Style { index, .. } => {
                if let Some(style_index) = self
                    .state
                    .style_summaries(scene)
                    .iter()
                    .position(|style| style.local_index == Some(index))
                {
                    self.apply_workspace_target(NativeWorkspaceTarget::Style {
                        scene_uid: scene,
                        style_index,
                    });
                }
            }
        }
        self.rebuild_snapshot();
        self.set_status(format!("Pasted {}", payload.kind));
    }
}

pub(super) fn edit_key(key: egui::Key, shift: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            shift,
            ..egui::Modifiers::COMMAND
        },
    }
}
