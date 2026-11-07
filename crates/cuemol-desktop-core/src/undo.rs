//! Document changes shared by application undo histories.

use super::*;
use cuemol_molstr::qsys::MolView;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Undo,
    Redo,
}

/// A recorded field change; fields outside the edit remain untouched.
#[derive(Debug, Clone)]
pub struct ValueChange<T> {
    before: T,
    after: T,
}

impl<T: Clone + PartialEq> ValueChange<T> {
    pub fn between(before: &T, after: &T) -> Option<Self> {
        (before != after).then(|| Self {
            before: before.clone(),
            after: after.clone(),
        })
    }

    pub fn values(&self, direction: Direction) -> (&T, &T) {
        match direction {
            Direction::Undo => (&self.after, &self.before),
            Direction::Redo => (&self.before, &self.after),
        }
    }

    pub fn apply(&self, current: &mut T, direction: Direction) {
        current.clone_from(self.values(direction).1);
    }
}

#[derive(Debug, Clone)]
pub struct MapChanges<K, V>(BTreeMap<K, ValueChange<Option<V>>>);

impl<K: Clone + Ord, V: Clone + PartialEq> MapChanges<K, V> {
    pub fn between(before: &BTreeMap<K, V>, after: &BTreeMap<K, V>) -> Self {
        Self(
            before
                .keys()
                .chain(after.keys())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter_map(|key| {
                    ValueChange::between(&before.get(key).cloned(), &after.get(key).cloned())
                        .map(|change| (key.clone(), change))
                })
                .collect(),
        )
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.0.keys()
    }

    pub fn apply(&self, current: &mut BTreeMap<K, V>, direction: Direction) {
        for (key, change) in &self.0 {
            if let Some(value) = change.values(direction).1 {
                current.insert(key.clone(), value.clone());
            } else {
                current.remove(key);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct SceneChange {
    uid: Uid,
    before: Option<Arc<DesktopSceneSnapshot>>,
    after: Option<Arc<DesktopSceneSnapshot>>,
    before_index: usize,
    after_index: usize,
    before_views: Vec<MolView>,
    after_views: Vec<MolView>,
    before_active_view: Option<Uid>,
    after_active_view: Option<Uid>,
    before_modified: bool,
    after_modified: bool,
}

/// Retains only changed scenes and shared immutable object storage.
#[derive(Debug, Clone)]
pub struct ChangeSet {
    scenes: Vec<SceneChange>,
    active_scene: Option<ValueChange<Option<Uid>>>,
    runtime: MapChanges<String, String>,
    surface_potential: Option<ValueChange<Option<DensityMap>>>,
    potential_scale: Option<ValueChange<Option<f64>>>,
    #[cfg(feature = "scripting")]
    script_history: MapChanges<u64, ScriptSceneUndoState>,
}

impl ChangeSet {
    pub fn between(before: &DesktopAppState, after: &DesktopAppState) -> Self {
        let scene_ids: BTreeSet<_> = before
            .scene_manager()
            .scene_uids()
            .into_iter()
            .chain(after.scene_manager().scene_uids())
            .collect();
        let scenes = scene_ids
            .into_iter()
            .filter_map(|uid| {
                let previous = before.capture_script_scene_snapshot(uid);
                let next = after.capture_script_scene_snapshot(uid);
                let before_modified = before.modified_scenes.contains(&uid);
                let after_modified = after.modified_scenes.contains(&uid);
                if previous == next && before_modified == after_modified {
                    return None;
                }
                let lifecycle = previous.is_none() || next.is_none();
                let index = |state: &DesktopAppState| {
                    state
                        .scene_manager()
                        .scenes()
                        .iter()
                        .position(|scene| scene.uid() == uid)
                        .unwrap_or(0)
                };
                let views = |state: &DesktopAppState| {
                    if lifecycle {
                        state
                            .command_manager()
                            .views()
                            .iter()
                            .filter(|view| view.scene_uid() == uid)
                            .cloned()
                            .collect()
                    } else {
                        Vec::new()
                    }
                };
                let active = |state: &DesktopAppState| {
                    state
                        .scene_manager()
                        .scene(uid)
                        .and_then(MolScene::active_view_uid)
                };
                Some(SceneChange {
                    uid,
                    before: previous.map(Arc::new),
                    after: next.map(Arc::new),
                    before_index: index(before),
                    after_index: index(after),
                    before_views: views(before),
                    after_views: views(after),
                    before_active_view: active(before),
                    after_active_view: active(after),
                    before_modified,
                    after_modified,
                })
            })
            .collect();
        Self {
            scenes,
            active_scene: ValueChange::between(&before.active_scene, &after.active_scene),
            runtime: MapChanges::between(&before.runtime, &after.runtime),
            surface_potential: ValueChange::between(
                &before.surface_potential,
                &after.surface_potential,
            ),
            potential_scale: ValueChange::between(&before.potential_scale, &after.potential_scale),
            #[cfg(feature = "scripting")]
            script_history: MapChanges::between(
                &before.script_scene_undo,
                &after.script_scene_undo,
            ),
        }
    }

    pub fn affected_scenes(&self) -> impl Iterator<Item = Uid> + '_ {
        let mut scenes = BTreeSet::new();
        scenes.extend(self.scenes.iter().map(|scene| scene.uid));
        #[cfg(feature = "scripting")]
        scenes.extend(self.script_history.0.keys().copied().map(Uid::new));
        scenes.into_iter()
    }

    pub fn touches(&self, scene: Uid) -> bool {
        self.scenes.iter().any(|change| change.uid == scene) || self.script_history_touches(scene)
    }

    fn script_history_touches(&self, scene: Uid) -> bool {
        #[cfg(feature = "scripting")]
        {
            self.script_history.0.contains_key(&scene.get())
        }
        #[cfg(not(feature = "scripting"))]
        {
            let _ = scene;
            false
        }
    }

    pub fn apply(&self, state: &mut DesktopAppState, direction: Direction) {
        let active = state.active_scene;
        for change in &self.scenes {
            change.apply(state, direction);
        }
        self.runtime.apply(&mut state.runtime, direction);
        if let Some(change) = &self.surface_potential {
            change.apply(&mut state.surface_potential, direction);
        }
        if let Some(change) = &self.potential_scale {
            change.apply(&mut state.potential_scale, direction);
        }
        #[cfg(feature = "scripting")]
        self.script_history
            .apply(&mut state.script_scene_undo, direction);
        if let Some(change) = &self.active_scene {
            let (from, to) = change.values(direction);
            if active == *from
                || active.is_none_or(|uid| state.scene_manager().scene(uid).is_none())
            {
                state.active_scene = to
                    .filter(|uid| state.scene_manager().scene(*uid).is_some())
                    .or_else(|| state.scene_manager().scene_uids().first().copied());
            }
        }
        state.gpu_pick_image = None;
        state.remember_script_style_bindings();
    }
}

impl SceneChange {
    fn apply(&self, state: &mut DesktopAppState, direction: Direction) {
        let (from, to, index, views, active_view, from_modified, to_modified) = match direction {
            Direction::Undo => (
                &self.after,
                &self.before,
                self.before_index,
                &self.before_views,
                self.before_active_view,
                self.after_modified,
                self.before_modified,
            ),
            Direction::Redo => (
                &self.before,
                &self.after,
                self.after_index,
                &self.after_views,
                self.after_active_view,
                self.before_modified,
                self.after_modified,
            ),
        };
        let extra_changes = from.as_ref().is_some_and(|from| {
            from_modified
                && !to_modified
                && state
                    .capture_script_scene_snapshot(self.uid)
                    .is_some_and(|mut current| {
                        let mut expected = from.as_ref().clone();
                        expected.scene.preserve_allocated_ids_from(&current.scene);
                        current.scene.preserve_allocated_ids_from(&expected.scene);
                        current != expected
                    })
        });
        match (from, to) {
            (Some(from), Some(to)) => state.apply_script_scene_snapshot_changes(from, to),
            (Some(_), None) => {
                state.destroy_scene(self.uid);
            }
            (None, Some(to)) => {
                let mut scene = to.scene.clone();
                scene.set_active_view_uid(active_view);
                if state.scene_manager_mut().restore_scene(scene, index) {
                    state.morphs.extend(to.morphs.clone());
                    state.density_maps.extend(to.density_maps.clone());
                    state.trajectories.extend(to.trajectories.clone());
                    state
                        .script_styles
                        .apply_scope_undo_changes(self.uid.get(), &[], &to.styles);
                    let mut restored = state.command_manager().views().to_vec();
                    restored.extend(views.iter().cloned());
                    state.command_manager_mut().replace_views(restored);
                }
            }
            (None, None) => {}
        }
        if state.scene_manager().scene(self.uid).is_some() {
            if to_modified || extra_changes {
                state.modified_scenes.insert(self.uid);
            } else if from_modified != to_modified {
                state.modified_scenes.remove(&self.uid);
            }
        }
        let map_ids: BTreeSet<_> = from
            .iter()
            .chain(to.iter())
            .flat_map(|snapshot| snapshot.density_maps.keys().copied())
            .collect();
        state
            .density_view_updates
            .retain(|uid, _| !map_ids.contains(uid));
        state
            .density_view_boxes
            .retain(|uid, _| !map_ids.contains(uid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_render(state: &mut DesktopAppState, scene: Uid, key: &str, value: &str) {
        state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .app_data_mut()
            .get_or_create("render", "RenderSettings")
            .unwrap()
            .unwrap()
            .set(key, value)
            .unwrap();
    }

    fn render(state: &DesktopAppState, scene: Uid, key: &str) -> String {
        state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .app_data()
            .get("render")
            .cloned()
            .unwrap_or_default()
            .get(key)
            .unwrap()
            .into()
    }

    #[test]
    fn scene_changes_preserve_other_scenes_unrecorded_fields_and_live_views() {
        let mut state = DesktopAppState::new();
        let first = state.new_scene("First", true).result_scene;
        let second = state.new_scene("Second", true).result_scene;
        let before = state.clone();
        set_render(&mut state, first, "width", "32");
        state.mark_scene_modified(first);
        let changes = ChangeSet::between(&before, &state);
        assert_eq!(changes.affected_scenes().collect::<Vec<_>>(), [first]);
        set_render(&mut state, first, "height", "44");
        state
            .scene_manager_mut()
            .scene_mut(second)
            .unwrap()
            .set_name("Other edit");
        state.set_active_scene(Some(second));
        state.put_config_value("test:preference", "new");
        let views = state.view_summaries();
        let camera = state.active_view_camera();
        changes.apply(&mut state, Direction::Undo);
        assert_eq!(render(&state, first, "width"), "1200");
        assert_eq!(render(&state, first, "height"), "44");
        assert_eq!(
            state.scene_manager().scene(second).unwrap().name(),
            "Other edit"
        );
        assert_eq!(state.active_scene(), Some(second));
        assert_eq!(state.active_view_camera(), camera);
        assert_eq!(state.view_summaries(), views);
        assert_eq!(state.config_value("test:preference"), "new");
        assert!(state.scene_is_modified(first));
        changes.apply(&mut state, Direction::Redo);
        assert_eq!(render(&state, first, "width"), "32");
        assert_eq!(render(&state, first, "height"), "44");
    }

    #[test]
    fn undoing_all_creations_clears_modified_without_reusing_object_ids() {
        let mut state = DesktopAppState::new();
        let scene = state.new_scene("Scene", true).result_scene;
        let mut edits = Vec::new();
        let mut last_uid = Uid::new(0);
        for name in ["First", "Second"] {
            let before = state.clone();
            last_uid = state
                .scene_manager_mut()
                .scene_mut(scene)
                .unwrap()
                .add_molecule(name, cuemol_molstr::MolCoord::new());
            state.mark_scene_modified(scene);
            edits.push(ChangeSet::between(&before, &state));
        }
        for edit in edits.iter().rev() {
            edit.apply(&mut state, Direction::Undo);
        }
        assert!(state
            .scene_manager()
            .scene(scene)
            .unwrap()
            .objects()
            .is_empty());
        assert!(!state.scene_is_modified(scene));
        let next = state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("Third", cuemol_molstr::MolCoord::new());
        assert!(next.get() > last_uid.get());
    }

    #[test]
    fn undoing_renderer_creations_clears_modified_and_keeps_allocation_limits() {
        let mut state = DesktopAppState::new();
        let scene = state.new_scene("Scene", true).result_scene;
        let object = state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule("Molecule", cuemol_molstr::MolCoord::new());
        let mut edits = Vec::new();
        let mut last_uid = Uid::new(0);
        for name in ["First", "Second"] {
            let before = state.clone();
            last_uid = state
                .scene_manager_mut()
                .object_mut(object)
                .unwrap()
                .add_renderer("simple", name);
            state.mark_scene_modified(scene);
            edits.push(ChangeSet::between(&before, &state));
        }
        for edit in edits.iter().rev() {
            edit.apply(&mut state, Direction::Undo);
        }
        assert!(state
            .scene_manager()
            .object(object)
            .unwrap()
            .renderers()
            .is_empty());
        assert!(!state.scene_is_modified(scene));
        let next = state
            .scene_manager_mut()
            .object_mut(object)
            .unwrap()
            .add_renderer("simple", "Third");
        assert!(next.get() > last_uid.get());
    }

    #[test]
    fn scene_creation_and_deletion_restore_views_without_reusing_ids_or_losing_later_scenes() {
        let mut state = DesktopAppState::new();
        let first = state.new_scene("First", true).result_scene;
        let before = state.clone();
        let second = state.new_scene("Second", true).result_scene;
        let second_view = state
            .scene_manager()
            .scene(second)
            .unwrap()
            .active_view_uid();
        let creation = ChangeSet::between(&before, &state);
        let third = state.new_scene("Third", true).result_scene;
        let views = state.view_summaries();
        creation.apply(&mut state, Direction::Undo);
        assert_eq!(state.scene_manager().scene_uids(), [first, third]);
        assert_eq!(state.active_scene(), Some(third));
        assert_eq!(state.view_summaries().len(), 2);
        creation.apply(&mut state, Direction::Redo);
        assert_eq!(state.scene_manager().scene_uids(), [first, second, third]);
        assert_eq!(state.active_scene(), Some(third));
        assert_eq!(
            state
                .scene_manager()
                .scene(second)
                .unwrap()
                .active_view_uid(),
            second_view
        );
        assert_eq!(state.view_summaries().len(), views.len());
        assert!(state.new_scene("Fourth", true).result_scene.get() > third.get());
        let before = state.clone();
        state.destroy_scene(second);
        let deletion = ChangeSet::between(&before, &state);
        deletion.apply(&mut state, Direction::Undo);
        assert!(state.scene_manager().scene(second).is_some());
        assert!(state.command_manager().view(second_view.unwrap()).is_some());
        deletion.apply(&mut state, Direction::Redo);
        assert!(state.scene_manager().scene(second).is_none());
        assert!(state.command_manager().view(second_view.unwrap()).is_none());
        assert_eq!(state.scene_manager().scenes().len(), 3);
    }

    #[test]
    fn multi_scene_edits_remain_atomic_and_skip_later_unrecorded_scene_deletions() {
        let mut state = DesktopAppState::new();
        let first = state.new_scene("First", true).result_scene;
        let second = state.new_scene("Second", true).result_scene;
        let before = state.clone();
        set_render(&mut state, first, "width", "32");
        set_render(&mut state, second, "height", "24");
        let changes = ChangeSet::between(&before, &state);
        assert!(changes.touches(first) && changes.touches(second));
        state.destroy_scene(second);
        changes.apply(&mut state, Direction::Undo);
        assert_eq!(render(&state, first, "width"), "1200");
        assert!(state.scene_manager().scene(second).is_none());
        changes.apply(&mut state, Direction::Redo);
        assert_eq!(render(&state, first, "width"), "32");
        assert!(state.scene_manager().scene(second).is_none());
    }
}
