use super::*;
use cuemol_desktop_core::undo::{ChangeSet, Direction, MapChanges, ValueChange};
use std::collections::BTreeSet;

pub(super) struct Entry {
    pub(super) record: cuemol_command::console::actions::Record,
    changes: Arc<ChangeSet>,
    scopes: BTreeSet<Uid>,
    pub(super) camera_sources: BTreeSet<(Uid, String)>,
    trajectories: MapChanges<Uid, BrowserTrajectoryGroup>,
    selection: Option<ValueChange<String>>,
    selected: Option<ValueChange<Option<Uid>>>,
    frame: Option<ValueChange<usize>>,
}

impl Entry {
    fn new(before: HistoryEntry, after: &App) -> Self {
        let changes = Arc::new(ChangeSet::between(&before.state, &after.state));
        let trajectories = MapChanges::between(&before.trajectories, &after.trajectories);
        let mut scopes: BTreeSet<_> = changes.affected_scenes().collect();
        for uid in trajectories.keys() {
            scopes.extend(
                before
                    .state
                    .scene_manager()
                    .scene_uid_for_object(*uid)
                    .or_else(|| after.state.scene_manager().scene_uid_for_object(*uid)),
            );
        }
        if scopes.is_empty() {
            scopes.extend(after.state.active_scene());
        }
        let camera_sources = scopes
            .iter()
            .flat_map(|scene| {
                before
                    .state
                    .camera_summaries(*scene)
                    .into_iter()
                    .chain(after.state.camera_summaries(*scene))
                    .map(move |camera| (*scene, camera.source))
            })
            .collect();
        Self {
            selection: ValueChange::between(&before.selection, &after.selection),
            selected: ValueChange::between(&before.selected, &after.selected),
            frame: ValueChange::between(&before.frame, &after.frame),
            record: before.record,
            changes,
            scopes,
            camera_sources,
            trajectories,
        }
    }

    fn touches(&self, scene: Uid) -> bool {
        self.scopes.is_empty() || self.scopes.contains(&scene)
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.scopes.is_empty() || other.scopes.is_empty() || !self.scopes.is_disjoint(&other.scopes)
    }

    fn apply(&self, app: &mut App, direction: Direction) {
        let active = app.state.active_scene();
        self.changes.apply(&mut app.state, direction);
        self.trajectories.apply(&mut app.trajectories, direction);
        app.trajectories
            .retain(|uid, _| app.state.scene_manager().object(*uid).is_some());
        if app
            .state
            .active_scene()
            .is_some_and(|scene| self.touches(scene))
        {
            if let Some(change) = &self.selection {
                change.apply(&mut app.selection, direction);
            }
            if let Some(change) = &self.selected {
                change.apply(&mut app.selected, direction);
            }
            if let Some(change) = &self.frame {
                change.apply(&mut app.frame, direction);
            }
            app.playing = false;
        }
        if active != app.state.active_scene() {
            app.camera = app.state.active_view_camera().unwrap_or_default();
        }
        if app
            .selected
            .is_some_and(|uid| app.state.scene_manager().object(uid).is_none())
        {
            app.selected = None;
        }
        app.navigation.cancel();
        app.revision = app.revision.wrapping_add(1);
        app.dirty = true;
    }
}

impl App {
    pub(super) fn push_history(&mut self, before: HistoryEntry) {
        let entry = Entry::new(before, self);
        self.future.retain(|redo| !entry.overlaps(redo));
        if let Some(index) = self
            .history
            .iter()
            .position(|existing| existing.record.id() == entry.record.id())
        {
            self.history[index] = entry;
        } else {
            self.history.push(entry);
            if self.history.len() > 12 {
                self.history.remove(0);
            }
        }
    }

    pub(super) fn record_agent_history(&mut self, name: &str) {
        let Some(mut before) = self.agent_history.take() else {
            return;
        };
        cuemol_command::retain_animation_seed_in_undo(&mut before.state, &self.state, name);
        if !before.state.same_scene_undo_state(&self.state)
            || before.selection != self.selection
            || before.trajectories != self.trajectories
        {
            self.state.mark_scene_changes_since(&before.state);
            self.push_history(before.clone());
            self.agent.changed_scene = true;
        }
        self.agent_history = Some(before);
    }

    pub(super) fn history_step(&mut self, direction: Direction, scene: Option<Uid>) {
        let stack = if direction == Direction::Undo {
            &mut self.history
        } else {
            &mut self.future
        };
        let Some(index) = stack
            .iter()
            .rposition(|entry| scene.is_none_or(|scene| entry.touches(scene)))
        else {
            return;
        };
        if stack[index + 1..]
            .iter()
            .any(|entry| stack[index].overlaps(entry))
        {
            self.message = "Undo or redo the more recent changes to linked scenes first".into();
            return;
        }
        let entry = stack.remove(index);
        entry.apply(self, direction);
        if direction == Direction::Undo {
            self.future.push(entry);
        } else {
            self.history.push(entry);
        }
    }
}
