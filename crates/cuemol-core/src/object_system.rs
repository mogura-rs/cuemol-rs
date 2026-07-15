use crate::{CueMolError, LString, Result, Uid, VariantObject};
use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct UidGenerator {
    value: Arc<AtomicU64>,
}

impl UidGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn starting_after(uid: u64) -> Self {
        Self {
            value: Arc::new(AtomicU64::new(uid)),
        }
    }

    pub fn next_uid(&self) -> Uid {
        Uid::new(self.value.fetch_add(1, Ordering::Relaxed) + 1)
    }

    pub fn ensure_after(&self, uid: Uid) {
        self.value.fetch_max(uid.get(), Ordering::Relaxed);
    }

    pub fn current(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for UidGenerator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UidGenerator")
            .field("current", &self.current())
            .finish()
    }
}

impl PartialEq for UidGenerator {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}

impl Eq for UidGenerator {}

type ObjectFactory = dyn Fn() -> Result<Box<dyn VariantObject>>;
type StringFactory = dyn Fn(&LString) -> Result<Box<dyn VariantObject>>;
type ClassHook = dyn Fn();
type ClassInitHook = dyn Fn() -> bool;

/// Runtime class metadata corresponding to `qlib::LClass`.
pub struct LClass {
    class_name: LString,
    abi_name: LString,
    type_id: TypeId,
    singleton: bool,
    factory: Rc<ObjectFactory>,
    string_factory: Option<Rc<StringFactory>>,
    init_hook: Rc<ClassInitHook>,
    fini_hook: Rc<ClassHook>,
}

impl fmt::Debug for LClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LClass")
            .field("class_name", &self.class_name)
            .field("abi_name", &self.abi_name)
            .field("singleton", &self.singleton)
            .finish_non_exhaustive()
    }
}

impl LClass {
    pub fn new<T>(class_name: impl Into<LString>) -> Self
    where
        T: VariantObject + Default + Clone + 'static,
    {
        Self::with_factory::<T, _>(class_name, || Ok(T::default()))
    }

    pub fn with_factory<T, F>(class_name: impl Into<LString>, factory: F) -> Self
    where
        T: VariantObject + Clone + 'static,
        F: Fn() -> Result<T> + 'static,
    {
        Self {
            class_name: class_name.into(),
            abi_name: std::any::type_name::<T>().into(),
            type_id: TypeId::of::<T>(),
            singleton: false,
            factory: Rc::new(move || {
                factory().map(|value| Box::new(value) as Box<dyn VariantObject>)
            }),
            string_factory: None,
            init_hook: Rc::new(|| true),
            fini_hook: Rc::new(|| {}),
        }
    }

    pub fn singleton<T, F>(class_name: impl Into<LString>, factory: F) -> Self
    where
        T: VariantObject + Clone + 'static,
        F: Fn() -> Result<T> + 'static,
    {
        let mut class = Self::with_factory::<T, F>(class_name, factory);
        class.singleton = true;
        class
    }

    pub fn with_string_factory<T, F>(mut self, factory: F) -> Self
    where
        T: VariantObject + Clone + 'static,
        F: Fn(&LString) -> Result<T> + 'static,
    {
        assert_eq!(self.type_id, TypeId::of::<T>());
        self.string_factory = Some(Rc::new(move |source| {
            factory(source).map(|value| Box::new(value) as Box<dyn VariantObject>)
        }));
        self
    }

    pub fn with_hooks<I, F>(mut self, init: I, fini: F) -> Self
    where
        I: Fn() -> bool + 'static,
        F: Fn() + 'static,
    {
        self.init_hook = Rc::new(init);
        self.fini_hook = Rc::new(fini);
        self
    }

    pub fn class_name(&self) -> &LString {
        &self.class_name
    }

    pub fn abi_class_name(&self) -> &LString {
        &self.abi_name
    }

    pub fn is_singleton(&self) -> bool {
        self.singleton
    }

    pub fn create_obj(&self) -> Result<Box<dyn VariantObject>> {
        (self.factory)()
    }

    pub fn create_scr_obj(&self) -> Result<Box<dyn VariantObject>> {
        self.create_obj()
    }

    pub fn create_from_string(&self, source: &LString) -> Result<Box<dyn VariantObject>> {
        self.string_factory.as_ref().map_or_else(
            || {
                Err(CueMolError::invalid_argument(
                    "conversion from string not supported",
                ))
            },
            |factory| factory(source),
        )
    }

    pub fn create_scr_obj_from_string(&self, source: &LString) -> Result<Box<dyn VariantObject>> {
        self.create_from_string(source)
    }

    pub fn is_instance(&self, object: &dyn VariantObject) -> bool {
        object.as_any().type_id() == self.type_id
    }

    fn call_init(&self) -> bool {
        (self.init_hook)()
    }

    fn call_fini(&self) {
        (self.fini_hook)();
    }
}

/// Runtime metaclass registry corresponding to `qlib::ClassRegistry`.
#[derive(Debug, Default)]
pub struct ClassRegistry {
    abi_classes: BTreeMap<LString, Rc<LClass>>,
    named_classes: BTreeMap<LString, Rc<LClass>>,
}

impl ClassRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_class(&mut self, class: LClass) -> bool {
        let class = Rc::new(class);

        if let Some(old) = self
            .abi_classes
            .insert(class.abi_class_name().clone(), Rc::clone(&class))
        {
            self.named_classes.remove(old.class_name());
            old.call_fini();
        }

        if let Some(old) = self
            .named_classes
            .insert(class.class_name().clone(), Rc::clone(&class))
        {
            if old.abi_class_name() != class.abi_class_name() {
                self.abi_classes.remove(old.abi_class_name());
                old.call_fini();
            }
        }

        class.call_init();
        true
    }

    pub fn unregister_by_abi_name(&mut self, abi_name: impl AsRef<str>) -> bool {
        let key = LString::from(abi_name.as_ref());
        let Some(class) = self.abi_classes.remove(&key) else {
            return false;
        };
        self.named_classes.remove(class.class_name());
        class.call_fini();
        true
    }

    pub fn unregister<T: 'static>(&mut self) -> bool {
        self.unregister_by_abi_name(std::any::type_name::<T>())
    }

    pub fn is_registered(&self, class_name: impl AsRef<str>) -> bool {
        self.named_classes
            .contains_key(&LString::from(class_name.as_ref()))
    }

    pub fn is_type_registered<T: 'static>(&self) -> bool {
        self.abi_classes
            .contains_key(&LString::from(std::any::type_name::<T>()))
    }

    pub fn get_class(&self, class_name: impl AsRef<str>) -> Result<Rc<LClass>> {
        self.get_class_nx(class_name.as_ref()).ok_or_else(|| {
            CueMolError::not_found(format!("class \"{}\" is not found", class_name.as_ref()))
        })
    }

    pub fn get_class_nx(&self, class_name: impl AsRef<str>) -> Option<Rc<LClass>> {
        self.named_classes
            .get(&LString::from(class_name.as_ref()))
            .cloned()
    }

    pub fn get_class_by_abi_name(&self, abi_name: impl AsRef<str>) -> Result<Rc<LClass>> {
        self.get_class_by_abi_name_nx(abi_name.as_ref())
            .ok_or_else(|| {
                CueMolError::not_found(format!("class \"{}\" is not found", abi_name.as_ref()))
            })
    }

    pub fn get_class_by_abi_name_nx(&self, abi_name: impl AsRef<str>) -> Option<Rc<LClass>> {
        self.abi_classes
            .get(&LString::from(abi_name.as_ref()))
            .cloned()
    }

    pub fn create_obj(&self, class_name: impl AsRef<str>) -> Result<Box<dyn VariantObject>> {
        let class = self.get_class(class_name.as_ref())?;
        if class.is_singleton() {
            return Err(CueMolError::invalid_argument(format!(
                "cannot instantiate a singleton class \"{}\"",
                class_name.as_ref()
            )));
        }
        class.create_obj()
    }

    pub fn get_singleton_obj(&self, class_name: impl AsRef<str>) -> Result<Box<dyn VariantObject>> {
        let class = self.get_class(class_name.as_ref())?;
        if !class.is_singleton() {
            return Err(CueMolError::invalid_argument(format!(
                "\"{}\" is not a singleton class",
                class_name.as_ref()
            )));
        }
        class.create_obj()
    }

    pub fn all_class_names(&self) -> Vec<LString> {
        self.named_classes.keys().cloned().collect()
    }
}

impl Drop for ClassRegistry {
    fn drop(&mut self) {
        for class in self.abi_classes.values() {
            class.call_fini();
        }
    }
}

/// UID-based object table corresponding to `qlib::ObjectManager`.
#[derive(Debug, Default)]
pub struct ObjectManager {
    uid_generator: UidGenerator,
    objects: BTreeMap<Uid, Rc<dyn Any>>,
}

impl ObjectManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_new_uid(&mut self) -> Uid {
        self.uid_generator.next_uid()
    }

    pub fn register_object<T: Any>(&mut self, object: Rc<T>) -> Uid {
        let uid = self.create_new_uid();
        self.objects.insert(uid, object);
        uid
    }

    pub fn get_object_by_uid(&self, uid: Uid) -> Option<Rc<dyn Any>> {
        self.objects.get(&uid).cloned()
    }

    pub fn get_typed_object<T: Any>(&self, uid: Uid) -> Option<Rc<T>> {
        self.get_object_by_uid(uid)?.downcast::<T>().ok()
    }

    pub fn unregister_object(&mut self, uid: Uid) -> bool {
        self.objects.remove(&uid).is_some()
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamedObject {
    name: LString,
}

impl NamedObject {
    pub fn new(name: impl Into<LString>) -> Self {
        Self { name: name.into() }
    }

    pub fn name(&self) -> &LString {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<LString>) {
        self.name = name.into();
    }

    pub fn is_valid_name(name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        !name.is_empty() && !name.contains(':')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    struct TestObject {
        value: i32,
    }

    impl fmt::Display for TestObject {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "{}", self.value)
        }
    }

    #[test]
    fn class_registry_creates_and_finds_objects_by_both_names() {
        let class =
            LClass::new::<TestObject>("qlib.TestObject").with_string_factory(|source: &LString| {
                Ok(TestObject {
                    value: source.as_str().parse().unwrap(),
                })
            });
        let abi_name = class.abi_class_name().clone();
        let mut registry = ClassRegistry::new();

        assert!(registry.register_class(class));
        assert!(registry.is_registered("qlib.TestObject"));
        assert!(registry.is_type_registered::<TestObject>());
        assert_eq!(registry.all_class_names(), vec!["qlib.TestObject".into()]);

        let object = registry.create_obj("qlib.TestObject").unwrap();
        assert_eq!(
            object
                .as_ref()
                .as_any()
                .downcast_ref::<TestObject>()
                .unwrap()
                .value,
            0
        );
        let class = registry.get_class_by_abi_name(&abi_name).unwrap();
        let parsed = class.create_from_string(&"42".into()).unwrap();
        assert_eq!(
            parsed
                .as_ref()
                .as_any()
                .downcast_ref::<TestObject>()
                .unwrap()
                .value,
            42
        );
        assert!(class.is_instance(parsed.as_ref()));
    }

    #[test]
    fn registration_overwrite_and_unregister_call_class_hooks() {
        let init_count = Rc::new(Cell::new(0));
        let fini_count = Rc::new(Cell::new(0));
        let make_class = || {
            let init_count = Rc::clone(&init_count);
            let fini_count = Rc::clone(&fini_count);
            LClass::new::<TestObject>("TestObject").with_hooks(
                move || {
                    init_count.set(init_count.get() + 1);
                    true
                },
                move || fini_count.set(fini_count.get() + 1),
            )
        };
        let mut registry = ClassRegistry::new();

        registry.register_class(make_class());
        registry.register_class(make_class());
        assert_eq!(init_count.get(), 2);
        assert_eq!(fini_count.get(), 1);
        assert!(registry.unregister::<TestObject>());
        assert_eq!(fini_count.get(), 2);
        assert!(!registry.unregister::<TestObject>());
    }

    #[test]
    fn singleton_and_normal_class_creation_are_kept_separate() {
        let mut registry = ClassRegistry::new();
        registry.register_class(LClass::singleton::<TestObject, _>("Singleton", || {
            Ok(TestObject { value: 7 })
        }));

        assert!(registry.create_obj("Singleton").is_err());
        let singleton = registry.get_singleton_obj("Singleton").unwrap();
        assert_eq!(
            singleton
                .as_ref()
                .as_any()
                .downcast_ref::<TestObject>()
                .unwrap()
                .value,
            7
        );
    }

    #[test]
    fn object_manager_generates_stable_uids_and_checks_types() {
        let object = Rc::new(TestObject { value: 9 });
        let mut manager = ObjectManager::new();
        let uid = manager.register_object(Rc::clone(&object));

        assert_eq!(uid.get(), 1);
        assert_eq!(manager.len(), 1);
        assert!(Rc::ptr_eq(
            &manager.get_typed_object::<TestObject>(uid).unwrap(),
            &object
        ));
        assert!(manager.get_typed_object::<String>(uid).is_none());
        assert!(manager.unregister_object(uid));
        assert!(!manager.unregister_object(uid));
    }

    #[test]
    fn shared_uid_generator_is_monotonic_and_reserves_explicit_ids() {
        let generator = UidGenerator::new();
        let shared = generator.clone();
        assert_eq!(generator.next_uid(), Uid::new(1));
        assert_eq!(shared.next_uid(), Uid::new(2));
        generator.ensure_after(Uid::new(100));
        assert_eq!(shared.next_uid(), Uid::new(101));
        generator.ensure_after(Uid::new(50));
        assert_eq!(generator.next_uid(), Uid::new(102));
    }

    #[test]
    fn named_object_validates_reference_name_rules() {
        let mut object = NamedObject::new("first");
        object.set_name("second");
        assert_eq!(object.name(), &LString::from("second"));
        assert!(NamedObject::is_valid_name("second"));
        assert!(!NamedObject::is_valid_name(""));
        assert!(!NamedObject::is_valid_name("scope:name"));
    }
}
