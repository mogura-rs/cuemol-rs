use crate::{CueMolError, LString, LVariant, Result};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::{Rc, Weak};

pub type EnumDef = BTreeMap<LString, i32>;

/// Property metadata corresponding to `qlib::PropSpec`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropSpec {
    pub type_name: LString,
    pub read_only: bool,
    pub has_default: bool,
    pub no_persist: bool,
    pub enum_def: Option<EnumDef>,
}

impl PropSpec {
    pub fn new(type_name: impl Into<LString>) -> Self {
        Self {
            type_name: type_name.into(),
            ..Self::default()
        }
    }

    pub fn writable(type_name: impl Into<LString>) -> Self {
        Self::new(type_name)
    }

    pub fn read_only(type_name: impl Into<LString>) -> Self {
        Self {
            read_only: true,
            ..Self::new(type_name)
        }
    }

    pub fn with_default(mut self) -> Self {
        self.has_default = true;
        self
    }

    pub fn no_persist(mut self) -> Self {
        self.no_persist = true;
        self
    }

    pub fn with_enum(mut self, values: EnumDef) -> Self {
        self.enum_def = Some(values);
        self
    }
}

/// Property-change event corresponding to `qlib::LPropEvent`.
#[derive(Debug, Clone, Default)]
pub struct LPropEvent {
    target_uid: u64,
    parent_name: LString,
    name: LString,
    new_value: LVariant,
    old_value: LVariant,
    new_default: bool,
    old_default: bool,
    internal_data_changed: bool,
}

impl LPropEvent {
    pub fn new(name: impl Into<LString>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn name(&self) -> &LString {
        &self.name
    }

    pub fn set_name(&mut self, value: impl Into<LString>) {
        self.name = value.into();
    }

    pub fn parent_name(&self) -> &LString {
        &self.parent_name
    }

    pub fn set_parent_name(&mut self, value: impl Into<LString>) {
        self.parent_name = value.into();
    }

    pub fn new_value(&self) -> &LVariant {
        &self.new_value
    }

    pub fn set_new_value(&mut self, value: &LVariant) {
        self.new_value = value.clone();
    }

    pub fn old_value(&self) -> &LVariant {
        &self.old_value
    }

    pub fn set_old_value(&mut self, value: &LVariant) {
        self.old_value = value.clone();
    }

    pub fn target_uid(&self) -> u64 {
        self.target_uid
    }

    pub fn set_target_uid(&mut self, value: u64) {
        self.target_uid = value;
    }

    pub fn is_new_default(&self) -> bool {
        self.new_default
    }

    pub fn set_new_default(&mut self, value: bool) {
        self.new_default = value;
    }

    pub fn is_old_default(&self) -> bool {
        self.old_default
    }

    pub fn set_old_default(&mut self, value: bool) {
        self.old_default = value;
    }

    pub fn is_internal_data_changed(&self) -> bool {
        self.internal_data_changed
    }

    pub fn set_internal_data_changed(&mut self, value: bool) {
        self.internal_data_changed = value;
    }
}

pub trait LPropEventListener {
    fn prop_changed(&mut self, event: &mut LPropEvent);
}

impl<F> LPropEventListener for F
where
    F: FnMut(&mut LPropEvent),
{
    fn prop_changed(&mut self, event: &mut LPropEvent) {
        self(event);
    }
}

type Listener = Box<dyn LPropEventListener>;

#[derive(Default)]
struct PropEventHub {
    next_id: usize,
    listeners: BTreeMap<usize, Listener>,
}

impl PropEventHub {
    fn add(&mut self, listener: Listener) -> usize {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.listeners.insert(id, listener);
        id
    }

    fn remove(&mut self, id: usize) -> bool {
        self.listeners.remove(&id).is_some()
    }

    fn fire(&mut self, event: &mut LPropEvent) {
        for listener in self.listeners.values_mut() {
            listener.prop_changed(event);
        }
    }
}

/// Shared state used by property-supporting objects.
pub struct PropSupportState {
    root_uid: u64,
    this_name: LString,
    default_flags: BTreeMap<LString, bool>,
    local_hub: Rc<RefCell<PropEventHub>>,
    root_hub: Option<Weak<RefCell<PropEventHub>>>,
}

impl Default for PropSupportState {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PropSupportState {
    fn clone(&self) -> Self {
        Self {
            root_uid: self.root_uid,
            this_name: self.this_name.clone(),
            default_flags: self.default_flags.clone(),
            local_hub: Rc::new(RefCell::new(PropEventHub::default())),
            root_hub: None,
        }
    }
}

impl fmt::Debug for PropSupportState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PropSupportState")
            .field("root_uid", &self.root_uid)
            .field("this_name", &self.this_name)
            .field("default_flags", &self.default_flags)
            .finish()
    }
}

impl PropSupportState {
    pub fn new() -> Self {
        Self {
            root_uid: 0,
            this_name: LString::new(),
            default_flags: BTreeMap::new(),
            local_hub: Rc::new(RefCell::new(PropEventHub::default())),
            root_hub: None,
        }
    }

    pub fn root_uid(&self) -> u64 {
        self.root_uid
    }

    pub fn set_root_uid(&mut self, value: u64) {
        self.root_uid = value;
    }

    pub fn this_name(&self) -> &LString {
        &self.this_name
    }

    pub fn is_default(&self, name: &LString) -> bool {
        self.default_flags.get(name).copied().unwrap_or(true)
    }

    pub fn set_default(&mut self, name: LString, value: bool) {
        self.default_flags.insert(name, value);
    }

    pub fn add_listener(&self, listener: Listener) -> usize {
        self.local_hub.borrow_mut().add(listener)
    }

    pub fn remove_listener(&self, id: usize) -> bool {
        self.local_hub.borrow_mut().remove(id)
    }

    fn setup_parent(
        &mut self,
        root_uid: u64,
        name: LString,
        root_hub: Weak<RefCell<PropEventHub>>,
    ) {
        self.root_uid = root_uid;
        self.this_name = name;
        self.root_hub = Some(root_hub);
    }

    fn fire(&self, event: &mut LPropEvent) {
        event.set_target_uid(self.root_uid);
        if let Some(root_hub) = self.root_hub.as_ref().and_then(Weak::upgrade) {
            event.set_parent_name(self.this_name.clone());
            root_hub.borrow_mut().fire(event);
            event.set_parent_name("");
        }
        self.local_hub.borrow_mut().fire(event);
    }
}

/// High-level and implementation-level property interface.
pub trait LPropSupport {
    fn prop_state(&self) -> &PropSupportState;
    fn prop_state_mut(&mut self) -> &mut PropSupportState;

    fn get_property_impl(&self, name: &LString) -> Option<LVariant>;
    fn set_property_impl(&mut self, name: &LString, value: &LVariant) -> bool;
    fn reset_property_impl(&mut self, name: &LString) -> bool;
    fn get_prop_spec_impl(&self, name: &LString) -> Option<PropSpec>;
    fn prop_names(&self) -> BTreeSet<LString>;

    fn get_prop_default(&self, _name: &LString) -> Option<LVariant> {
        None
    }

    fn get_property(&self, name: &LString) -> Option<LVariant> {
        self.get_property_impl(name)
    }

    fn set_property(&mut self, name: &LString, value: &LVariant) -> bool {
        let mut event = LPropEvent::new(name.clone());
        if let Some(old_value) = self.get_property_impl(name) {
            event.set_old_value(&old_value);
        }
        if self.is_prop_default(name) {
            self.set_default_prop_flag(name, false);
            event.set_old_default(true);
        }
        event.set_new_value(value);
        let result = self.set_property_impl(name, value);
        self.setup_parent_data(name);
        self.prop_state().fire(&mut event);
        result
    }

    fn reset_property(&mut self, name: &LString) -> bool {
        if self.is_prop_default(name) {
            return true;
        }
        let mut event = LPropEvent::new(name.clone());
        if let Some(old_value) = self.get_property_impl(name) {
            event.set_old_value(&old_value);
        }
        event.set_new_default(true);
        self.set_default_prop_flag(name, true);
        let result = self.reset_property_impl(name);
        self.setup_parent_data(name);
        self.prop_state().fire(&mut event);
        result
    }

    fn has_property(&self, name: &LString) -> bool {
        self.get_prop_spec_impl(name).is_some()
    }

    fn has_writable_property(&self, name: &LString) -> bool {
        self.get_prop_spec_impl(name)
            .is_some_and(|spec| !spec.read_only)
    }

    fn prop_type_name(&self, name: &LString) -> LString {
        self.get_prop_spec_impl(name)
            .map(|spec| spec.type_name)
            .unwrap_or_default()
    }

    fn has_prop_default(&self, name: &LString) -> bool {
        self.get_prop_spec_impl(name)
            .is_some_and(|spec| spec.has_default)
    }

    fn is_prop_default(&self, name: &LString) -> bool {
        self.prop_state().is_default(name)
    }

    fn set_default_prop_flag(&mut self, name: &LString, value: bool) {
        self.prop_state_mut().set_default(name.clone(), value);
    }

    fn root_uid(&self) -> u64 {
        self.prop_state().root_uid()
    }

    fn add_prop_listener(&self, listener: impl LPropEventListener + 'static) -> usize
    where
        Self: Sized,
    {
        self.prop_state().add_listener(Box::new(listener))
    }

    fn remove_prop_listener(&self, id: usize) -> bool {
        self.prop_state().remove_listener(id)
    }

    fn setup_parent_data(&mut self, name: &LString) {
        let Some(value) = self.get_property_impl(name) else {
            return;
        };
        let Ok(child) = value.get_object::<PropObject>() else {
            return;
        };
        let root_uid = self.root_uid();
        let child_name = LString::from(format!("{}{}", self.prop_state().this_name(), name));
        let root_hub = Rc::downgrade(&self.prop_state().local_hub);
        child
            .inner
            .borrow_mut()
            .prop_state_mut()
            .setup_parent(root_uid, child_name, root_hub);
    }

    fn set_prop_bool(&mut self, name: &str, value: bool) -> bool {
        self.set_property(&name.into(), &LVariant::from(value))
    }

    fn set_prop_int(&mut self, name: &str, value: i32) -> bool {
        self.set_property(&name.into(), &LVariant::from(value))
    }

    fn set_prop_real(&mut self, name: &str, value: f64) -> bool {
        self.set_property(&name.into(), &LVariant::from(value))
    }

    fn set_prop_str(&mut self, name: &str, value: impl Into<LString>) -> bool
    where
        Self: Sized,
    {
        self.set_property(&name.into(), &LVariant::String(value.into()))
    }

    fn get_prop_bool(&self, name: &str) -> Option<bool> {
        self.get_property(&name.into())?.get_bool_value().ok()
    }

    fn get_prop_int(&self, name: &str) -> Option<i32> {
        self.get_property(&name.into())?.get_int_value().ok()
    }

    fn get_prop_real(&self, name: &str) -> Option<f64> {
        self.get_property(&name.into())?.get_real_value().ok()
    }

    fn get_prop_str(&self, name: &str) -> Option<LString> {
        self.get_property(&name.into())
            .and_then(|value| value.get_string_value().ok().cloned())
    }

    fn has_nested_property(&self, name: &LString) -> bool {
        nested_get_target(self, name)
            .is_some_and(|(target, last)| target.borrow().has_property(&last))
    }

    fn has_nested_writable_property(&self, name: &LString) -> bool {
        nested_get_target(self, name)
            .is_some_and(|(target, last)| target.borrow().has_writable_property(&last))
    }

    fn get_nested_property(&self, name: &LString) -> Option<LVariant> {
        let (target, last) = nested_get_target(self, name)?;
        let result = target.borrow().get_property(&last);
        result
    }

    fn set_nested_property(&mut self, name: &LString, value: &LVariant) -> bool {
        let Some((target, last)) = nested_get_target(self, name) else {
            return false;
        };
        let result = target.borrow_mut().set_property(&last, value);
        result
    }

    fn reset_nested_property(&mut self, name: &LString) -> bool {
        let Some((target, last)) = nested_get_target(self, name) else {
            return false;
        };
        let result = target.borrow_mut().reset_property(&last);
        result
    }

    fn has_nested_prop_default(&self, name: &LString) -> bool {
        nested_get_target(self, name)
            .is_some_and(|(target, last)| target.borrow().has_prop_default(&last))
    }

    fn is_nested_prop_default(&self, name: &LString) -> bool {
        nested_get_target(self, name)
            .is_some_and(|(target, last)| target.borrow().is_prop_default(&last))
    }
}

fn nested_get_target(
    root: &(impl LPropSupport + ?Sized),
    name: &LString,
) -> Option<(Rc<RefCell<dyn LPropSupport>>, LString)> {
    let parts = name.as_str().split('.').collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }
    let mut value = root.get_property_impl(&LString::from(parts[0]))?;
    let mut target = value.get_object::<PropObject>().ok()?.inner.clone();
    for part in &parts[1..parts.len() - 1] {
        value = target.borrow().get_property(&LString::from(*part))?;
        target = value.get_object::<PropObject>().ok()?.inner.clone();
    }
    Some((target, LString::from(*parts.last()?)))
}

/// Shared property object stored inside an `LVariant::Object`.
#[derive(Clone)]
pub struct PropObject {
    inner: Rc<RefCell<dyn LPropSupport>>,
    class_name: LString,
}

impl PropObject {
    pub fn new<T: LPropSupport + 'static>(value: T) -> Self {
        Self {
            inner: Rc::new(RefCell::new(value)),
            class_name: LString::new(),
        }
    }

    pub fn new_typed<T: LPropSupport + 'static>(class_name: impl Into<LString>, value: T) -> Self {
        Self {
            inner: Rc::new(RefCell::new(value)),
            class_name: class_name.into(),
        }
    }

    pub fn inner(&self) -> Rc<RefCell<dyn LPropSupport>> {
        self.inner.clone()
    }

    pub fn class_name(&self) -> &LString {
        &self.class_name
    }

    pub fn set_class_name(&mut self, value: impl Into<LString>) {
        self.class_name = value.into();
    }
}

impl fmt::Debug for PropObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("PropObject").finish_non_exhaustive()
    }
}

impl fmt::Display for PropObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[property object]")
    }
}

#[derive(Debug, Clone)]
struct PropertyEntry {
    value: LVariant,
    default_value: Option<LVariant>,
    spec: PropSpec,
}

/// Concrete map-backed property container.
#[derive(Debug, Clone, Default)]
pub struct MapPropContainer {
    state: PropSupportState,
    properties: BTreeMap<LString, PropertyEntry>,
}

impl MapPropContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_root_uid(&mut self, value: u64) {
        self.state.set_root_uid(value);
    }

    pub fn add_property(
        &mut self,
        name: impl Into<LString>,
        spec: PropSpec,
        initial_value: LVariant,
    ) -> bool {
        let name = name.into();
        if self.properties.contains_key(&name) {
            return false;
        }
        let default_value = spec.has_default.then(|| initial_value.clone());
        self.properties.insert(
            name.clone(),
            PropertyEntry {
                value: initial_value,
                default_value,
                spec,
            },
        );
        self.setup_parent_data(&name);
        true
    }

    pub fn remove_property(&mut self, name: &LString) -> bool {
        self.properties.remove(name).is_some()
    }

    pub fn set_property_default(&mut self, name: &LString, value: LVariant) -> bool {
        let Some(entry) = self.properties.get_mut(name) else {
            return false;
        };
        entry.default_value = Some(value);
        entry.spec.has_default = true;
        true
    }
}

impl LPropSupport for MapPropContainer {
    fn prop_state(&self) -> &PropSupportState {
        &self.state
    }

    fn prop_state_mut(&mut self) -> &mut PropSupportState {
        &mut self.state
    }

    fn get_property_impl(&self, name: &LString) -> Option<LVariant> {
        self.properties.get(name).map(|entry| entry.value.clone())
    }

    fn set_property_impl(&mut self, name: &LString, value: &LVariant) -> bool {
        let Some(entry) = self.properties.get_mut(name) else {
            return false;
        };
        if entry.spec.read_only || !variant_matches_type(value, entry.spec.type_name.as_str()) {
            return false;
        }
        entry.value = value.clone();
        true
    }

    fn reset_property_impl(&mut self, name: &LString) -> bool {
        let Some(entry) = self.properties.get_mut(name) else {
            return false;
        };
        let Some(value) = entry.default_value.clone() else {
            return false;
        };
        entry.value = value;
        true
    }

    fn get_prop_spec_impl(&self, name: &LString) -> Option<PropSpec> {
        self.properties.get(name).map(|entry| entry.spec.clone())
    }

    fn prop_names(&self) -> BTreeSet<LString> {
        self.properties.keys().cloned().collect()
    }

    fn get_prop_default(&self, name: &LString) -> Option<LVariant> {
        self.properties
            .get(name)
            .and_then(|entry| entry.default_value.clone())
    }
}

fn variant_matches_type(value: &LVariant, expected: &str) -> bool {
    expected.is_empty()
        || matches!(
            (expected, value),
            ("boolean", LVariant::Boolean(_))
                | ("integer", LVariant::Integer(_))
                | ("real", LVariant::Integer(_) | LVariant::Real(_))
                | ("string", LVariant::String(_))
                | ("enum", LVariant::Enum(_))
                | ("object", LVariant::Object(_) | LVariant::SmartPtr(_))
                | ("array", LVariant::Array(_))
                | ("list", LVariant::List(_))
                | ("dict", LVariant::Dict(_))
        )
}

/// Dynamic property bag corresponding to `qlib::LDynPropContainer`.
#[derive(Debug, Clone, Default)]
pub struct LDynPropContainer {
    properties: BTreeMap<LString, LVariant>,
}

impl LDynPropContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_dyn_prop(&self, name: &LString) -> bool {
        self.properties.contains_key(name)
    }

    pub fn get_dyn_prop(&self, name: &LString) -> Option<LVariant> {
        self.properties.get(name).cloned()
    }

    pub fn set_dyn_prop(&mut self, name: LString, value: LVariant) -> bool {
        self.properties.insert(name, value);
        true
    }

    pub fn remove_dyn_prop(&mut self, name: &LString) -> bool {
        self.properties.remove(name).is_some()
    }

    pub fn dyn_prop_names(&self) -> BTreeSet<LString> {
        self.properties.keys().cloned().collect()
    }

    pub fn dyn_prop_type_name(&self, name: &LString) -> LString {
        self.properties
            .get(name)
            .map(|value| LString::from(value.get_type_string()))
            .unwrap_or_default()
    }

    pub fn set_dyn_prop_bool(&mut self, name: impl Into<LString>, value: bool) -> bool {
        self.set_dyn_prop(name.into(), value.into())
    }

    pub fn set_dyn_prop_int(&mut self, name: impl Into<LString>, value: i32) -> bool {
        self.set_dyn_prop(name.into(), value.into())
    }

    pub fn set_dyn_prop_real(&mut self, name: impl Into<LString>, value: f64) -> bool {
        self.set_dyn_prop(name.into(), value.into())
    }

    pub fn set_dyn_prop_str(
        &mut self,
        name: impl Into<LString>,
        value: impl Into<LString>,
    ) -> bool {
        self.set_dyn_prop(name.into(), LVariant::String(value.into()))
    }

    pub fn get_dyn_prop_bool(&self, name: &LString) -> Result<bool> {
        self.get_dyn_prop(name)
            .ok_or_else(|| prop_not_found(name, "Boolean"))?
            .get_bool_value()
    }

    pub fn get_dyn_prop_int(&self, name: &LString) -> Result<i32> {
        self.get_dyn_prop(name)
            .ok_or_else(|| prop_not_found(name, "Integer"))?
            .get_int_value()
    }

    pub fn get_dyn_prop_real(&self, name: &LString) -> Result<f64> {
        self.get_dyn_prop(name)
            .ok_or_else(|| prop_not_found(name, "Realnum"))?
            .get_real_value()
    }

    pub fn get_dyn_prop_str(&self, name: &LString) -> Result<LString> {
        self.get_dyn_prop(name)
            .ok_or_else(|| prop_not_found(name, "String"))?
            .get_string_value()
            .cloned()
    }
}

fn prop_not_found(name: &LString, property_type: &str) -> CueMolError {
    CueMolError::NotFound(format!("{property_type} property {name} not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_specs_readonly_defaults_and_events_match_reference_flow() {
        let mut object = MapPropContainer::new();
        object.set_root_uid(42);
        object.add_property(
            "count",
            PropSpec::writable("integer").with_default(),
            7.into(),
        );
        object.add_property(
            "name",
            PropSpec::read_only("string"),
            LVariant::from("fixed"),
        );
        let events = Rc::new(RefCell::new(Vec::new()));
        let events_copy = events.clone();
        object.add_prop_listener(move |event: &mut LPropEvent| {
            events_copy.borrow_mut().push(event.clone());
        });

        assert!(object.has_property(&"count".into()));
        assert!(object.has_writable_property(&"count".into()));
        assert!(!object.has_writable_property(&"name".into()));
        assert!(object.is_prop_default(&"count".into()));
        assert_eq!(
            object
                .get_prop_default(&"count".into())
                .unwrap()
                .get_int_value()
                .unwrap(),
            7
        );

        assert!(object.set_prop_int("count", 9));
        assert_eq!(object.get_prop_int("count"), Some(9));
        assert!(!object.is_prop_default(&"count".into()));
        assert!(object.reset_property(&"count".into()));
        assert_eq!(object.get_prop_int("count"), Some(7));
        assert!(object.is_prop_default(&"count".into()));

        let events = events.borrow();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].target_uid(), 42);
        assert!(events[0].is_old_default());
        assert_eq!(events[0].old_value().get_int_value().unwrap(), 7);
        assert_eq!(events[0].new_value().get_int_value().unwrap(), 9);
        assert!(events[1].is_new_default());
        assert_eq!(events[1].old_value().get_int_value().unwrap(), 9);
    }

    #[test]
    fn nested_properties_traverse_objects_and_propagate_events() {
        let mut child = MapPropContainer::new();
        child.add_property(
            "value",
            PropSpec::writable("real").with_default(),
            1.5.into(),
        );
        let mut root = MapPropContainer::new();
        root.set_root_uid(77);
        root.add_property(
            "child",
            PropSpec::writable("object"),
            LVariant::Object(Box::new(PropObject::new(child))),
        );

        let events = Rc::new(RefCell::new(Vec::new()));
        let events_copy = events.clone();
        root.add_prop_listener(move |event: &mut LPropEvent| {
            events_copy
                .borrow_mut()
                .push((event.parent_name().clone(), event.name().clone()));
        });

        assert!(root.has_nested_property(&"child.value".into()));
        assert!(root.has_nested_writable_property(&"child.value".into()));
        assert_eq!(
            root.get_nested_property(&"child.value".into())
                .unwrap()
                .get_real_value()
                .unwrap(),
            1.5
        );
        assert!(root.set_nested_property(&"child.value".into(), &2.25.into()));
        assert_eq!(
            root.get_nested_property(&"child.value".into())
                .unwrap()
                .get_real_value()
                .unwrap(),
            2.25
        );
        assert_eq!(
            events.borrow().as_slice(),
            &[("child".into(), "value".into())]
        );
    }

    #[test]
    fn dynamic_property_container_preserves_types_and_sorted_names() {
        let mut properties = LDynPropContainer::new();
        properties.set_dyn_prop_int("zeta", 3);
        properties.set_dyn_prop_str("alpha", "text");
        properties.set_dyn_prop_real("real", 2.5);
        assert_eq!(properties.get_dyn_prop_int(&"zeta".into()).unwrap(), 3);
        assert_eq!(
            properties
                .get_dyn_prop_str(&"alpha".into())
                .unwrap()
                .as_str(),
            "text"
        );
        assert_eq!(
            properties.dyn_prop_type_name(&"real".into()).as_str(),
            "real"
        );
        assert_eq!(
            properties
                .dyn_prop_names()
                .into_iter()
                .map(|name| name.as_str().to_string())
                .collect::<Vec<_>>(),
            ["alpha", "real", "zeta"]
        );
        assert!(properties.remove_dyn_prop(&"alpha".into()));
        assert!(properties.get_dyn_prop_str(&"alpha".into()).is_err());
    }
}
