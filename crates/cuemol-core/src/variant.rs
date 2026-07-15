use crate::{CueMolError, LString, Result};
use std::any::Any;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// Cloneable script object stored by an [`LVariant`].
pub trait VariantObject: fmt::Debug + fmt::Display {
    fn clone_box(&self) -> Box<dyn VariantObject>;
    fn as_any(&self) -> &dyn Any;
    fn type_name(&self) -> &'static str;
}

impl<T> VariantObject for T
where
    T: fmt::Debug + fmt::Display + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn VariantObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

impl Clone for Box<dyn VariantObject> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

/// Dynamically typed value corresponding to `qlib::LVariant`.
#[derive(Debug, Clone, Default)]
pub enum LVariant {
    #[default]
    Null,
    Boolean(bool),
    Integer(i32),
    Real(f64),
    String(LString),
    Object(Box<dyn VariantObject>),
    SmartPtr(Box<dyn VariantObject>),
    Enum(LString),
    Array(LVarArray),
    List(LVarList),
    Dict(LVarDict),
}

// The reference implementation intentionally defines variant comparison as a
// dummy operation that always returns false.
impl PartialEq for LVariant {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl LVariant {
    pub const LT_NULL: i32 = 0;
    pub const LT_BOOLEAN: i32 = 1;
    pub const LT_INTEGER: i32 = 2;
    pub const LT_REAL: i32 = 3;
    pub const LT_STRING: i32 = 4;
    pub const LT_OBJECT: i32 = 5;
    pub const LT_SMARTPTR: i32 = 6;
    pub const LT_ENUM: i32 = 7;
    pub const LT_ARRAY: i32 = 8;
    pub const LT_LIST: i32 = 9;
    pub const LT_DICT: i32 = 10;

    pub fn cleanup(&mut self) {
        *self = Self::Null;
    }

    pub fn forget(&mut self) {
        *self = Self::Null;
    }

    pub fn copy_from(&mut self, source: &Self) {
        *self = source.clone();
    }

    pub fn get_type_id(&self) -> i32 {
        match self {
            Self::Null => Self::LT_NULL,
            Self::Boolean(_) => Self::LT_BOOLEAN,
            Self::Integer(_) => Self::LT_INTEGER,
            Self::Real(_) => Self::LT_REAL,
            Self::String(_) => Self::LT_STRING,
            Self::Object(_) => Self::LT_OBJECT,
            Self::SmartPtr(_) => Self::LT_SMARTPTR,
            Self::Enum(_) => Self::LT_ENUM,
            Self::Array(_) => Self::LT_ARRAY,
            Self::List(_) => Self::LT_LIST,
            Self::Dict(_) => Self::LT_DICT,
        }
    }

    pub fn get_type_string(&self) -> String {
        match self {
            Self::Null => "null".to_string(),
            Self::Boolean(_) => "boolean".to_string(),
            Self::Integer(_) => "integer".to_string(),
            Self::Real(_) => "real".to_string(),
            Self::String(_) => "string".to_string(),
            Self::Object(value) | Self::SmartPtr(value) => {
                format!("object({})", value.as_ref().type_name())
            }
            Self::Enum(_) => "enum".to_string(),
            Self::Array(_) => "array".to_string(),
            Self::List(_) => "list".to_string(),
            Self::Dict(_) => "dict".to_string(),
        }
    }

    pub fn is_owned(&self) -> bool {
        true
    }

    pub fn is_str_conv(&self) -> bool {
        !matches!(self, Self::Array(_) | Self::List(_) | Self::Dict(_))
    }

    pub fn to_string_value(&self) -> LString {
        match self {
            Self::Null => LString::from("null"),
            Self::Boolean(value) => LString::from(if *value { "true" } else { "false" }),
            Self::Integer(value) => LString::from_int(*value),
            Self::Real(value) => LString::from_real(*value, 6),
            Self::String(value) | Self::Enum(value) => value.clone(),
            Self::Object(value) | Self::SmartPtr(value) => LString::from(value.to_string()),
            Self::Array(value) => LString::from(format!("array(len={})", value.len())),
            Self::List(value) => LString::from(format!("list(len={})", value.len())),
            Self::Dict(value) => LString::from(format!("dict(len={})", value.len())),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn set_null(&mut self) {
        *self = Self::Null;
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Boolean(_))
    }

    pub fn set_bool_value(&mut self, value: bool) {
        *self = Self::Boolean(value);
    }

    pub fn get_bool_value(&self) -> Result<bool> {
        match self {
            Self::Boolean(value) => Ok(*value),
            _ => Err(self.invalid_cast("boolean")),
        }
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    pub fn set_int_value(&mut self, value: i32) {
        *self = Self::Integer(value);
    }

    pub fn get_int_value(&self) -> Result<i32> {
        match self {
            Self::Integer(value) => Ok(*value),
            _ => Err(self.invalid_cast("integer")),
        }
    }

    /// Integers are real-compatible in the reference implementation.
    pub fn is_real(&self) -> bool {
        matches!(self, Self::Integer(_) | Self::Real(_))
    }

    pub fn set_real_value(&mut self, value: f64) {
        *self = Self::Real(value);
    }

    pub fn get_real_value(&self) -> Result<f64> {
        match self {
            Self::Real(value) => Ok(*value),
            Self::Integer(value) => Ok(f64::from(*value)),
            _ => Err(self.invalid_cast("real number")),
        }
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn set_string_value(&mut self, value: impl Into<LString>) {
        *self = Self::String(value.into());
    }

    pub fn get_string_value(&self) -> Result<&LString> {
        match self {
            Self::String(value) => Ok(value),
            _ => Err(self.invalid_cast("string")),
        }
    }

    pub fn is_enum(&self) -> bool {
        matches!(self, Self::Enum(_))
    }

    pub fn set_enum_value(&mut self, value: impl Into<LString>) {
        *self = Self::Enum(value.into());
    }

    pub fn get_enum_value(&self) -> Result<&LString> {
        match self {
            Self::Enum(value) => Ok(value),
            _ => Err(self.invalid_cast("enum")),
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    pub fn set_array_value(&mut self, value: &LVarArray) {
        *self = Self::Array(value.clone());
    }

    pub fn get_array(&self) -> Result<&LVarArray> {
        match self {
            Self::Array(value) => Ok(value),
            _ => Err(self.invalid_cast("array")),
        }
    }

    pub fn get_array_mut(&mut self) -> Result<&mut LVarArray> {
        let actual = self.get_type_string();
        match self {
            Self::Array(value) => Ok(value),
            _ => Err(CueMolError::invalid_argument(format!(
                "cannot cast type {actual:?} to array"
            ))),
        }
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    pub fn set_list_value(&mut self, value: &LVarList) {
        *self = Self::List(value.clone());
    }

    pub fn get_list(&self) -> Result<&LVarList> {
        match self {
            Self::List(value) => Ok(value),
            _ => Err(self.invalid_cast("list")),
        }
    }

    pub fn get_list_mut(&mut self) -> Result<&mut LVarList> {
        let actual = self.get_type_string();
        match self {
            Self::List(value) => Ok(value),
            _ => Err(CueMolError::invalid_argument(format!(
                "cannot cast type {actual:?} to list"
            ))),
        }
    }

    pub fn is_dict(&self) -> bool {
        matches!(self, Self::Dict(_))
    }

    pub fn set_dict_value(&mut self, value: &LVarDict) {
        *self = Self::Dict(value.clone());
    }

    pub fn get_dict(&self) -> Result<&LVarDict> {
        match self {
            Self::Dict(value) => Ok(value),
            _ => Err(self.invalid_cast("dict")),
        }
    }

    pub fn get_dict_mut(&mut self) -> Result<&mut LVarDict> {
        let actual = self.get_type_string();
        match self {
            Self::Dict(value) => Ok(value),
            _ => Err(CueMolError::invalid_argument(format!(
                "cannot cast type {actual:?} to dict"
            ))),
        }
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_) | Self::SmartPtr(_))
    }

    pub fn set_object<T>(&mut self, value: T)
    where
        T: VariantObject + Clone + 'static,
    {
        *self = Self::Object(Box::new(value));
    }

    pub fn get_object<T: 'static>(&self) -> Result<&T> {
        match self {
            Self::Object(value) | Self::SmartPtr(value) => value
                .as_ref()
                .as_any()
                .downcast_ref::<T>()
                .ok_or_else(|| self.invalid_cast(std::any::type_name::<T>())),
            _ => Err(self.invalid_cast("object")),
        }
    }

    fn invalid_cast(&self, expected: &str) -> CueMolError {
        CueMolError::invalid_argument(format!(
            "cannot cast type {:?} to {expected}",
            self.get_type_string()
        ))
    }
}

impl fmt::Display for LVariant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.to_string_value().as_str())
    }
}

impl From<bool> for LVariant {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<i32> for LVariant {
    fn from(value: i32) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for LVariant {
    fn from(value: f64) -> Self {
        Self::Real(value)
    }
}

impl From<&str> for LVariant {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}

impl From<String> for LVariant {
    fn from(value: String) -> Self {
        Self::String(value.into())
    }
}

impl From<LString> for LVariant {
    fn from(value: LString) -> Self {
        Self::String(value)
    }
}

/// Scriptable, zero-based array of variants.
#[derive(Debug, Clone, Default)]
pub struct LVarArray {
    values: Vec<LVariant>,
}

impl LVarArray {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_size(size: usize) -> Self {
        Self {
            values: vec![LVariant::Null; size],
        }
    }

    pub fn from_values(values: Vec<LVariant>) -> Self {
        Self { values }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn get_size(&self) -> usize {
        self.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn at(&self, index: usize) -> Result<&LVariant> {
        self.values
            .get(index)
            .ok_or_else(|| CueMolError::invalid_argument("array index out of range"))
    }

    pub fn at_mut(&mut self, index: usize) -> Result<&mut LVariant> {
        self.values
            .get_mut(index)
            .ok_or_else(|| CueMolError::invalid_argument("array index out of range"))
    }

    pub fn push(&mut self, value: impl Into<LVariant>) {
        self.values.push(value.into());
    }

    pub fn is_null(&self, index: usize) -> Result<bool> {
        Ok(self.at(index)?.is_null())
    }

    pub fn set_null(&mut self, index: usize) -> Result<()> {
        self.at_mut(index)?.set_null();
        Ok(())
    }

    pub fn is_bool(&self, index: usize) -> Result<bool> {
        Ok(self.at(index)?.is_bool())
    }

    pub fn set_bool_value(&mut self, index: usize, value: bool) -> Result<()> {
        self.at_mut(index)?.set_bool_value(value);
        Ok(())
    }

    pub fn get_bool_value(&self, index: usize) -> Result<bool> {
        self.at(index)?.get_bool_value()
    }

    pub fn is_int(&self, index: usize) -> Result<bool> {
        Ok(self.at(index)?.is_int())
    }

    pub fn set_int_value(&mut self, index: usize, value: i32) -> Result<()> {
        self.at_mut(index)?.set_int_value(value);
        Ok(())
    }

    pub fn get_int_value(&self, index: usize) -> Result<i32> {
        self.at(index)?.get_int_value()
    }

    pub fn is_real(&self, index: usize) -> Result<bool> {
        Ok(self.at(index)?.is_real())
    }

    pub fn set_real_value(&mut self, index: usize, value: f64) -> Result<()> {
        self.at_mut(index)?.set_real_value(value);
        Ok(())
    }

    pub fn get_real_value(&self, index: usize) -> Result<f64> {
        self.at(index)?.get_real_value()
    }

    pub fn is_string(&self, index: usize) -> Result<bool> {
        Ok(self.at(index)?.is_string())
    }

    pub fn set_string_value(&mut self, index: usize, value: impl Into<LString>) -> Result<()> {
        self.at_mut(index)?.set_string_value(value);
        Ok(())
    }

    pub fn get_string_value(&self, index: usize) -> Result<&LString> {
        self.at(index)?.get_string_value()
    }

    pub fn is_enum(&self, index: usize) -> Result<bool> {
        Ok(self.at(index)?.is_enum())
    }

    pub fn set_enum_value(&mut self, index: usize, value: impl Into<LString>) -> Result<()> {
        self.at_mut(index)?.set_enum_value(value);
        Ok(())
    }

    pub fn get_enum_value(&self, index: usize) -> Result<&LString> {
        self.at(index)?.get_enum_value()
    }

    pub fn iter(&self) -> impl Iterator<Item = &LVariant> {
        self.values.iter()
    }
}

/// Scriptable list that preserves the reference implementation's nullable slots.
#[derive(Debug, Clone, Default)]
pub struct LVarList {
    values: VecDeque<Option<LVariant>>,
}

impl LVarList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_size(size: usize) -> Self {
        Self {
            values: std::iter::repeat_with(|| None).take(size).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn push_back(&mut self, value: impl Into<LVariant>) {
        self.values.push_back(Some(value.into()));
    }

    pub fn push_front(&mut self, value: impl Into<LVariant>) {
        self.values.push_front(Some(value.into()));
    }

    pub fn push_null_back(&mut self) {
        self.values.push_back(None);
    }

    pub fn front_pop_front(&mut self) -> Option<LVariant> {
        self.values.pop_front().flatten()
    }

    pub fn back_pop_back(&mut self) -> Option<LVariant> {
        self.values.pop_back().flatten()
    }

    pub fn at(&self, index: usize) -> Result<&LVariant> {
        self.values
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| CueMolError::NotFound(format!("list index {index}")))
    }

    pub fn get_string(&self, index: usize) -> Result<&LString> {
        self.at(index)?.get_string_value()
    }

    pub fn get_int(&self, index: usize) -> Result<i32> {
        self.at(index)?.get_int_value()
    }

    pub fn get_real(&self, index: usize) -> Result<f64> {
        self.at(index)?.get_real_value()
    }

    pub fn get_list(&self, index: usize) -> Result<&LVarList> {
        self.at(index)?.get_list()
    }

    pub fn iter(&self) -> impl Iterator<Item = Option<&LVariant>> {
        self.values.iter().map(Option::as_ref)
    }
}

/// Ordered string-keyed dictionary of variants.
#[derive(Debug, Clone, Default)]
pub struct LVarDict {
    values: BTreeMap<LString, LVariant>,
}

impl LVarDict {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Insert without replacing an existing key, matching `MapTable::set`.
    pub fn set(&mut self, key: impl Into<LString>, value: impl Into<LVariant>) -> bool {
        use std::collections::btree_map::Entry;
        match self.values.entry(key.into()) {
            Entry::Vacant(entry) => {
                entry.insert(value.into());
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    pub fn force_set(&mut self, key: impl Into<LString>, value: impl Into<LVariant>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn contains_key(&self, key: impl AsRef<str>) -> bool {
        self.values.keys().any(|item| item.as_str() == key.as_ref())
    }

    pub fn get(&self, key: impl AsRef<str>) -> Result<&LVariant> {
        self.values
            .iter()
            .find(|(item, _)| item.as_str() == key.as_ref())
            .map(|(_, value)| value)
            .ok_or_else(|| CueMolError::NotFound(format!("dict key {}", key.as_ref())))
    }

    /// Return a copied value or a null variant for an absent key.
    pub fn get_value(&self, key: impl AsRef<str>) -> LVariant {
        self.get(key).cloned().unwrap_or_default()
    }

    pub fn get_mut(&mut self, key: impl AsRef<str>) -> Result<&mut LVariant> {
        let key = key.as_ref();
        self.values
            .iter_mut()
            .find(|(item, _)| item.as_str() == key)
            .map(|(_, value)| value)
            .ok_or_else(|| CueMolError::NotFound(format!("dict key {key}")))
    }

    pub fn remove_value(&mut self, key: impl AsRef<str>) -> Option<LVariant> {
        let stored = self
            .values
            .keys()
            .find(|item| item.as_str() == key.as_ref())
            .cloned()?;
        self.values.remove(&stored)
    }

    pub fn remove(&mut self, key: impl AsRef<str>) -> bool {
        self.remove_value(key).is_some()
    }

    pub fn topkey(&self) -> Result<LString> {
        self.values
            .first_key_value()
            .map(|(key, _)| key.clone())
            .ok_or_else(|| CueMolError::NotFound("dict is empty".to_string()))
    }

    pub fn get_string(&self, key: impl AsRef<str>) -> Result<&LString> {
        self.get(key)?.get_string_value()
    }

    pub fn get_int(&self, key: impl AsRef<str>) -> Result<i32> {
        self.get(key)?.get_int_value()
    }

    pub fn get_real(&self, key: impl AsRef<str>) -> Result<f64> {
        self.get(key)?.get_real_value()
    }

    pub fn get_list(&self, key: impl AsRef<str>) -> Result<&LVarList> {
        self.get(key)?.get_list()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&LString, &LVariant)> {
        self.values.iter()
    }
}

pub type LArray = LVarArray;
pub type LDict = LVarDict;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestObject {
        value: i32,
    }

    impl fmt::Display for TestObject {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "TestObject({})", self.value)
        }
    }

    #[test]
    fn scalar_type_ids_casts_and_strings_match_reference() {
        let mut value = LVariant::default();
        assert!(value.is_null());
        assert_eq!(value.get_type_id(), LVariant::LT_NULL);
        assert_eq!(value.to_string_value().as_str(), "null");

        value.set_int_value(42);
        assert!(value.is_int());
        assert!(value.is_real());
        assert_eq!(value.get_real_value().unwrap(), 42.0);
        assert_eq!(value.to_string_value().as_str(), "42");
        assert!(value.get_bool_value().is_err());

        value.set_real_value(1.25);
        assert_eq!(value.to_string_value().as_str(), "1.25");
        value.set_enum_value("alpha");
        assert_eq!(value.get_type_string(), "enum");
        assert_eq!(value.get_enum_value().unwrap().as_str(), "alpha");
    }

    #[test]
    fn arrays_lists_and_dicts_are_typed_and_deep_cloned() {
        let mut array = LVarArray::with_size(3);
        array.set_bool_value(0, true).unwrap();
        array.set_int_value(1, 7).unwrap();
        array.set_string_value(2, "text").unwrap();
        assert!(array.get_bool_value(0).unwrap());
        assert_eq!(array.get_real_value(1).unwrap(), 7.0);
        assert_eq!(array.get_string_value(2).unwrap().as_str(), "text");

        let mut list = LVarList::new();
        list.push_back("first");
        list.push_back(9_i32);
        assert_eq!(list.get_string(0).unwrap().as_str(), "first");
        assert_eq!(list.get_int(1).unwrap(), 9);

        let mut dict = LVarDict::new();
        dict.set("array", LVariant::Array(array));
        dict.set("list", LVariant::List(list));
        dict.set("real", 2.5_f64);
        let mut clone = dict.clone();
        clone.get_mut("real").unwrap().set_real_value(8.0);
        assert_eq!(dict.get_real("real").unwrap(), 2.5);
        assert_eq!(clone.get_real("real").unwrap(), 8.0);
        assert_eq!(
            LVariant::Dict(dict).to_string_value().as_str(),
            "dict(len=3)"
        );
    }

    #[test]
    fn object_variants_clone_and_downcast() {
        let mut value = LVariant::Null;
        value.set_object(TestObject { value: 12 });
        assert!(value.is_object());
        assert_eq!(value.to_string_value().as_str(), "TestObject(12)");
        assert_eq!(value.get_object::<TestObject>().unwrap().value, 12);
        let clone = value.clone();
        assert_eq!(clone.get_object::<TestObject>().unwrap().value, 12);
        assert_ne!(value, clone);
    }
}
