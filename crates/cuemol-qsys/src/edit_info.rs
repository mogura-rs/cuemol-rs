//! Undo/redo records for cameras, objects, renderers, and style sets.

use crate::{Camera, EditInfo, Renderer, Scene, SceneObject, StyleManager, StyleSet};
use cuemol_core::Uid;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub type SharedScene<T> = Rc<RefCell<Scene<T>>>;
pub type SharedStyleManager = Rc<RefCell<StyleManager>>;

/// Camera creation/destruction edit record.
pub struct CameraCreateEditInfo<T> {
    scene: Weak<RefCell<Scene<T>>>,
    camera: Camera,
    create: bool,
}

impl<T> CameraCreateEditInfo<T> {
    pub fn setup_create(scene: &SharedScene<T>, camera: Camera) -> Self {
        Self {
            scene: Rc::downgrade(scene),
            camera,
            create: true,
        }
    }

    pub fn setup_destroy(scene: &SharedScene<T>, camera: Camera) -> Self {
        Self {
            scene: Rc::downgrade(scene),
            camera,
            create: false,
        }
    }

    pub fn camera_name(&self) -> &str {
        self.camera.name()
    }

    fn insert_camera(&self) -> bool {
        let Some(scene) = self.scene.upgrade() else {
            return false;
        };
        let mut scene = scene.borrow_mut();
        if scene.has_camera(self.camera.name()) {
            return false;
        }
        scene.set_camera(self.camera.name(), self.camera.clone());
        true
    }

    fn remove_camera(&self) -> bool {
        self.scene
            .upgrade()
            .is_some_and(|scene| scene.borrow_mut().destroy_camera(self.camera.name()))
    }

    fn camera_exists(&self) -> bool {
        self.scene
            .upgrade()
            .is_some_and(|scene| scene.borrow().has_camera(self.camera.name()))
    }
}

impl<T: 'static> EditInfo for CameraCreateEditInfo<T> {
    fn undo(&mut self) -> bool {
        if self.create {
            self.remove_camera()
        } else {
            self.insert_camera()
        }
    }

    fn redo(&mut self) -> bool {
        if self.create {
            self.insert_camera()
        } else {
            self.remove_camera()
        }
    }

    fn is_undoable(&self) -> bool {
        self.camera_exists() == self.create
    }

    fn is_redoable(&self) -> bool {
        self.camera_exists() != self.create
    }
}

/// Camera value-change edit record.
pub struct CameraPropEditInfo<T> {
    scene: Weak<RefCell<Scene<T>>>,
    camera_name: String,
    before: Camera,
    after: Camera,
}

impl<T> CameraPropEditInfo<T> {
    pub fn setup(
        scene: &SharedScene<T>,
        camera_name: impl Into<String>,
        before: Camera,
        after: Camera,
    ) -> Self {
        Self {
            scene: Rc::downgrade(scene),
            camera_name: camera_name.into(),
            before,
            after,
        }
    }

    pub fn camera_name(&self) -> &str {
        &self.camera_name
    }

    fn apply(&self, camera: &Camera) -> bool {
        let Some(scene) = self.scene.upgrade() else {
            return false;
        };
        let mut scene = scene.borrow_mut();
        if !scene.has_camera(&self.camera_name) {
            return false;
        }
        scene.set_camera(&self.camera_name, camera.clone());
        true
    }

    fn target_exists(&self) -> bool {
        self.scene
            .upgrade()
            .is_some_and(|scene| scene.borrow().has_camera(&self.camera_name))
    }
}

impl<T: 'static> EditInfo for CameraPropEditInfo<T> {
    fn undo(&mut self) -> bool {
        self.apply(&self.before)
    }

    fn redo(&mut self) -> bool {
        self.apply(&self.after)
    }

    fn is_undoable(&self) -> bool {
        self.target_exists()
    }

    fn is_redoable(&self) -> bool {
        self.target_exists()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjLoadEditMode {
    ObjectCreate,
    ObjectDestroy,
    RendererCreate,
    RendererDestroy,
}

/// Object/renderer creation/destruction edit record.
pub struct ObjLoadEditInfo<T> {
    scene: Weak<RefCell<Scene<T>>>,
    mode: ObjLoadEditMode,
    object_uid: Uid,
    renderer_uid: Uid,
    object: Option<SceneObject<T>>,
    renderer: Option<Renderer>,
}

impl<T> ObjLoadEditInfo<T> {
    pub fn setup_object_create(scene: &SharedScene<T>, object_uid: Uid) -> Self {
        Self {
            scene: Rc::downgrade(scene),
            mode: ObjLoadEditMode::ObjectCreate,
            object_uid,
            renderer_uid: Uid::new(0),
            object: None,
            renderer: None,
        }
    }

    pub fn setup_object_destroy(scene: &SharedScene<T>, object: SceneObject<T>) -> Self {
        let object_uid = object.uid();
        Self {
            scene: Rc::downgrade(scene),
            mode: ObjLoadEditMode::ObjectDestroy,
            object_uid,
            renderer_uid: Uid::new(0),
            object: Some(object),
            renderer: None,
        }
    }

    pub fn setup_renderer_create(
        scene: &SharedScene<T>,
        object_uid: Uid,
        renderer_uid: Uid,
    ) -> Self {
        Self {
            scene: Rc::downgrade(scene),
            mode: ObjLoadEditMode::RendererCreate,
            object_uid,
            renderer_uid,
            object: None,
            renderer: None,
        }
    }

    pub fn setup_renderer_destroy(
        scene: &SharedScene<T>,
        object_uid: Uid,
        renderer: Renderer,
    ) -> Self {
        let renderer_uid = renderer.uid();
        Self {
            scene: Rc::downgrade(scene),
            mode: ObjLoadEditMode::RendererDestroy,
            object_uid,
            renderer_uid,
            object: None,
            renderer: Some(renderer),
        }
    }

    pub fn mode(&self) -> ObjLoadEditMode {
        self.mode
    }

    pub fn object_uid(&self) -> Uid {
        self.object_uid
    }

    pub fn renderer_uid(&self) -> Option<Uid> {
        matches!(
            self.mode,
            ObjLoadEditMode::RendererCreate | ObjLoadEditMode::RendererDestroy
        )
        .then_some(self.renderer_uid)
    }

    fn object_exists(&self) -> bool {
        self.scene
            .upgrade()
            .is_some_and(|scene| scene.borrow().object(self.object_uid).is_some())
    }

    fn renderer_exists(&self) -> bool {
        self.scene.upgrade().is_some_and(|scene| {
            scene
                .borrow()
                .object(self.object_uid)
                .is_some_and(|object| object.renderer(self.renderer_uid).is_some())
        })
    }

    fn detach_object(&mut self) -> bool {
        if self.object.is_some() {
            return false;
        }
        let Some(scene) = self.scene.upgrade() else {
            return false;
        };
        let Some(object) = scene.borrow_mut().detach_object(self.object_uid) else {
            return false;
        };
        self.object = Some(object);
        true
    }

    fn restore_object(&mut self) -> bool {
        let Some(scene) = self.scene.upgrade() else {
            return false;
        };
        let mut scene = scene.borrow_mut();
        if scene.object(self.object_uid).is_some() {
            return false;
        }
        let Some(object) = self.object.take() else {
            return false;
        };
        scene.insert_object(object)
    }

    fn detach_renderer(&mut self) -> bool {
        if self.renderer.is_some() {
            return false;
        }
        let Some(scene) = self.scene.upgrade() else {
            return false;
        };
        let mut scene = scene.borrow_mut();
        let Some(object) = scene.object_mut(self.object_uid) else {
            return false;
        };
        let Some(renderer) = object.detach_renderer(self.renderer_uid) else {
            return false;
        };
        self.renderer = Some(renderer);
        true
    }

    fn restore_renderer(&mut self) -> bool {
        let Some(scene) = self.scene.upgrade() else {
            return false;
        };
        let mut scene = scene.borrow_mut();
        let Some(object) = scene.object_mut(self.object_uid) else {
            return false;
        };
        if object.renderer(self.renderer_uid).is_some() {
            return false;
        }
        let Some(renderer) = self.renderer.take() else {
            return false;
        };
        object.attach_renderer(renderer)
    }
}

impl<T: 'static> EditInfo for ObjLoadEditInfo<T> {
    fn undo(&mut self) -> bool {
        match self.mode {
            ObjLoadEditMode::ObjectCreate => self.detach_object(),
            ObjLoadEditMode::ObjectDestroy => self.restore_object(),
            ObjLoadEditMode::RendererCreate => self.detach_renderer(),
            ObjLoadEditMode::RendererDestroy => self.restore_renderer(),
        }
    }

    fn redo(&mut self) -> bool {
        match self.mode {
            ObjLoadEditMode::ObjectCreate => self.restore_object(),
            ObjLoadEditMode::ObjectDestroy => self.detach_object(),
            ObjLoadEditMode::RendererCreate => self.restore_renderer(),
            ObjLoadEditMode::RendererDestroy => self.detach_renderer(),
        }
    }

    fn is_undoable(&self) -> bool {
        match self.mode {
            ObjLoadEditMode::ObjectCreate => self.object.is_none() && self.object_exists(),
            ObjLoadEditMode::ObjectDestroy => self.object.is_some() && !self.object_exists(),
            ObjLoadEditMode::RendererCreate => self.renderer.is_none() && self.renderer_exists(),
            ObjLoadEditMode::RendererDestroy => self.renderer.is_some() && !self.renderer_exists(),
        }
    }

    fn is_redoable(&self) -> bool {
        match self.mode {
            ObjLoadEditMode::ObjectCreate => self.object.is_some() && !self.object_exists(),
            ObjLoadEditMode::ObjectDestroy => self.object.is_none() && self.object_exists(),
            ObjLoadEditMode::RendererCreate => self.renderer.is_some() && !self.renderer_exists(),
            ObjLoadEditMode::RendererDestroy => self.renderer.is_none() && self.renderer_exists(),
        }
    }
}

/// Style-set creation/destruction edit record.
pub struct StyleCreateEditInfo {
    manager: Weak<RefCell<StyleManager>>,
    scope_id: u64,
    style_set_id: u64,
    insertion_index: usize,
    create: bool,
    style_set: Option<StyleSet>,
}

impl StyleCreateEditInfo {
    pub fn setup_create(
        manager: &SharedStyleManager,
        scope_id: u64,
        style_set_id: u64,
    ) -> Option<Self> {
        let insertion_index = manager.borrow().style_set_index(scope_id, style_set_id)?;
        Some(Self {
            manager: Rc::downgrade(manager),
            scope_id,
            style_set_id,
            insertion_index,
            create: true,
            style_set: None,
        })
    }

    pub fn setup_destroy(
        manager: &SharedStyleManager,
        scope_id: u64,
        style_set: StyleSet,
        insertion_index: usize,
    ) -> Self {
        let style_set_id = style_set.uid();
        Self {
            manager: Rc::downgrade(manager),
            scope_id,
            style_set_id,
            insertion_index,
            create: false,
            style_set: Some(style_set),
        }
    }

    pub fn style_set_id(&self) -> u64 {
        self.style_set_id
    }

    fn registered(&self) -> bool {
        self.manager.upgrade().is_some_and(|manager| {
            manager
                .borrow()
                .style_set_index(self.scope_id, self.style_set_id)
                .is_some()
        })
    }

    fn detach(&mut self) -> bool {
        if self.style_set.is_some() {
            return false;
        }
        let Some(manager) = self.manager.upgrade() else {
            return false;
        };
        let Some((style_set, index)) = manager
            .borrow_mut()
            .take_style_set(self.scope_id, self.style_set_id)
        else {
            return false;
        };
        self.insertion_index = index;
        self.style_set = Some(style_set);
        true
    }

    fn restore(&mut self) -> bool {
        let Some(manager) = self.manager.upgrade() else {
            return false;
        };
        if self.registered() {
            return false;
        }
        let Some(style_set) = self.style_set.as_ref() else {
            return false;
        };
        let restored = manager.borrow_mut().register_style_set(
            style_set.clone(),
            self.insertion_index as isize,
            self.scope_id,
        );
        if restored {
            self.style_set = None;
        }
        restored
    }
}

impl EditInfo for StyleCreateEditInfo {
    fn undo(&mut self) -> bool {
        if self.create {
            self.detach()
        } else {
            self.restore()
        }
    }

    fn redo(&mut self) -> bool {
        if self.create {
            self.restore()
        } else {
            self.detach()
        }
    }

    fn is_undoable(&self) -> bool {
        if self.create {
            self.style_set.is_none() && self.registered()
        } else {
            self.style_set.is_some() && !self.registered()
        }
    }

    fn is_redoable(&self) -> bool {
        if self.create {
            self.style_set.is_some() && !self.registered()
        } else {
            self.style_set.is_none() && self.registered()
        }
    }
}

/// Style-set source-path edit record.
pub struct StyleSrcEditInfo {
    manager: Weak<RefCell<StyleManager>>,
    style_set_id: u64,
    before: String,
    after: String,
}

impl StyleSrcEditInfo {
    pub fn setup(
        manager: &SharedStyleManager,
        style_set_id: u64,
        before: impl Into<String>,
        after: impl Into<String>,
    ) -> Self {
        Self {
            manager: Rc::downgrade(manager),
            style_set_id,
            before: before.into(),
            after: after.into(),
        }
    }

    fn apply(&self, value: &str) -> bool {
        let Some(manager) = self.manager.upgrade() else {
            return false;
        };
        let mut manager = manager.borrow_mut();
        let Some(style_set) = manager.get_style_set_mut(self.style_set_id) else {
            return false;
        };
        style_set.set_source(value);
        true
    }

    fn target_exists(&self) -> bool {
        self.manager
            .upgrade()
            .is_some_and(|manager| manager.borrow().get_style_set(self.style_set_id).is_some())
    }
}

impl EditInfo for StyleSrcEditInfo {
    fn undo(&mut self) -> bool {
        self.apply(&self.before)
    }

    fn redo(&mut self) -> bool {
        self.apply(&self.after)
    }

    fn is_undoable(&self) -> bool {
        self.target_exists()
    }

    fn is_redoable(&self) -> bool {
        self.target_exists()
    }
}

/// Renderer style-list edit record.
pub struct RendStyleEditInfo<T> {
    scene: Weak<RefCell<Scene<T>>>,
    renderer_uid: Uid,
    before: String,
    after: String,
}

impl<T> RendStyleEditInfo<T> {
    pub fn setup(
        scene: &SharedScene<T>,
        renderer_uid: Uid,
        before: impl Into<String>,
        after: impl Into<String>,
    ) -> Self {
        Self {
            scene: Rc::downgrade(scene),
            renderer_uid,
            before: before.into(),
            after: after.into(),
        }
    }

    fn apply(&self, value: &str) -> bool {
        let Some(scene) = self.scene.upgrade() else {
            return false;
        };
        let mut scene = scene.borrow_mut();
        let Some(renderer) = scene.get_renderer_mut(self.renderer_uid) else {
            return false;
        };
        renderer.apply_styles(value);
        true
    }

    fn target_exists(&self) -> bool {
        self.scene
            .upgrade()
            .is_some_and(|scene| scene.borrow().get_renderer(self.renderer_uid).is_some())
    }
}

impl<T: 'static> EditInfo for RendStyleEditInfo<T> {
    fn undo(&mut self) -> bool {
        self.apply(&self.before)
    }

    fn redo(&mut self) -> bool {
        self.apply(&self.after)
    }

    fn is_undoable(&self) -> bool {
        self.target_exists()
    }

    fn is_redoable(&self) -> bool {
        self.target_exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UndoManager, UndoManagerHandle};

    fn committed(edit: Box<dyn EditInfo>) -> UndoManager {
        let mut manager = UndoManager::new();
        manager.start_txn("edit");
        assert!(manager.add_edit_info(edit));
        assert!(manager.commit_txn());
        manager
    }

    fn committed_to_scene<T>(scene: &SharedScene<T>, edit: Box<dyn EditInfo>) -> UndoManagerHandle {
        let manager = scene.borrow().undo_manager();
        manager.with_mut(|manager| {
            manager.start_txn("edit");
            assert!(manager.add_edit_info(edit));
            assert!(manager.commit_txn());
        });
        manager
    }

    #[test]
    fn camera_create_destroy_and_property_edits_round_trip() {
        let scene = Rc::new(RefCell::new(Scene::<()>::new(Uid::new(1), "scene")));
        let mut camera = Camera::new();
        camera.set_name("saved");
        camera.set_zoom(25.0);
        scene.borrow_mut().set_camera("saved", camera.clone());

        let create = committed_to_scene(
            &scene,
            Box::new(CameraCreateEditInfo::setup_create(&scene, camera.clone())),
        );
        assert!(create.with_mut(UndoManager::undo));
        assert!(!scene.borrow().has_camera("saved"));
        assert!(create.with_mut(UndoManager::redo));
        assert_eq!(scene.borrow().camera("saved").unwrap().zoom(), 25.0);

        let destroyed = scene.borrow().camera("saved").unwrap().clone();
        assert!(scene.borrow_mut().destroy_camera("saved"));
        let destroy = committed_to_scene(
            &scene,
            Box::new(CameraCreateEditInfo::setup_destroy(&scene, destroyed)),
        );
        assert!(destroy.with_mut(UndoManager::undo));
        assert!(scene.borrow().has_camera("saved"));
        assert!(destroy.with_mut(UndoManager::redo));
        assert!(!scene.borrow().has_camera("saved"));

        scene.borrow_mut().set_camera("saved", camera.clone());
        let mut after = camera.clone();
        after.set_zoom(75.0);
        scene.borrow_mut().set_camera("saved", after.clone());
        let property = committed_to_scene(
            &scene,
            Box::new(CameraPropEditInfo::setup(&scene, "saved", camera, after)),
        );
        assert!(property.with_mut(UndoManager::undo));
        assert_eq!(scene.borrow().camera("saved").unwrap().zoom(), 25.0);
        assert!(property.with_mut(UndoManager::redo));
        assert_eq!(scene.borrow().camera("saved").unwrap().zoom(), 75.0);
    }

    #[test]
    fn object_and_renderer_load_edits_preserve_instances() {
        let scene = Rc::new(RefCell::new(Scene::<String>::new(Uid::new(1), "scene")));
        let object_uid = scene
            .borrow_mut()
            .add_object("object", "payload".to_string());
        let object_create = committed_to_scene(
            &scene,
            Box::new(ObjLoadEditInfo::setup_object_create(&scene, object_uid)),
        );
        assert!(object_create.with_mut(UndoManager::undo));
        assert!(scene.borrow().object(object_uid).is_none());
        assert!(object_create.with_mut(UndoManager::redo));
        assert_eq!(scene.borrow().object(object_uid).unwrap().data(), "payload");

        let renderer_uid = scene
            .borrow_mut()
            .object_mut(object_uid)
            .unwrap()
            .create_renderer("simple");
        let renderer_create = committed_to_scene(
            &scene,
            Box::new(ObjLoadEditInfo::setup_renderer_create(
                &scene,
                object_uid,
                renderer_uid,
            )),
        );
        assert!(renderer_create.with_mut(UndoManager::undo));
        assert!(scene.borrow().get_renderer(renderer_uid).is_none());
        assert!(renderer_create.with_mut(UndoManager::redo));
        assert_eq!(
            scene
                .borrow()
                .get_renderer(renderer_uid)
                .unwrap()
                .type_name(),
            "simple"
        );

        let renderer = scene
            .borrow_mut()
            .object_mut(object_uid)
            .unwrap()
            .detach_renderer(renderer_uid)
            .unwrap();
        let renderer_destroy = committed_to_scene(
            &scene,
            Box::new(ObjLoadEditInfo::setup_renderer_destroy(
                &scene, object_uid, renderer,
            )),
        );
        assert!(renderer_destroy.with_mut(UndoManager::undo));
        assert!(scene.borrow().get_renderer(renderer_uid).is_some());
        assert!(renderer_destroy.with_mut(UndoManager::redo));
        assert!(scene.borrow().get_renderer(renderer_uid).is_none());

        let object = scene.borrow_mut().detach_object(object_uid).unwrap();
        let object_destroy = committed_to_scene(
            &scene,
            Box::new(ObjLoadEditInfo::setup_object_destroy(&scene, object)),
        );
        assert!(object_destroy.with_mut(UndoManager::undo));
        assert_eq!(scene.borrow().object(object_uid).unwrap().data(), "payload");
        assert!(object_destroy.with_mut(UndoManager::redo));
        assert!(scene.borrow().object(object_uid).is_none());
    }

    #[test]
    fn style_set_source_and_renderer_style_edits_round_trip() {
        let styles = Rc::new(RefCell::new(StyleManager::new()));
        let first = styles.borrow_mut().create_style_set("first", 7);
        let second = styles.borrow_mut().create_style_set("second", 7);
        let mut create = committed(Box::new(
            StyleCreateEditInfo::setup_create(&styles, 7, first).unwrap(),
        ));
        assert!(create.undo());
        assert!(styles.borrow().get_style_set(first).is_none());
        assert!(create.redo());
        assert_eq!(styles.borrow().style_set_index(7, first), Some(0));
        assert_eq!(styles.borrow().style_set_index(7, second), Some(1));

        styles
            .borrow_mut()
            .get_style_set_mut(first)
            .unwrap()
            .set_source("after.xml");
        let mut source = committed(Box::new(StyleSrcEditInfo::setup(
            &styles,
            first,
            "before.xml",
            "after.xml",
        )));
        assert!(source.undo());
        assert_eq!(
            styles.borrow().get_style_set(first).unwrap().source(),
            "before.xml"
        );
        assert!(source.redo());
        assert_eq!(
            styles.borrow().get_style_set(first).unwrap().source(),
            "after.xml"
        );

        let (removed, index) = styles.borrow_mut().take_style_set(7, first).unwrap();
        let mut destroy = committed(Box::new(StyleCreateEditInfo::setup_destroy(
            &styles, 7, removed, index,
        )));
        assert!(destroy.undo());
        assert!(styles.borrow().get_style_set(first).is_some());
        assert!(destroy.redo());
        assert!(styles.borrow().get_style_set(first).is_none());

        let scene = Rc::new(RefCell::new(Scene::<()>::new(Uid::new(10), "scene")));
        let object_uid = scene.borrow_mut().add_object("object", ());
        let renderer_uid = scene
            .borrow_mut()
            .object_mut(object_uid)
            .unwrap()
            .create_renderer("simple");
        scene
            .borrow_mut()
            .get_renderer_mut(renderer_uid)
            .unwrap()
            .apply_styles("new");
        let mut renderer_style = committed(Box::new(RendStyleEditInfo::setup(
            &scene,
            renderer_uid,
            "old",
            "new",
        )));
        assert!(renderer_style.undo());
        assert_eq!(
            scene
                .borrow()
                .get_renderer(renderer_uid)
                .unwrap()
                .style_names(),
            "old"
        );
        assert!(renderer_style.redo());
        assert_eq!(
            scene
                .borrow()
                .get_renderer(renderer_uid)
                .unwrap()
                .style_names(),
            "new"
        );
    }
}
