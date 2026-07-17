use std::collections::BTreeMap;

use cuemol_core::Uid;
use cuemol_gfx::{DisplayContext, DisplayList};

use crate::{DispListCacheImpl, ObjectEvent, SceneEvent};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DrawObj {
    enabled: bool,
}

impl DrawObj {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn enabled(&self) -> bool {
        self.is_enabled()
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DispListRenderer {
    display_cache_valid: bool,
    hittest_cache_valid: bool,
}

impl DispListRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate_display_cache(&mut self) {
        self.display_cache_valid = false;
    }

    pub fn invalidate_hittest_cache(&mut self) {
        self.hittest_cache_valid = false;
    }

    pub fn mark_display_cache_valid(&mut self) {
        self.display_cache_valid = true;
    }

    pub fn mark_hittest_cache_valid(&mut self) {
        self.hittest_cache_valid = true;
    }

    pub fn is_display_cache_valid(&self) -> bool {
        self.display_cache_valid
    }

    pub fn is_hittest_cache_valid(&self) -> bool {
        self.hittest_cache_valid
    }
}

/// Renderer-side display-list cache and invalidation policy.
#[derive(Debug)]
pub struct DispCacheRenderer {
    cache: DispListCacheImpl,
    shader_alpha: bool,
    scene_update_requested: bool,
}

impl DispCacheRenderer {
    pub fn new(shader_alpha: bool) -> Self {
        Self {
            cache: DispListCacheImpl::new(),
            shader_alpha,
            scene_update_requested: false,
        }
    }

    pub fn display<Render>(&mut self, context: &mut DisplayContext, render: Render) -> bool
    where
        Render: FnOnce(&mut DisplayList),
    {
        self.cache.display(
            context,
            render,
            |context| context.set_lighting(true),
            |context| context.set_lighting(false),
        )
    }

    pub fn display_hit<Render>(&mut self, context: &DisplayContext, render: Render) -> bool
    where
        Render: FnOnce(&mut DisplayList),
    {
        self.cache.display_hit(context, render)
    }

    pub fn invalidate_display_cache(&mut self) {
        self.cache.invalidate();
        self.scene_update_requested = true;
    }

    pub fn invalidate_hittest_cache(&mut self) {
        self.cache.invalidate_hit();
    }

    pub fn unloading(&mut self) {
        self.invalidate_display_cache();
        self.invalidate_hittest_cache();
    }

    pub fn object_changed(&mut self, event: &ObjectEvent) {
        if event.event_type() == ObjectEvent::OBE_CHANGED {
            self.invalidate_display_cache();
            self.invalidate_hittest_cache();
        } else if event.event_type() == ObjectEvent::OBE_PROPCHG && event.descr() == "xformMat" {
            self.invalidate_display_cache();
        }
    }

    pub fn property_changed(&mut self, property_name: &str) {
        if !self.shader_alpha && matches!(property_name, "alpha" | "material") {
            self.invalidate_display_cache();
        }
    }

    pub fn style_changed(&mut self) {
        self.invalidate_display_cache();
    }

    pub fn scene_changed(&mut self, event: &SceneEvent) {
        if event.event_type() == SceneEvent::SCE_SCENE_PROPCHG
            && matches!(
                event.descr(),
                "icc_filename" | "use_colproof" | "icc_intent"
            )
        {
            self.invalidate_display_cache();
        }
    }

    pub fn use_shader_alpha(&self) -> bool {
        self.shader_alpha
    }

    pub fn is_display_cache_valid(&self) -> bool {
        self.cache.is_valid()
    }

    pub fn is_hittest_cache_valid(&self) -> bool {
        self.cache.is_hit_valid()
    }

    pub fn take_scene_update_request(&mut self) -> bool {
        std::mem::take(&mut self.scene_update_requested)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShaderObjMgr {
    entries: BTreeMap<(Uid, String), usize>,
}

impl ShaderObjMgr {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_shader_object(
        &mut self,
        name: impl Into<String>,
        scene_uid: Uid,
        handle: usize,
    ) -> bool {
        if scene_uid.get() == 0 {
            return false;
        }
        let key = (scene_uid, name.into());
        if self.entries.contains_key(&key) {
            return false;
        }
        self.entries.insert(key, handle);
        true
    }

    pub fn shader_object(&self, name: &str, scene_uid: Uid) -> Option<usize> {
        self.entries.get(&(scene_uid, name.to_string())).copied()
    }

    pub fn remove_scene(&mut self, scene_uid: Uid) {
        self.entries.retain(|(uid, _), _| *uid != scene_uid);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn draw_obj_and_disp_list_renderer_match_reference_state_contract() {
        let mut draw = DrawObj::new();
        assert!(!draw.is_enabled());
        draw.set_enabled(true);
        assert!(draw.is_enabled());
        draw.set_enabled(false);
        assert!(!draw.is_enabled());

        let mut renderer = DispListRenderer::new();
        renderer.invalidate_display_cache();
        renderer.invalidate_display_cache();
        renderer.invalidate_hittest_cache();
        renderer.invalidate_hittest_cache();
        assert!(!renderer.is_display_cache_valid());
        assert!(!renderer.is_hittest_cache_valid());
    }

    #[test]
    fn shader_obj_mgr_registers_first_entry_and_clears_by_scene() {
        let mut manager = ShaderObjMgr::new();
        let scene = Uid::new(1);
        assert!(manager.register_shader_object("shader", scene, 100));
        assert_eq!(manager.shader_object("shader", scene), Some(100));
        assert_eq!(manager.shader_object("missing", scene), None);
        assert!(!manager.register_shader_object("bad", Uid::new(0), 200));
        assert!(!manager.register_shader_object("shader", scene, 300));
        assert_eq!(manager.shader_object("shader", scene), Some(100));

        let scene2 = Uid::new(2);
        assert!(manager.register_shader_object("shader2", scene2, 400));
        manager.remove_scene(scene);
        assert_eq!(manager.shader_object("shader", scene), None);
        assert_eq!(manager.shader_object("shader2", scene2), Some(400));
    }

    #[test]
    fn display_cache_builds_once_and_tracks_reference_invalidations() {
        let mut renderer = DispCacheRenderer::new(false);
        let mut context = DisplayContext::new();
        let render_count = Cell::new(0);

        assert!(renderer.display(&mut context, |_| {
            render_count.set(render_count.get() + 1);
        }));
        assert!(!renderer.display(&mut context, |_| {
            render_count.set(render_count.get() + 1);
        }));
        assert_eq!(render_count.get(), 1);
        assert!(renderer.is_display_cache_valid());
        assert!(!context.lighting());

        renderer.property_changed("alpha");
        assert!(!renderer.is_display_cache_valid());
        assert!(renderer.take_scene_update_request());

        let mut object_event = ObjectEvent::new();
        object_event.set_event_type(ObjectEvent::OBE_CHANGED);
        renderer.display_hit(&context, |_| {});
        assert!(renderer.is_hittest_cache_valid());
        renderer.object_changed(&object_event);
        assert!(!renderer.is_hittest_cache_valid());
    }

    #[test]
    fn shader_alpha_avoids_geometry_rebuild_for_alpha_changes() {
        let mut renderer = DispCacheRenderer::new(true);
        let mut context = DisplayContext::new();
        renderer.display(&mut context, |_| {});
        renderer.property_changed("alpha");
        assert!(renderer.is_display_cache_valid());

        let mut event = SceneEvent::new();
        event.set_event_type(SceneEvent::SCE_SCENE_PROPCHG);
        event.set_descr("icc_filename");
        renderer.scene_changed(&event);
        assert!(!renderer.is_display_cache_valid());
    }
}
