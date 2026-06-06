use std::collections::BTreeMap;

use cuemol_core::Uid;

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
}
