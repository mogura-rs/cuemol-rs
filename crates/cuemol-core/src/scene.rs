use crate::{Matrix4, Vector4};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uid(u64);

impl Uid {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    uid: Uid,
    name: String,
    transform: Matrix4,
}

impl Object {
    pub fn new(uid: Uid, name: impl Into<String>) -> Self {
        Self {
            uid,
            name: name.into(),
            transform: Matrix4::IDENTITY,
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn transform(&self) -> Matrix4 {
        self.transform
    }
}

pub trait Renderer {
    fn name(&self) -> &str;
    fn render_bounds(&self) -> Option<(Vector4, Vector4)>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    uid: Uid,
    name: String,
    objects: BTreeMap<Uid, Object>,
    next_uid: u64,
}

impl Scene {
    pub fn new(uid: Uid, name: impl Into<String>) -> Self {
        Self {
            uid,
            name: name.into(),
            objects: BTreeMap::new(),
            next_uid: 1,
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_object(&mut self, name: impl Into<String>) -> Uid {
        let uid = Uid::new(self.next_uid);
        self.next_uid += 1;
        self.objects.insert(uid, Object::new(uid, name));
        uid
    }

    pub fn object(&self, uid: Uid) -> Option<&Object> {
        self.objects.get(&uid)
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }
}

#[derive(Debug, Default)]
pub struct SceneManager {
    scenes: BTreeMap<Uid, Scene>,
    next_uid: u64,
}

impl SceneManager {
    pub fn new() -> Self {
        Self {
            scenes: BTreeMap::new(),
            next_uid: 1,
        }
    }

    pub fn create_scene(&mut self, name: impl Into<String>) -> Uid {
        let uid = Uid::new(self.next_uid);
        self.next_uid += 1;
        self.scenes.insert(uid, Scene::new(uid, name));
        uid
    }

    pub fn scene(&self, uid: Uid) -> Option<&Scene> {
        self.scenes.get(&uid)
    }

    pub fn scene_mut(&mut self, uid: Uid) -> Option<&mut Scene> {
        self.scenes.get_mut(&uid)
    }

    pub fn destroy_all_scenes(&mut self) {
        self.scenes.clear();
    }

    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_scene_and_objects_with_stable_uids() {
        let mut manager = SceneManager::new();
        let scene_uid = manager.create_scene("new scene");
        let scene = manager.scene_mut(scene_uid).unwrap();
        let object_uid = scene.add_object("1CRN");

        assert_eq!(scene.uid(), scene_uid);
        assert_eq!(scene.object(object_uid).unwrap().name(), "1CRN");
        assert_eq!(scene.object_count(), 1);
    }
}
