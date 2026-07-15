use crate::{CueMolError, EnumDef, LString, LVariant, PropSpec, Result};
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

/// Variant-backed call frame corresponding to `qlib::LVarArgs`.
pub struct LVarArgs {
    data: Vec<LVariant>,
    this_object: Option<Rc<dyn Any>>,
    retval: LVariant,
}

impl fmt::Debug for LVarArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LVarArgs")
            .field("data", &self.data)
            .field("has_this_object", &self.this_object.is_some())
            .field("retval", &self.retval)
            .finish()
    }
}

impl LVarArgs {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![LVariant::Null; size],
            this_object: None,
            retval: LVariant::Null,
        }
    }

    pub fn from_values(values: Vec<LVariant>) -> Self {
        Self {
            data: values,
            this_object: None,
            retval: LVariant::Null,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn check_arg_size(&self, expected: usize) -> Result<()> {
        if self.len() == expected {
            Ok(())
        } else {
            Err(CueMolError::invalid_argument(format!(
                "argument size mismatch {expected} != {}",
                self.len()
            )))
        }
    }

    pub fn get(&self, index: usize) -> &LVariant {
        &self.data[index]
    }

    pub fn at(&mut self, index: usize) -> &mut LVariant {
        &mut self.data[index]
    }

    pub fn retval(&self) -> &LVariant {
        &self.retval
    }

    pub fn retval_mut(&mut self) -> &mut LVariant {
        &mut self.retval
    }

    pub fn set_ret_void(&mut self) {
        self.retval.set_null();
    }

    pub fn set_this<T: Any>(&mut self, object: Rc<T>) {
        self.this_object = Some(object);
    }

    pub fn set_this_object(&mut self, object: Rc<dyn Any>) {
        self.this_object = Some(object);
    }

    pub fn this_object(&self) -> Option<Rc<dyn Any>> {
        self.this_object.clone()
    }

    pub fn get_this<T: Any>(&self) -> Option<Rc<T>> {
        self.this_object.clone()?.downcast::<T>().ok()
    }
}

pub trait LInvokable {
    fn has_method(&self, name: &LString) -> bool;
    fn invoke_method(&mut self, name: &LString, args: &mut LVarArgs) -> Result<bool>;
}

type InvokeFunction = dyn Fn(&mut LVarArgs) -> Result<bool>;

/// Function/property metadata map corresponding to `qlib::FuncMap`.
#[derive(Default)]
pub struct FuncMap {
    functions: BTreeMap<LString, Rc<InvokeFunction>>,
    property_types: BTreeMap<LString, LString>,
    defaults: BTreeMap<LString, LVariant>,
    enums: BTreeMap<(LString, LString), i32>,
}

impl fmt::Debug for FuncMap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FuncMap")
            .field("function_names", &self.functions.keys())
            .field("property_types", &self.property_types)
            .field("defaults", &self.defaults)
            .field("enums", &self.enums)
            .finish()
    }
}

impl FuncMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cleanup(&mut self) {
        self.functions.clear();
        self.property_types.clear();
        self.defaults.clear();
        self.enums.clear();
    }

    pub fn put_func<F>(&mut self, name: impl Into<LString>, function: F) -> bool
    where
        F: Fn(&mut LVarArgs) -> Result<bool> + 'static,
    {
        let name = name.into();
        if self.functions.contains_key(&name) {
            return false;
        }
        self.functions.insert(name, Rc::new(function));
        true
    }

    pub fn put_prop_attr(&mut self, name: impl Into<LString>, value: impl Into<LString>) -> bool {
        let name = name.into();
        if self.property_types.contains_key(&name) {
            return false;
        }
        self.property_types.insert(name, value.into());
        true
    }

    pub fn has_prop(&self, name: impl AsRef<str>) -> bool {
        self.has_function_with_prefix("get_", name.as_ref())
    }

    pub fn has_writable_prop(&self, name: impl AsRef<str>) -> bool {
        self.has_function_with_prefix("set_", name.as_ref())
    }

    pub fn has_method(&self, name: impl AsRef<str>) -> bool {
        self.has_function_with_prefix("mth_", name.as_ref())
    }

    pub fn set_prop<T: Any>(
        &self,
        this_object: Rc<T>,
        name: impl AsRef<str>,
        value: &LVariant,
    ) -> Result<bool> {
        let mut args = LVarArgs::new(1);
        args.set_this(this_object);
        *args.at(0) = value.clone();
        self.execute_prefixed("set_", name.as_ref(), &mut args)
    }

    pub fn get_prop<T: Any>(
        &self,
        this_object: Rc<T>,
        name: impl AsRef<str>,
    ) -> Result<Option<LVariant>> {
        let mut args = LVarArgs::new(0);
        args.set_this(this_object);
        if !self.execute_prefixed("get_", name.as_ref(), &mut args)? {
            return Ok(None);
        }
        Ok(Some(args.retval().clone()))
    }

    pub fn invoke<T: Any>(
        &self,
        this_object: Rc<T>,
        name: impl AsRef<str>,
        args: &mut LVarArgs,
    ) -> Result<bool> {
        args.set_this(this_object);
        self.execute_prefixed("mth_", name.as_ref(), args)
    }

    pub fn property_names(&self) -> Vec<LString> {
        self.property_types
            .keys()
            .filter(|name| !name.as_str().starts_with('@'))
            .cloned()
            .collect()
    }

    pub fn prop_type_name(&self, name: impl AsRef<str>) -> LString {
        self.property_types
            .get(&LString::from(name.as_ref()))
            .cloned()
            .unwrap_or_default()
    }

    pub fn is_no_persist(&self, name: impl AsRef<str>) -> bool {
        self.attribute_is_yes(format!("@{}_nopersist", name.as_ref()))
    }

    pub fn implements(&self, interface_name: impl AsRef<str>) -> bool {
        self.attribute_is_yes(format!("@implement_{}", interface_name.as_ref()))
    }

    pub fn put_default(&mut self, name: impl Into<LString>, value: &LVariant) -> bool {
        self.defaults.insert(name.into(), value.clone()).is_none()
    }

    pub fn default_value(&self, name: impl AsRef<str>) -> Option<LVariant> {
        self.defaults.get(&LString::from(name.as_ref())).cloned()
    }

    pub fn has_default(&self, name: impl AsRef<str>) -> bool {
        self.defaults.contains_key(&LString::from(name.as_ref()))
    }

    pub fn put_enum_def(
        &mut self,
        prop_name: impl Into<LString>,
        enum_name: impl Into<LString>,
        value: i32,
    ) -> bool {
        self.enums
            .insert((prop_name.into(), enum_name.into()), value)
            .is_none()
    }

    pub fn enum_value(
        &self,
        prop_name: impl AsRef<str>,
        enum_name: impl AsRef<str>,
    ) -> Option<i32> {
        self.enums
            .get(&(
                LString::from(prop_name.as_ref()),
                LString::from(enum_name.as_ref()),
            ))
            .copied()
    }

    pub fn enum_name(&self, prop_name: impl AsRef<str>, value: i32) -> Option<LString> {
        let prop_name = LString::from(prop_name.as_ref());
        self.enums.iter().find_map(|((property, name), candidate)| {
            (property == &prop_name && *candidate == value).then(|| name.clone())
        })
    }

    pub fn enum_def(&self, prop_name: impl AsRef<str>) -> EnumDef {
        let prop_name = LString::from(prop_name.as_ref());
        self.enums
            .iter()
            .filter(|((property, _), _)| property == &prop_name)
            .map(|((_, name), value)| (name.clone(), *value))
            .collect()
    }

    pub fn get_prop_spec(&self, name: impl AsRef<str>) -> Option<PropSpec> {
        let name = name.as_ref();
        if !self.has_prop(name) {
            return None;
        }
        let type_name = self.prop_type_name(name);
        let mut spec = PropSpec {
            type_name: type_name.clone(),
            read_only: !self.has_writable_prop(name),
            has_default: self.has_default(name),
            no_persist: self.is_no_persist(name),
            enum_def: None,
        };
        if type_name.as_str() == "enum" {
            spec.enum_def = Some(self.enum_def(name));
        }
        Some(spec)
    }

    pub fn convert_to_bool(source: &LVariant, prop_name: impl AsRef<str>) -> Result<bool> {
        if let LVariant::String(value) = source {
            if ["true", "on", "yes"]
                .iter()
                .any(|candidate| value.equals_ignore_case(candidate))
            {
                return Ok(true);
            }
            if ["false", "off", "no"]
                .iter()
                .any(|candidate| value.equals_ignore_case(candidate))
            {
                return Ok(false);
            }
            return Err(CueMolError::invalid_argument(format!(
                "cannot cast string ({value}) to boolean for {}",
                prop_name.as_ref()
            )));
        }
        source.get_bool_value()
    }

    pub fn convert_to_real(source: &LVariant, prop_name: impl AsRef<str>) -> Result<f64> {
        if let LVariant::String(value) = source {
            return value.as_str().parse::<f64>().map_err(|_| {
                CueMolError::invalid_argument(format!(
                    "cannot cast string {value} to real for {}",
                    prop_name.as_ref()
                ))
            });
        }
        source.get_real_value()
    }

    pub fn convert_to_int(source: &LVariant, prop_name: impl AsRef<str>) -> Result<i32> {
        if let LVariant::String(value) = source {
            return value.as_str().parse::<i32>().map_err(|_| {
                CueMolError::invalid_argument(format!(
                    "cannot cast string {value} to integer for {}",
                    prop_name.as_ref()
                ))
            });
        }
        if source.is_real() {
            return Ok(source.get_real_value()? as i32);
        }
        source.get_int_value()
    }

    fn execute_prefixed(&self, prefix: &str, name: &str, args: &mut LVarArgs) -> Result<bool> {
        let key = LString::from(format!("{prefix}{name}"));
        let Some(function) = self.functions.get(&key) else {
            return Ok(false);
        };
        function(args)
    }

    fn has_function_with_prefix(&self, prefix: &str, name: &str) -> bool {
        self.functions
            .contains_key(&LString::from(format!("{prefix}{name}")))
    }

    fn attribute_is_yes(&self, key: String) -> bool {
        self.property_types
            .get(&LString::from(key))
            .is_some_and(|value| value.as_str() == "yes")
    }
}

pub type LWrapperImpl = FuncMap;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Debug, Default)]
    struct Target {
        value: i32,
    }

    #[test]
    fn var_args_store_values_return_and_typed_this_object() {
        let target = Rc::new(RefCell::new(Target::default()));
        let mut args = LVarArgs::new(2);
        *args.at(0) = 7.into();
        *args.retval_mut() = "done".into();
        args.set_this(Rc::clone(&target));

        assert_eq!(args.len(), 2);
        assert_eq!(args.get(0).get_int_value().unwrap(), 7);
        assert_eq!(args.retval().get_string_value().unwrap().as_str(), "done");
        assert!(Rc::ptr_eq(
            &args.get_this::<RefCell<Target>>().unwrap(),
            &target
        ));
        assert!(args.check_arg_size(2).is_ok());
        assert!(args.check_arg_size(1).is_err());
    }

    #[test]
    fn func_map_dispatches_properties_and_methods_with_call_frame() {
        let mut map = FuncMap::new();
        map.put_func("set_value", |args| {
            args.check_arg_size(1)?;
            let value = args.get(0).get_int_value()?;
            args.get_this::<RefCell<Target>>()
                .unwrap()
                .borrow_mut()
                .value = value;
            Ok(true)
        });
        map.put_func("get_value", |args| {
            args.check_arg_size(0)?;
            let value = args.get_this::<RefCell<Target>>().unwrap().borrow().value;
            *args.retval_mut() = value.into();
            Ok(true)
        });
        map.put_func("mth_add", |args| {
            args.check_arg_size(1)?;
            let delta = args.get(0).get_int_value()?;
            args.get_this::<RefCell<Target>>()
                .unwrap()
                .borrow_mut()
                .value += delta;
            args.set_ret_void();
            Ok(true)
        });
        let target = Rc::new(RefCell::new(Target::default()));

        assert!(map
            .set_prop(Rc::clone(&target), "value", &10.into())
            .unwrap());
        assert_eq!(
            map.get_prop(Rc::clone(&target), "value")
                .unwrap()
                .unwrap()
                .get_int_value()
                .unwrap(),
            10
        );
        let mut args = LVarArgs::from_values(vec![5.into()]);
        assert!(map.invoke(Rc::clone(&target), "add", &mut args).unwrap());
        assert_eq!(target.borrow().value, 15);
        assert!(!map.has_method("missing"));
    }

    #[test]
    fn wrapper_metadata_defaults_enums_and_conversions_match_reference() {
        let mut map = FuncMap::new();
        map.put_func("get_mode", |_| Ok(true));
        map.put_func("set_mode", |_| Ok(true));
        map.put_prop_attr("mode", "enum");
        map.put_prop_attr("@mode_nopersist", "yes");
        map.put_prop_attr("@implement_TestInterface", "yes");
        map.put_default("mode", &LVariant::Enum("foo".into()));
        map.put_enum_def("mode", "foo", 1);
        map.put_enum_def("mode", "bar", 2);

        let spec = map.get_prop_spec("mode").unwrap();
        assert_eq!(spec.type_name.as_str(), "enum");
        assert!(!spec.read_only);
        assert!(spec.has_default);
        assert!(spec.no_persist);
        assert_eq!(spec.enum_def.unwrap().get(&"bar".into()), Some(&2));
        assert!(map.implements("TestInterface"));
        assert_eq!(map.enum_value("mode", "foo"), Some(1));
        assert_eq!(map.enum_name("mode", 2), Some("bar".into()));

        assert!(FuncMap::convert_to_bool(&"YES".into(), "flag").unwrap());
        assert_eq!(
            FuncMap::convert_to_real(&"1.25".into(), "real").unwrap(),
            1.25
        );
        assert_eq!(FuncMap::convert_to_int(&3.9.into(), "int").unwrap(), 3);
        assert!(FuncMap::convert_to_bool(&"invalid".into(), "flag").is_err());
    }
}
