use super::*;
use cuemol_desktop_core::undo::{ChangeSet, Direction, ValueChange};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub(super) struct Entry {
    pub(super) record: cuemol_command::console::actions::Record,
    changes: Arc<ChangeSet>,
    scopes: BTreeSet<cuemol_core::Uid>,
    selected_object: Option<ValueChange<Option<cuemol_core::Uid>>>,
    selected_renderer: Option<ValueChange<Option<cuemol_core::Uid>>>,
    workspace_aux_selection: Option<ValueChange<Option<WorkspaceAuxSelection>>>,
    workspace_multiple_selection: Option<ValueChange<Vec<NativeWorkspaceTarget>>>,
    selection_text: Option<ValueChange<String>>,
    last_loaded_path: Option<ValueChange<Option<PathBuf>>>,
}

impl Entry {
    pub(super) fn new(
        before: GuiUndoSnapshot,
        after: &GuiApplication,
        scope: Option<cuemol_core::Uid>,
    ) -> Self {
        let changes = Arc::new(ChangeSet::between(&before.state, &after.state));
        let scopes = changes.affected_scenes().chain(scope).collect();
        Self {
            selected_object: ValueChange::between(&before.selected_object, &after.selected_object),
            selected_renderer: ValueChange::between(
                &before.selected_renderer,
                &after.selected_renderer,
            ),
            workspace_aux_selection: ValueChange::between(
                &before.workspace_aux_selection,
                &after.workspace_aux_selection,
            ),
            workspace_multiple_selection: ValueChange::between(
                &before.workspace_multiple_selection,
                &after.workspace_multiple_selection,
            ),
            selection_text: ValueChange::between(&before.selection_text, &after.selection_text),
            last_loaded_path: ValueChange::between(
                &before.last_loaded_path,
                &after.last_loaded_path,
            ),
            record: before.record,
            changes,
            scopes,
        }
    }

    fn touches(&self, scene: cuemol_core::Uid) -> bool {
        self.scopes.is_empty() || self.scopes.contains(&scene)
    }

    pub(super) fn overlaps(&self, other: &Self) -> bool {
        self.scopes.is_empty() || other.scopes.is_empty() || !self.scopes.is_disjoint(&other.scopes)
    }

    fn apply(&self, app: &mut GuiApplication, camera: &mut CameraController, direction: Direction) {
        let active = app.state.active_scene();
        self.changes.apply(&mut app.state, direction);
        if app
            .state
            .active_scene()
            .is_some_and(|scene| self.touches(scene))
        {
            if let Some(change) = &self.selected_object {
                change.apply(&mut app.selected_object, direction);
            }
            if let Some(change) = &self.selected_renderer {
                change.apply(&mut app.selected_renderer, direction);
            }
            if let Some(change) = &self.workspace_aux_selection {
                change.apply(&mut app.workspace_aux_selection, direction);
            }
            if let Some(change) = &self.workspace_multiple_selection {
                change.apply(&mut app.workspace_multiple_selection, direction);
            }
            if let Some(change) = &self.selection_text {
                change.apply(&mut app.selection_text, direction);
            }
            if let Some(change) = &self.last_loaded_path {
                change.apply(&mut app.last_loaded_path, direction);
            }
            app.animation_playing = false;
            app.animation_elapsed = 0.;
            app.animation_start_camera = None;
            app.animation_started_from_ui = false;
        }
        if active != app.state.active_scene() {
            if let Some(view) = app.state.active_view_camera() {
                camera.apply_qsys_camera(&view);
            }
        }
        camera.read_view_options(&app.state);
        app.sync_active_scene_properties();
        app.rebuild_snapshot();
    }
}

impl GuiApplication {
    pub(super) fn update_grouped_undo(&mut self, before: GuiUndoSnapshot) -> bool {
        if before.state.same_scene_undo_state(&self.state)
            && before.selection_text == self.selection_text
        {
            return false;
        }
        self.state.mark_scene_changes_since(&before.state);
        if before.selection_text != self.selection_text {
            if let Some(scene) = self.state.active_scene() {
                self.state.mark_scene_modified(scene);
            }
        }
        let entry = Entry::new(before, self, None);
        self.redo_stack.retain(|redo| !entry.overlaps(redo));
        if let Some(index) = self
            .undo_stack
            .iter()
            .position(|existing| existing.record.id() == entry.record.id())
        {
            self.undo_stack[index] = entry;
        } else {
            self.undo_stack.push(entry);
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
        }
        true
    }

    pub(super) fn record_agent_history(&mut self, name: &str) {
        let Some(mut before) = self.agent_history.take() else {
            return;
        };
        cuemol_command::retain_animation_seed_in_undo(&mut before.state, &self.state, name);
        self.agent.changed_scene |= self.update_grouped_undo(before.clone());
        self.agent_history = Some(before);
    }

    pub(super) fn history_step(
        &mut self,
        camera: &mut CameraController,
        direction: Direction,
        scene: Option<cuemol_core::Uid>,
    ) {
        self.cancel_numeric_previews(camera);
        if !self.flush_native_render_settings(&mut camera.to_qsys_camera("__current"), 0., true) {
            return;
        }
        let label = if direction == Direction::Undo {
            "Undo"
        } else {
            "Redo"
        };
        let stack = if direction == Direction::Undo {
            &mut self.undo_stack
        } else {
            &mut self.redo_stack
        };
        let Some(index) = stack
            .iter()
            .rposition(|entry| scene.is_none_or(|scene| entry.touches(scene)))
        else {
            self.set_status(format!("Nothing to {}", label.to_lowercase()));
            return;
        };
        if stack[index + 1..]
            .iter()
            .any(|entry| stack[index].overlaps(entry))
        {
            self.set_status("Undo or redo the more recent changes to linked scenes first");
            return;
        }
        let entry = stack.remove(index);
        entry.apply(self, camera, direction);
        if direction == Direction::Undo {
            self.redo_stack.push(entry);
        } else {
            self.undo_stack.push(entry);
        }
        self.set_status(label);
    }
}
