use cuemol_core::LString;

pub const INVALID_UID: u64 = 0;

macro_rules! delegate_event_accessors {
    () => {
        pub fn event_type(&self) -> i32 {
            self.inner.event_type()
        }

        pub fn set_event_type(&mut self, value: i32) {
            self.inner.set_event_type(value);
        }

        pub fn target(&self) -> u64 {
            self.inner.target()
        }

        pub fn set_target(&mut self, value: u64) {
            self.inner.set_target(value);
        }

        pub fn descr(&self) -> &str {
            self.inner.descr()
        }

        pub fn set_descr(&mut self, value: impl Into<String>) {
            self.inner.set_descr(value);
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrEventManager;

impl ScrEventManager {
    pub const SEM_ANY: i32 = -1;
    pub const SEM_LOG: i32 = 0x0001;
    pub const SEM_INDEV: i32 = 0x0002;
    pub const SEM_SCENE: i32 = 0x0004;
    pub const SEM_OBJECT: i32 = 0x0008;
    pub const SEM_RENDERER: i32 = 0x0010;
    pub const SEM_VIEW: i32 = 0x0020;
    pub const SEM_CAMERA: i32 = 0x0040;
    pub const SEM_STYLE: i32 = 0x0080;
    pub const SEM_ANIM: i32 = 0x0100;
    pub const SEM_EXTND: i32 = 0x8000;

    pub const SEM_ADDED: i32 = 1;
    pub const SEM_REMOVING: i32 = 2;
    pub const SEM_PROPCHG: i32 = 3;
    pub const SEM_CHANGED: i32 = 4;
    pub const SEM_OTHER: i32 = 9999;

    pub fn sem_any(&self) -> i32 {
        Self::SEM_ANY
    }

    pub fn sem_log(&self) -> i32 {
        Self::SEM_LOG
    }

    pub fn sem_indev(&self) -> i32 {
        Self::SEM_INDEV
    }

    pub fn sem_scene(&self) -> i32 {
        Self::SEM_SCENE
    }

    pub fn sem_object(&self) -> i32 {
        Self::SEM_OBJECT
    }

    pub fn sem_renderer(&self) -> i32 {
        Self::SEM_RENDERER
    }

    pub fn sem_view(&self) -> i32 {
        Self::SEM_VIEW
    }

    pub fn sem_camera(&self) -> i32 {
        Self::SEM_CAMERA
    }

    pub fn sem_style(&self) -> i32 {
        Self::SEM_STYLE
    }

    pub fn sem_anim(&self) -> i32 {
        Self::SEM_ANIM
    }

    pub fn sem_extnd(&self) -> i32 {
        Self::SEM_EXTND
    }

    pub fn sem_added(&self) -> i32 {
        Self::SEM_ADDED
    }

    pub fn sem_removing(&self) -> i32 {
        Self::SEM_REMOVING
    }

    pub fn sem_propchg(&self) -> i32 {
        Self::SEM_PROPCHG
    }

    pub fn sem_changed(&self) -> i32 {
        Self::SEM_CHANGED
    }

    pub fn sem_other(&self) -> i32 {
        Self::SEM_OTHER
    }

    pub fn add_listener<C: cuemol_core::LScrCallBack + ?Sized>(&self, _callback: &C) -> i32 {
        0
    }

    pub fn remove_listener(&self, _id: i32) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InDevEvent {
    event_type: i32,
    modifier: i32,
    consumed: bool,
    x: i32,
    y: i32,
    root_x: i32,
    root_y: i32,
    delta_x: i32,
    delta_y: i32,
    move_x: i32,
    move_y: i32,
    velo_x: f64,
    velo_y: f64,
}

impl Default for InDevEvent {
    fn default() -> Self {
        Self {
            event_type: Self::INDEV_NONE,
            modifier: 0,
            consumed: false,
            x: 0,
            y: 0,
            root_x: 0,
            root_y: 0,
            delta_x: 0,
            delta_y: 0,
            move_x: 0,
            move_y: 0,
            velo_x: 0.0,
            velo_y: 0.0,
        }
    }
}

impl InDevEvent {
    pub const INDEV_NONE: i32 = 0;
    pub const INDEV_LBTN_CLICK: i32 = 1;
    pub const INDEV_RBTN_CLICK: i32 = 2;
    pub const INDEV_MBTN_CLICK: i32 = 3;
    pub const INDEV_LBTN_DBLCLICK: i32 = 4;
    pub const INDEV_RBTN_DBLCLICK: i32 = 5;
    pub const INDEV_MBTN_DBLCLICK: i32 = 6;
    pub const INDEV_DRAG_START: i32 = 7;
    pub const INDEV_DRAG_MOVE: i32 = 8;
    pub const INDEV_DRAG_END: i32 = 9;
    pub const INDEV_WHEEL: i32 = 10;
    pub const INDEV_MOUSE_DOWN: i32 = 11;
    pub const INDEV_MOUSE_ENTER: i32 = 12;
    pub const INDEV_GESTURE: i32 = 13;

    pub const INDEV_SHIFT: i32 = 1 << 0;
    pub const INDEV_CTRL: i32 = 1 << 1;
    pub const INDEV_ALT: i32 = 1 << 2;
    pub const INDEV_LBTN: i32 = 1 << 3;
    pub const INDEV_MBTN: i32 = 1 << 4;
    pub const INDEV_RBTN: i32 = 1 << 5;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn event_type(&self) -> i32 {
        self.event_type
    }

    pub fn set_event_type(&mut self, value: i32) {
        self.event_type = value;
    }

    pub fn modifier(&self) -> i32 {
        self.modifier
    }

    pub fn set_modifier(&mut self, value: i32) {
        self.modifier = value;
    }

    pub fn is_shift_on(&self) -> bool {
        self.modifier & Self::INDEV_SHIFT != 0
    }

    pub fn indev_shift(&self) -> bool {
        self.is_shift_on()
    }

    pub fn is_ctrl_on(&self) -> bool {
        self.modifier & Self::INDEV_CTRL != 0
    }

    pub fn indev_ctrl(&self) -> bool {
        self.is_ctrl_on()
    }

    pub fn is_alt_on(&self) -> bool {
        self.modifier & Self::INDEV_ALT != 0
    }

    pub fn indev_alt(&self) -> bool {
        self.is_alt_on()
    }

    pub fn is_lbutton_on(&self) -> bool {
        self.modifier & Self::INDEV_LBTN != 0
    }

    pub fn indev_lbtn(&self) -> bool {
        self.is_lbutton_on()
    }

    pub fn is_mbutton_on(&self) -> bool {
        self.modifier & Self::INDEV_MBTN != 0
    }

    pub fn indev_mbtn(&self) -> bool {
        self.is_mbutton_on()
    }

    pub fn is_rbutton_on(&self) -> bool {
        self.modifier & Self::INDEV_RBTN != 0
    }

    pub fn indev_rbtn(&self) -> bool {
        self.is_rbutton_on()
    }

    pub fn is_consumed(&self) -> bool {
        self.consumed
    }

    pub fn set_consumed(&mut self, value: bool) {
        self.consumed = value;
    }

    pub fn x(&self) -> i32 {
        self.x
    }

    pub fn set_x(&mut self, value: i32) {
        self.x = value;
    }

    pub fn y(&self) -> i32 {
        self.y
    }

    pub fn set_y(&mut self, value: i32) {
        self.y = value;
    }

    pub fn root_x(&self) -> i32 {
        self.root_x
    }

    pub fn set_root_x(&mut self, value: i32) {
        self.root_x = value;
    }

    pub fn root_y(&self) -> i32 {
        self.root_y
    }

    pub fn set_root_y(&mut self, value: i32) {
        self.root_y = value;
    }

    pub fn delta_x(&self) -> i32 {
        self.delta_x
    }

    pub fn set_delta_x(&mut self, value: i32) {
        self.delta_x = value;
    }

    pub fn delta_y(&self) -> i32 {
        self.delta_y
    }

    pub fn set_delta_y(&mut self, value: i32) {
        self.delta_y = value;
    }

    pub fn move_x(&self) -> i32 {
        self.move_x
    }

    pub fn set_move_x(&mut self, value: i32) {
        self.move_x = value;
    }

    pub fn move_y(&self) -> i32 {
        self.move_y
    }

    pub fn set_move_y(&mut self, value: i32) {
        self.move_y = value;
    }

    pub fn velo_x(&self) -> f64 {
        self.velo_x
    }

    pub fn set_velo_x(&mut self, value: f64) {
        self.velo_x = value;
    }

    pub fn velo_y(&self) -> f64 {
        self.velo_y
    }

    pub fn set_velo_y(&mut self, value: f64) {
        self.velo_y = value;
    }

    pub fn json(&self) -> LString {
        LString::from(format!(
            "{{ \"x\": {}, \"y\": {}, \"mod\": {} }}",
            self.x, self.y, self.modifier
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QsysEvent {
    event_type: i32,
    target: u64,
    source: u64,
    descr: String,
}

impl Default for QsysEvent {
    fn default() -> Self {
        Self {
            event_type: 0,
            target: INVALID_UID,
            source: INVALID_UID,
            descr: String::new(),
        }
    }
}

impl QsysEvent {
    pub fn event_type(&self) -> i32 {
        self.event_type
    }

    pub fn set_event_type(&mut self, value: i32) {
        self.event_type = value;
    }

    pub fn target(&self) -> u64 {
        self.target
    }

    pub fn set_target(&mut self, value: u64) {
        self.target = value;
    }

    pub fn source(&self) -> u64 {
        self.source
    }

    pub fn set_source(&mut self, value: u64) {
        self.source = value;
    }

    pub fn descr(&self) -> &str {
        &self.descr
    }

    pub fn set_descr(&mut self, value: impl Into<String>) {
        self.descr = value.into();
    }

    fn json(&self) -> LString {
        LString::from(format!(
            "{{\"target_uid\": {},\"descr\": \"{}\" }}",
            self.target,
            escape_json(&self.descr)
        ))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectEvent {
    inner: QsysEvent,
}

impl ObjectEvent {
    pub const OBE_CHANGED: i32 = 2;
    pub const OBE_PROPCHG: i32 = 3;

    pub fn new() -> Self {
        Self::default()
    }

    delegate_event_accessors!();

    pub fn json(&self) -> LString {
        self.inner.json()
    }

    pub fn category(&self) -> Option<(LString, i32, i32)> {
        match self.event_type() {
            Self::OBE_CHANGED => Some((
                LString::from(self.descr().to_string()),
                ScrEventManager::SEM_OBJECT,
                ScrEventManager::SEM_CHANGED,
            )),
            Self::OBE_PROPCHG => Some((
                LString::from(self.descr().to_string()),
                ScrEventManager::SEM_OBJECT,
                ScrEventManager::SEM_PROPCHG,
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneEvent {
    inner: QsysEvent,
}

impl SceneEvent {
    pub const SCE_SCENE_REMOVING: i32 = 1;
    pub const SCE_SCENE_PROPCHG: i32 = 2;
    pub const SCE_SCENE_UNDOINFO: i32 = 3;
    pub const SCE_SCENE_ONLOADED: i32 = 4;
    pub const SCE_SCENE_CLEARALL: i32 = 5;
    pub const SCE_OBJ_ADDED: i32 = 11;
    pub const SCE_OBJ_REMOVING: i32 = 12;
    pub const SCE_REND_ADDED: i32 = 21;
    pub const SCE_REND_REMOVING: i32 = 22;
    pub const SCE_VIEW_ADDED: i32 = 31;
    pub const SCE_VIEW_REMOVING: i32 = 32;
    pub const SCE_STYLE_ADDED: i32 = 41;
    pub const SCE_STYLE_REMOVING: i32 = 42;

    pub fn new() -> Self {
        Self::default()
    }

    delegate_event_accessors!();

    pub fn json(&self) -> LString {
        self.inner.json()
    }

    pub fn category(&self) -> Option<(LString, i32, i32)> {
        let (category, source, event) = match self.event_type() {
            Self::SCE_SCENE_REMOVING => (
                "sceneRemoving",
                ScrEventManager::SEM_SCENE,
                ScrEventManager::SEM_REMOVING,
            ),
            Self::SCE_SCENE_PROPCHG => (
                "scenePropChanged",
                ScrEventManager::SEM_SCENE,
                ScrEventManager::SEM_PROPCHG,
            ),
            Self::SCE_SCENE_UNDOINFO => (
                "sceneUndoInfo",
                ScrEventManager::SEM_SCENE,
                ScrEventManager::SEM_CHANGED,
            ),
            Self::SCE_SCENE_ONLOADED => (
                "sceneLoaded",
                ScrEventManager::SEM_SCENE,
                ScrEventManager::SEM_CHANGED,
            ),
            Self::SCE_SCENE_CLEARALL => (
                "sceneAllCleared",
                ScrEventManager::SEM_SCENE,
                ScrEventManager::SEM_CHANGED,
            ),
            Self::SCE_OBJ_ADDED => (
                "objectAdded",
                ScrEventManager::SEM_OBJECT,
                ScrEventManager::SEM_ADDED,
            ),
            Self::SCE_OBJ_REMOVING => (
                "objectRemoving",
                ScrEventManager::SEM_OBJECT,
                ScrEventManager::SEM_REMOVING,
            ),
            Self::SCE_REND_ADDED => (
                "rendererAdded",
                ScrEventManager::SEM_RENDERER,
                ScrEventManager::SEM_ADDED,
            ),
            Self::SCE_REND_REMOVING => (
                "rendererRemoving",
                ScrEventManager::SEM_RENDERER,
                ScrEventManager::SEM_REMOVING,
            ),
            Self::SCE_VIEW_ADDED => (
                "viewAdded",
                ScrEventManager::SEM_VIEW,
                ScrEventManager::SEM_ADDED,
            ),
            Self::SCE_VIEW_REMOVING => (
                "viewRemoving",
                ScrEventManager::SEM_VIEW,
                ScrEventManager::SEM_REMOVING,
            ),
            Self::SCE_STYLE_ADDED => (
                "styleAdded",
                ScrEventManager::SEM_STYLE,
                ScrEventManager::SEM_ADDED,
            ),
            Self::SCE_STYLE_REMOVING => (
                "styleRemoving",
                ScrEventManager::SEM_STYLE,
                ScrEventManager::SEM_REMOVING,
            ),
            _ => return None,
        };
        Some((LString::from(category), source, event))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RendererEvent {
    inner: QsysEvent,
}

impl RendererEvent {
    pub const RNE_CHANGED: i32 = 2;
    pub const RNE_PROPCHG: i32 = 3;

    pub fn new() -> Self {
        Self::default()
    }

    delegate_event_accessors!();

    pub fn json(&self) -> LString {
        self.inner.json()
    }

    pub fn category(&self) -> Option<(LString, i32, i32)> {
        match self.event_type() {
            Self::RNE_CHANGED => Some((
                LString::from(self.descr().to_string()),
                ScrEventManager::SEM_RENDERER,
                ScrEventManager::SEM_CHANGED,
            )),
            Self::RNE_PROPCHG => Some((
                LString::from(self.descr().to_string()),
                ScrEventManager::SEM_RENDERER,
                ScrEventManager::SEM_PROPCHG,
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewEvent {
    inner: QsysEvent,
}

impl ViewEvent {
    pub const VWE_PROPCHG: i32 = 2;
    pub const VWE_PROPCHG_DRG: i32 = 3;
    pub const VWE_ACTIVATED: i32 = 4;
    pub const VWE_SIZECHG: i32 = 5;

    pub fn new() -> Self {
        Self::default()
    }

    delegate_event_accessors!();

    pub fn json(&self) -> LString {
        let dragging = self.event_type() == Self::VWE_PROPCHG_DRG;
        LString::from(format!(
            "{{\"dragging\": {}, \"propname\": \"{}\", \"target_uid\": {},\"descr\": \"{}\" }}",
            dragging,
            escape_json(self.descr()),
            self.target(),
            escape_json(self.descr())
        ))
    }

    pub fn category(&self) -> Option<(LString, i32, i32)> {
        match self.event_type() {
            Self::VWE_PROPCHG => Some((
                LString::from("viewPropChanged"),
                ScrEventManager::SEM_VIEW,
                ScrEventManager::SEM_PROPCHG,
            )),
            Self::VWE_PROPCHG_DRG => Some((
                LString::from("viewPropChgDragging"),
                ScrEventManager::SEM_VIEW,
                ScrEventManager::SEM_PROPCHG,
            )),
            Self::VWE_ACTIVATED => Some((
                LString::from("viewActivated"),
                ScrEventManager::SEM_VIEW,
                ScrEventManager::SEM_OTHER,
            )),
            Self::VWE_SIZECHG => Some((
                LString::from("viewSizeChanged"),
                ScrEventManager::SEM_VIEW,
                ScrEventManager::SEM_CHANGED,
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispListCacheImpl {
    valid: bool,
    hit_valid: bool,
}

impl DispListCacheImpl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    pub fn invalidate_hit(&mut self) {
        self.hit_valid = false;
    }
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qsys_event_constants_match_reference_values() {
        assert_eq!(ScrEventManager::SEM_RENDERER, 0x0010);
        assert_eq!(ScrEventManager::SEM_OBJECT, 0x0008);
        assert_eq!(ScrEventManager::SEM_VIEW, 0x0020);
        assert_eq!(ScrEventManager::SEM_PROPCHG, 3);
        assert_eq!(ScrEventManager::SEM_CHANGED, 4);
    }

    #[test]
    fn indev_and_scene_events_match_reference_cases() {
        let mut input = InDevEvent::new();
        assert_eq!(input.event_type(), InDevEvent::INDEV_NONE);
        assert_eq!(input.modifier(), 0);
        assert!(!input.is_consumed());
        input.set_event_type(InDevEvent::INDEV_LBTN_CLICK);
        input.set_modifier(InDevEvent::INDEV_SHIFT | InDevEvent::INDEV_CTRL);
        assert!(input.is_shift_on());
        assert!(input.is_ctrl_on());
        assert!(!input.is_alt_on());
        input.set_x(10);
        input.set_y(20);
        input.set_delta_x(1);
        input.set_delta_y(2);
        input.set_move_x(3);
        input.set_move_y(4);
        input.set_root_x(30);
        input.set_root_y(40);
        input.set_velo_x(1.5);
        input.set_velo_y(-2.5);
        assert_eq!(input.x(), 10);
        assert_eq!(input.y(), 20);
        assert_eq!(input.delta_x(), 1);
        assert_eq!(input.delta_y(), 2);
        assert_eq!(input.move_x(), 3);
        assert_eq!(input.move_y(), 4);
        assert_eq!(input.root_x(), 30);
        assert_eq!(input.root_y(), 40);
        assert_eq!(input.velo_x(), 1.5);
        assert_eq!(input.velo_y(), -2.5);
        assert!(input.json().index_of("\"x\"") >= 0);

        let mut scene = SceneEvent::new();
        scene.set_event_type(SceneEvent::SCE_OBJ_ADDED);
        scene.set_target(5);
        scene.set_descr("added");
        assert!(scene.json().index_of("target_uid") >= 0);
        assert_eq!(
            scene.category().unwrap(),
            (
                LString::from("objectAdded"),
                ScrEventManager::SEM_OBJECT,
                ScrEventManager::SEM_ADDED
            )
        );
        scene.set_event_type(SceneEvent::SCE_STYLE_REMOVING);
        assert_eq!(scene.category().unwrap().0, LString::from("styleRemoving"));
    }

    #[test]
    fn object_and_renderer_events_match_reference_json_and_category() {
        let mut object_event = ObjectEvent::new();
        assert_eq!(object_event.event_type(), 0);
        assert_eq!(object_event.target(), INVALID_UID);
        assert_eq!(object_event.descr(), "");
        object_event.set_event_type(ObjectEvent::OBE_CHANGED);
        object_event.set_target(10);
        object_event.set_descr("changed");
        let json = object_event.json();
        assert!(json.index_of("target_uid") >= 0);
        assert!(json.index_of("descr") >= 0);
        assert_eq!(
            object_event.category().unwrap(),
            (
                LString::from("changed"),
                ScrEventManager::SEM_OBJECT,
                ScrEventManager::SEM_CHANGED
            )
        );

        let mut renderer_event = RendererEvent::new();
        renderer_event.set_event_type(RendererEvent::RNE_PROPCHG);
        renderer_event.set_target(7);
        renderer_event.set_descr("propDesc");
        assert!(renderer_event.json().index_of("target_uid") >= 0);
        assert_eq!(
            renderer_event.category().unwrap(),
            (
                LString::from("propDesc"),
                ScrEventManager::SEM_RENDERER,
                ScrEventManager::SEM_PROPCHG
            )
        );
    }

    #[test]
    fn view_event_and_display_cache_match_reference() {
        let mut event = ViewEvent::new();
        event.set_event_type(ViewEvent::VWE_PROPCHG);
        event.set_target(8);
        event.set_descr("zoom");
        assert!(event.json().index_of("dragging") >= 0);
        assert!(event.json().index_of("false") >= 0);
        assert_eq!(
            event.category().unwrap(),
            (
                LString::from("viewPropChanged"),
                ScrEventManager::SEM_VIEW,
                ScrEventManager::SEM_PROPCHG
            )
        );

        event.set_event_type(ViewEvent::VWE_PROPCHG_DRG);
        assert!(event.json().index_of("true") >= 0);
        event.set_event_type(ViewEvent::VWE_SIZECHG);
        assert_eq!(event.category().unwrap().2, ScrEventManager::SEM_CHANGED);

        let mut cache = DispListCacheImpl::new();
        cache.invalidate();
        cache.invalidate();
        cache.invalidate_hit();
        cache.invalidate_hit();
    }
}
