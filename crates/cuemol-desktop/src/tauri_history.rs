use super::*;
use cuemol_desktop_core::undo::{ChangeSet, Direction, ValueChange};

#[derive(Clone)]
pub(super) struct ModelChanges {
    structure_revision: Option<ValueChange<u64>>,
    selection: Option<ValueChange<String>>,
    background: Option<ValueChange<String>>,
    show_unit_cell: Option<ValueChange<bool>>,
    show_symmetry_mates: Option<ValueChange<bool>>,
    show_density_contour: Option<ValueChange<bool>>,
    color_proofing: Option<ValueChange<bool>>,
    density_level: Option<ValueChange<f64>>,
    animation_elapsed: Option<ValueChange<f64>>,
}

impl ModelChanges {
    fn between(before: &AppModelUndo, after: &AppModel) -> Self {
        Self {
            structure_revision: ValueChange::between(
                &before.structure_revision,
                &after.structure_revision,
            ),
            selection: ValueChange::between(&before.selection, &after.selection),
            background: ValueChange::between(&before.background, &after.background),
            show_unit_cell: ValueChange::between(&before.show_unit_cell, &after.show_unit_cell),
            show_symmetry_mates: ValueChange::between(
                &before.show_symmetry_mates,
                &after.show_symmetry_mates,
            ),
            show_density_contour: ValueChange::between(
                &before.show_density_contour,
                &after.show_density_contour,
            ),
            color_proofing: ValueChange::between(&before.color_proofing, &after.color_proofing),
            density_level: ValueChange::between(&before.density_level, &after.density_level),
            animation_elapsed: ValueChange::between(
                &before.animation_elapsed,
                &after.animation_elapsed,
            ),
        }
    }

    fn apply(&self, model: &mut AppModel, direction: Direction) {
        if let Some(change) = &self.selection {
            change.apply(&mut model.selection, direction);
        }
        if let Some(change) = &self.background {
            change.apply(&mut model.background, direction);
        }
        if let Some(change) = &self.show_unit_cell {
            change.apply(&mut model.show_unit_cell, direction);
        }
        if let Some(change) = &self.show_symmetry_mates {
            change.apply(&mut model.show_symmetry_mates, direction);
        }
        if let Some(change) = &self.show_density_contour {
            change.apply(&mut model.show_density_contour, direction);
        }
        if let Some(change) = &self.color_proofing {
            change.apply(&mut model.color_proofing, direction);
        }
        if let Some(change) = &self.density_level {
            change.apply(&mut model.density_level, direction);
        }
        if let Some(change) = &self.animation_elapsed {
            change.apply(&mut model.animation_elapsed, direction);
        }
    }
}

impl AppModelUndoEntry {
    pub(super) fn new(
        before: AppModelUndo,
        after: &AppModel,
        scope: Option<Uid>,
        description: String,
    ) -> Self {
        let changes = Arc::new(ChangeSet::between(&before.desktop, &after.desktop));
        let scopes = changes.affected_scenes().chain(scope).collect();
        Self {
            record: cuemol_command::console::actions::Record::new(&description, "gui"),
            values: ModelChanges::between(&before, after),
            changes,
            scopes,
            description,
        }
    }

    fn touches(&self, scene: Uid) -> bool {
        self.scopes.is_empty() || self.scopes.contains(&scene) || self.changes.touches(scene)
    }

    pub(super) fn overlaps(&self, other: &Self) -> bool {
        self.scopes.is_empty() || other.scopes.is_empty() || !self.scopes.is_disjoint(&other.scopes)
    }

    fn apply(&self, model: &mut AppModel, direction: Direction) {
        let active = model.desktop.active_scene();
        self.changes.apply(&mut model.desktop, direction);
        if model
            .desktop
            .active_scene()
            .is_some_and(|scene| self.touches(scene))
        {
            self.values.apply(model, direction);
            model.animation_playing = false;
            model.animation_start_camera = None;
            model.animation_started_from_ui = false;
        }
        if active != model.desktop.active_scene() {
            model.camera = model
                .desktop
                .active_view_camera()
                .unwrap_or_else(Camera::new);
        }
        let options = model.desktop.active_view_options();
        model.trans_mms = options.trans_mms;
        model.rot_mms = options.rot_mms;
        model.swap_stereo_eyes = options.swap_stereo_eyes;
        if let Some(change) = &self.values.structure_revision {
            let (from, to) = change.values(direction);
            model.structure_revision = if model.structure_revision == *from {
                *to
            } else {
                model.structure_revision.wrapping_add(1)
            };
        }
        model.invalidate_snapshot(false);
    }
}

#[derive(Clone)]
pub(super) struct Group {
    pub(super) before: AppModelUndo,
    pub(super) record: cuemol_command::console::actions::Record,
}

impl Group {
    pub(super) fn new(before: AppModelUndo, description: &str) -> Self {
        Self {
            before,
            record: cuemol_command::console::actions::Record::new(description, "gui"),
        }
    }
}

impl AppModel {
    pub(super) fn update_grouped_undo(
        &mut self,
        before: AppModelUndo,
        record: cuemol_command::console::actions::Record,
    ) -> bool {
        if before.desktop.same_scene_undo_state(&self.desktop) && before.selection == self.selection
        {
            return false;
        }
        self.desktop.mark_scene_changes_since(&before.desktop);
        if before.selection != self.selection {
            if let Some(scene) = self.desktop.active_scene() {
                self.desktop.mark_scene_modified(scene);
            }
        }
        let mut entry = AppModelUndoEntry::new(before, self, None, record.label());
        entry.record = record;
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
    pub(super) fn history_step(
        &mut self,
        direction: Direction,
        depth: usize,
        scene: Option<Uid>,
    ) -> Result<(), String> {
        self.flush_rendering()?;
        let label = if direction == Direction::Undo {
            "Undo"
        } else {
            "Redo"
        };
        let mut description = None;
        for _ in 0..=depth {
            let stack = if direction == Direction::Undo {
                &mut self.undo_stack
            } else {
                &mut self.redo_stack
            };
            let index = stack
                .iter()
                .rposition(|entry| scene.is_none_or(|scene| entry.touches(scene)));
            let Some(index) = index else {
                break;
            };
            if stack[index + 1..]
                .iter()
                .any(|entry| stack[index].overlaps(entry))
            {
                return Err("Undo or redo the more recent changes to linked scenes first".into());
            }
            let entry = stack.remove(index);
            description = Some(entry.description.clone());
            entry.apply(self, direction);
            if direction == Direction::Undo {
                self.redo_stack.push(entry);
            } else {
                self.undo_stack.push(entry);
            }
        }
        self.status = description.map_or_else(
            || format!("Nothing to {}", label.to_lowercase()),
            |description| format!("{label}: {description}"),
        );
        Ok(())
    }
}
