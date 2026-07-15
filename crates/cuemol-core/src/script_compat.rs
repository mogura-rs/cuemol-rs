use crate::{LVarArgs, Result, Vector4};

pub trait LScrObject {}

pub trait LScrCallBack: LScrObject {
    fn invoke(&self, _args: &mut LVarArgs) -> Result<bool> {
        Ok(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassAEnum {
    Foo = 0,
    Bar = 1,
    Baz = 2,
    Qux = 3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassA {
    prop0: bool,
    prop1: i32,
    realprop: f64,
    strprop: String,
    vecprop: Vector4,
    prop3: Option<Box<ClassA>>,
    prop4: i32,
    enumprop: ClassAEnum,
}

impl Default for ClassA {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassA {
    pub const ENUM_TYPE0: i32 = 0;
    pub const ENUM_TYPE1: i32 = 1;
    pub const ENUM_TYPE2: i32 = 2;
    pub const ENUM_TYPE3: i32 = 3;

    pub fn new() -> Self {
        Self {
            prop0: false,
            prop1: 246,
            realprop: 345.678,
            strprop: "abcd".to_string(),
            vecprop: Vector4::ZERO,
            prop3: None,
            prop4: 123,
            enumprop: ClassAEnum::Bar,
        }
    }

    pub fn prop0(&self) -> bool {
        self.prop0
    }

    pub fn prop_0(&self) -> bool {
        self.prop0()
    }

    pub fn set_prop0(&mut self, value: bool) {
        self.prop0 = value;
    }

    pub fn set_prop_0(&mut self, value: bool) {
        self.set_prop0(value);
    }

    pub fn prop1(&self) -> i32 {
        self.prop1
    }

    pub fn prop_1(&self) -> i32 {
        self.prop1()
    }

    pub fn set_prop1(&mut self, value: i32) {
        self.prop1 = value;
    }

    pub fn set_prop_1(&mut self, value: i32) {
        self.set_prop1(value);
    }

    pub fn realprop(&self) -> f64 {
        self.realprop
    }

    pub fn set_realprop(&mut self, value: f64) {
        self.realprop = value;
    }

    pub fn strprop(&self) -> &str {
        &self.strprop
    }

    pub fn set_strprop(&mut self, value: impl Into<String>) {
        self.strprop = value.into();
    }

    pub fn vecprop(&self) -> Vector4 {
        self.vecprop
    }

    pub fn set_vecprop(&mut self, value: Vector4) {
        self.vecprop = value;
    }

    pub fn prop3(&self) -> Option<&ClassA> {
        self.prop3.as_deref()
    }

    pub fn prop_3(&self) -> Option<&ClassA> {
        self.prop3()
    }

    pub fn set_prop3(&mut self, value: Option<ClassA>) {
        self.prop3 = value.map(Box::new);
    }

    pub fn set_prop_3(&mut self, value: Option<ClassA>) {
        self.set_prop3(value);
    }

    pub fn prop4(&self) -> i32 {
        self.prop4
    }

    pub fn prop_4(&self) -> i32 {
        self.prop4()
    }

    pub fn set_prop4(&mut self, value: i32) {
        self.prop4 = value;
    }

    pub fn set_prop_4(&mut self, value: i32) {
        self.set_prop4(value);
    }

    pub fn enumprop(&self) -> ClassAEnum {
        self.enumprop
    }

    pub fn set_enumprop(&mut self, value: ClassAEnum) {
        self.enumprop = value;
    }

    pub fn foo(&mut self, value: i32, text: impl AsRef<str>, real: f64) {
        self.strprop = format!("value is {value} and {}", text.as_ref());
        self.realprop = real;
    }

    pub fn foo2(&mut self, value: &ClassA, shared: Option<&ClassA>) {
        self.prop4 = value.prop1() + shared.map_or(0, ClassA::prop1);
    }

    pub fn foo_2(&mut self, value: &ClassA, shared: Option<&ClassA>) {
        self.foo2(value, shared);
    }

    pub fn foo3(&self) -> Option<&ClassA> {
        self.prop3()
    }

    pub fn foo_3(&self) -> Option<&ClassA> {
        self.foo3()
    }
}

impl std::fmt::Display for ClassA {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "ClassA({}, {})", self.prop1, self.strprop)
    }
}

impl LScrObject for ClassA {}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassB {
    base: ClassA,
    last_bar: Option<f64>,
}

impl Default for ClassB {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassB {
    pub fn new() -> Self {
        Self {
            base: ClassA::new(),
            last_bar: None,
        }
    }

    pub fn base(&self) -> &ClassA {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut ClassA {
        &mut self.base
    }

    pub fn bar(&mut self, value: f64) {
        self.last_bar = Some(value);
    }

    pub fn last_bar(&self) -> Option<f64> {
        self.last_bar
    }
}

impl std::ops::Deref for ClassB {
    type Target = ClassA;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for ClassB {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl std::fmt::Display for ClassB {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "ClassB ({})", self.base)
    }
}

impl LScrObject for ClassB {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassS;

impl ClassS {
    pub fn new() -> Self {
        Self
    }

    pub fn instance() -> &'static Self {
        static INSTANCE: ClassS = ClassS;
        &INSTANCE
    }
}

impl std::fmt::Display for ClassS {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ClassS")
    }
}

impl LScrObject for ClassS {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qlib_test_classes_match_qif_defaults() {
        let mut class_a = ClassA::new();
        assert!(!class_a.prop0());
        assert_eq!(class_a.prop1(), 246);
        assert_eq!(class_a.strprop(), "abcd");
        assert_eq!(class_a.enumprop(), ClassAEnum::Bar);

        class_a.foo(7, "abc", 1.25);
        assert_eq!(class_a.strprop(), "value is 7 and abc");
        assert_eq!(class_a.realprop(), 1.25);
        let mut other = ClassA::new();
        other.set_prop1(10);
        let mut shared = ClassA::new();
        shared.set_prop1(20);
        class_a.foo2(&other, Some(&shared));
        assert_eq!(class_a.prop4(), 30);

        let mut class_b = ClassB::new();
        class_b.bar(2.0);
        assert_eq!(class_b.last_bar(), Some(2.0));
        assert_eq!(ClassS::instance().to_string(), "ClassS");
    }
}
