use super::*;
use cuemol_command::clipboard::{self, Target};
use cuemol_desktop_core::GuiPastedNode;
use cuemol_ui::clipboard::Scope;

pub(super) enum Completion {
    Written {
        revision: u64,
        cut: Option<Target>,
        result: Result<(), String>,
    },
    Read {
        revision: u64,
        target: Option<Target>,
        text: Option<egui::Id>,
        result: Result<String, String>,
    },
}

impl App {
    fn clipboard_target(&self) -> Option<Target> {
        if self.clipboard_focus.scope == Some(Scope::Cameras) {
            return Some(Target::Camera {
                scene: self.state.active_scene()?,
                name: self.clipboard_camera.clone()?,
            });
        }
        let target = self.inspector_target.as_ref();
        match target.map(|target| target.kind.as_str()) {
            Some("renderer") => Some(Target::Renderer {
                object: self.selected?,
                renderer: Uid::new(target?.id?),
            }),
            Some("object") => Some(Target::Object(Uid::new(target?.id?))),
            _ => None,
        }
    }

    pub(super) fn clipboard_action(&mut self, ctx: &egui::Context, action: &str) {
        if self.pending_close.is_some() {
            return;
        }
        if self.clipboard_focus.text.is_some() && action != "paste" {
            let event = match action {
                "copy" => egui::Event::Copy,
                "cut" => egui::Event::Cut,
                "select-all" => edit_key(egui::Key::A, false),
                "undo" => edit_key(egui::Key::Z, false),
                "redo" => edit_key(egui::Key::Z, true),
                _ => return,
            };
            self.clipboard_focus.text_event(ctx, event);
            return;
        }
        if self.clipboard_focus.text.is_none()
            && !matches!(
                self.clipboard_focus.scope,
                Some(Scope::Tree | Scope::Cameras)
            )
        {
            return;
        }
        let Some(window) = web_sys::window() else {
            return;
        };
        let system = window.navigator().clipboard();
        let revision = self.revision;
        let target = self.clipboard_target();
        if action == "paste" {
            let text = self.clipboard_focus.text;
            let promise = system.read_text();
            spawn_local(async move {
                let result = JsFuture::from(promise)
                    .await
                    .map_err(js_error)
                    .map(|value| value.as_string().unwrap_or_default());
                enqueue(Event::Clipboard(Completion::Read {
                    revision,
                    target,
                    text,
                    result,
                }));
            });
        } else if matches!(action, "copy" | "cut") {
            let Some(target) = target else {
                return;
            };
            let text = match clipboard::copy(&self.state, &target) {
                Ok(text) => text,
                Err(error) => {
                    self.message = error.to_string();
                    return;
                }
            };
            let promise = system.write_text(&text);
            let cut = (action == "cut").then_some(target);
            spawn_local(async move {
                let result = JsFuture::from(promise).await.map_err(js_error).map(|_| ());
                enqueue(Event::Clipboard(Completion::Written {
                    revision,
                    cut,
                    result,
                }));
            });
        }
    }

    pub(super) fn clipboard_complete(&mut self, ctx: &egui::Context, completion: Completion) {
        match completion {
            Completion::Written {
                result: Err(error), ..
            }
            | Completion::Read {
                result: Err(error), ..
            } => {
                self.message = format!("Clipboard access failed: {error}. Use the keyboard clipboard shortcut if your browser requires it.");
            }
            Completion::Written {
                revision,
                cut,
                result: Ok(()),
            } => {
                self.message = "Copied selection".into();
                if let Some(target) = cut {
                    if revision != self.revision {
                        self.message = "Copied; cut cancelled because the scene changed.".into();
                        return;
                    }
                    match target {
                        Target::Object(id) => self.call(
                            "delete_node",
                            json!({"nodeType":"object","nodeId":id.get()}),
                        ),
                        Target::Renderer { renderer, .. } => self.call(
                            "delete_node",
                            json!({"nodeType":"renderer","nodeId":renderer.get()}),
                        ),
                        Target::Camera { name, .. } => {
                            self.call("delete_camera", json!({"name":name}))
                        }
                        _ => {}
                    }
                }
            }
            Completion::Read {
                revision,
                target,
                text,
                result: Ok(value),
            } => {
                if let Some(id) = text {
                    if self.clipboard_focus.text == Some(id) {
                        self.clipboard_focus
                            .text_event(ctx, egui::Event::Paste(value));
                    }
                } else if revision == self.revision && self.pending_close.is_none() {
                    self.paste_clipboard(&value, target);
                } else {
                    self.message = "Paste cancelled because the scene changed.".into();
                }
            }
        }
    }

    fn paste_clipboard(&mut self, value: &str, selected: Option<Target>) {
        let (target, target_type) = match selected {
            Some(Target::Object(id)) => (Some(id), Some("object")),
            Some(Target::Renderer { object, renderer }) => {
                let group = self
                    .state
                    .renderer_summaries(object)
                    .iter()
                    .any(|item| item.uid == renderer && item.renderer_type == "*group");
                (
                    Some(if group { renderer } else { object }),
                    Some(if group { "rendGroup" } else { "object" }),
                )
            }
            _ => (None, None),
        };
        let payload = match clipboard::decode(value) {
            Ok(payload) => payload,
            Err(error) => {
                self.message = error.to_string();
                return;
            }
        };
        let mut before = self.capture_history();
        before.record = if let Ok(xml) = std::str::from_utf8(&payload.bytes) {
            cuemol_command::console::actions::Record::operation(
                &self.state,
                "paste_node",
                &json!({"kind":payload.kind,"form":payload.form,"xml":xml,"target":target.map(Uid::get),"targetType":target_type}),
            )
        } else {
            cuemol_command::console::actions::Record::new("Paste selection", "clipboard")
        };
        match self.state.paste_clipboard_bytes(
            &payload.kind,
            &payload.form,
            &payload.bytes,
            target,
            target_type,
        ) {
            Ok(result) => {
                if self.state.same_undo_state(&before.state) {
                    self.message = "No compatible clipboard items".into();
                    return;
                }
                self.cancel_console(
                    "Scene changed outside the console; remaining commands cancelled.",
                );
                self.navigation.cancel();
                self.revision = self.revision.wrapping_add(1);
                self.history.push(before);
                if self.history.len() > 12 {
                    self.history.remove(0);
                }
                self.future.clear();
                self.dirty = true;
                match result {
                    GuiPastedNode::Object(id) => {
                        self.selected = Some(id);
                        self.open_properties("object", Some(id.get()));
                    }
                    GuiPastedNode::Renderers { uids, group } => {
                        if let Some(id) = group.or_else(|| uids.first().copied()) {
                            self.open_properties("renderer", Some(id.get()));
                        }
                    }
                    GuiPastedNode::Camera(name) => self.clipboard_camera = Some(name),
                    GuiPastedNode::Style { .. } => {}
                }
                self.message = format!("Pasted {}", payload.kind);
            }
            Err(error) => {
                self.restore_history(before);
                self.message = error.to_string();
            }
        }
    }

    pub(super) fn clipboard_keyboard(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input()
            || self.pending_close.is_some()
            || self.clipboard_focus.scope.is_none()
        {
            return;
        }
        let action = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Copy => Some(("copy", None)),
                egui::Event::Cut => Some(("cut", None)),
                egui::Event::Paste(text) => Some(("paste", Some(text.clone()))),
                _ => None,
            })
        });
        if let Some((action, text)) = action {
            ctx.input_mut(|input| {
                input.events.retain(|event| {
                    !matches!(
                        event,
                        egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)
                    )
                })
            });
            if let Some(text) = text {
                self.paste_clipboard(&text, self.clipboard_target());
            } else {
                self.clipboard_action(ctx, action);
            }
        }
    }
}

fn edit_key(key: egui::Key, shift: bool) -> egui::Event {
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
