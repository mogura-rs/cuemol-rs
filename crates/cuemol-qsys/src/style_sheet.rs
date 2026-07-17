use crate::{StyleManager, INVALID_UID};
use cuemol_core::{
    variant_from_dom_node, ClassRegistry, LDom2Node, LPropSupport, LRegExpr, LString, PropObject,
};

/// Ordered list of named styles applied within one style-manager scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleSheet {
    style_names: Vec<String>,
    scope_id: u64,
}

impl Default for StyleSheet {
    fn default() -> Self {
        Self {
            style_names: Vec::new(),
            scope_id: INVALID_UID,
        }
    }
}

impl StyleSheet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scope_id(&self) -> u64 {
        self.scope_id
    }

    pub fn set_scope_id(&mut self, scope_id: u64) {
        self.scope_id = scope_id;
    }

    pub fn set_style_names(&mut self, names: impl AsRef<str>) {
        self.style_names = names
            .as_ref()
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
    }

    pub fn style_names(&self) -> String {
        self.style_names.join(",")
    }

    pub fn contains(&self, name: &str) -> bool {
        self.style_names.iter().any(|style| style == name)
    }

    /// Append a style at the highest-priority end of the cascade.
    pub fn append(&mut self, name: impl Into<String>) -> bool {
        let name = name.into();
        if name.is_empty() || self.style_names.last() == Some(&name) {
            return false;
        }
        self.style_names.retain(|style| style != &name);
        self.style_names.push(name);
        true
    }

    pub fn remove_by_regex(&mut self, pattern: &str) -> bool {
        let before = self.style_names.len();
        let mut expression = LRegExpr::with_pattern(pattern);
        self.style_names
            .retain(|name| !expression.match_text(name.as_str()).unwrap_or(false));
        before != self.style_names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.style_names.is_empty()
    }

    /// Resolve a dotted property path. Later styles override earlier styles.
    pub fn resolve_style_node<'a>(
        &self,
        manager: &'a StyleManager,
        property_name: &str,
    ) -> Option<&'a LDom2Node> {
        self.style_names.iter().rev().find_map(|style_name| {
            let mut node = manager.style_node(self.scope_id, style_name)?;
            for part in property_name.split('.').filter(|part| !part.is_empty()) {
                node = node.find_child(part)?;
            }
            Some(node)
        })
    }

    pub fn resolve_style_value(
        &self,
        manager: &StyleManager,
        property_name: &str,
    ) -> Option<String> {
        self.resolve_style_node(manager, property_name)
            .map(|node| node.value().to_string())
    }

    /// Apply resolved style values to properties still marked as default.
    ///
    /// Read-only property objects are traversed recursively. Writable object
    /// properties are replaced through the class registry when the style node
    /// names a concrete type.
    pub fn apply_style(
        &self,
        manager: &StyleManager,
        registry: &ClassRegistry,
        target: &mut dyn LPropSupport,
    ) -> usize {
        self.apply_style_helper(manager, registry, "", target)
    }

    fn apply_style_helper(
        &self,
        manager: &StyleManager,
        registry: &ClassRegistry,
        parent_name: &str,
        target: &mut dyn LPropSupport,
    ) -> usize {
        let mut changed = 0;
        for name in target.prop_names() {
            let Some(spec) = target.get_prop_spec_impl(&name) else {
                continue;
            };
            let Some(current) = target.get_property_impl(&name) else {
                continue;
            };
            let nested_name = if parent_name.is_empty() {
                name.to_string()
            } else {
                format!("{parent_name}.{name}")
            };

            if let Ok(child) = current.get_object::<PropObject>() {
                if spec.read_only {
                    changed += self.apply_style_helper(
                        manager,
                        registry,
                        &nested_name,
                        &mut *child.inner().borrow_mut(),
                    );
                    continue;
                }
            }

            if spec.read_only || !target.is_prop_default(&name) {
                continue;
            }
            let Some(node) = self.resolve_style_node(manager, &nested_name) else {
                changed += usize::from(target.reset_property_impl(&name));
                continue;
            };
            let Ok(value) = variant_from_dom_node(node, &spec, registry) else {
                continue;
            };
            changed += usize::from(target.set_property_impl(&name, &value));
        }
        changed
    }
}

#[derive(Debug, Clone, Default)]
pub struct StyleEvent;

pub trait StyleSupports {
    fn style_sheet(&self) -> &StyleSheet;
    fn style_changed(&mut self, event: &StyleEvent);
    fn style_context_id(&self) -> u64;
}

/// Reset a property to its stylesheet value while keeping its default flag.
pub fn reset_property_to_style(
    sheet: &StyleSheet,
    manager: &StyleManager,
    registry: &ClassRegistry,
    target: &mut dyn LPropSupport,
    property_name: &LString,
) -> bool {
    if target.is_prop_default(property_name) {
        return true;
    }
    let Some(spec) = target.get_prop_spec_impl(property_name) else {
        return false;
    };
    let Some(node) = sheet.resolve_style_node(manager, property_name.as_str()) else {
        return false;
    };
    let Ok(value) = variant_from_dom_node(node, &spec, registry) else {
        return false;
    };
    target.set_default_prop_flag(property_name, true);
    if target.set_property_impl(property_name, &value) {
        true
    } else {
        target.set_default_prop_flag(property_name, false);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuemol_core::{LPropSupport, LVariant, MapPropContainer, PropSpec};

    fn style_manager() -> StyleManager {
        let mut manager = StyleManager::new();
        let xml = r#"
            <styles>
              <style id="base" count="3" label="styled">
                <child count="5"/>
              </style>
              <style id="override" count="7"/>
            </styles>
        "#;
        assert_ne!(
            manager.load_style_set_from_xml(11, "test", xml, false),
            INVALID_UID
        );
        manager
    }

    #[test]
    fn style_sheet_orders_appends_and_removes_names() {
        let mut sheet = StyleSheet::new();
        sheet.set_style_names(" base, override ");
        assert_eq!(sheet.style_names(), "base,override");
        assert!(!sheet.append("override"));
        assert!(sheet.append("base"));
        assert_eq!(sheet.style_names(), "override,base");
        assert!(sheet.remove_by_regex("^over"));
        assert_eq!(sheet.style_names(), "base");
    }

    #[test]
    fn style_sheet_applies_last_style_only_to_default_properties() {
        let manager = style_manager();
        let registry = ClassRegistry::new();
        let mut sheet = StyleSheet::new();
        sheet.set_scope_id(11);
        sheet.set_style_names("base,override");

        let mut target = MapPropContainer::new();
        target.add_property(
            "count",
            PropSpec::writable("integer").with_default(),
            1.into(),
        );
        target.add_property(
            "label",
            PropSpec::writable("string").with_default(),
            LVariant::from("initial"),
        );
        target.add_property(
            "missing",
            PropSpec::writable("integer").with_default(),
            9.into(),
        );
        target.set_property_impl(&"missing".into(), &20.into());
        target.set_property(&"label".into(), &LVariant::from("manual"));

        assert_eq!(sheet.apply_style(&manager, &registry, &mut target), 2);
        assert_eq!(target.get_prop_int("count"), Some(7));
        assert_eq!(target.get_prop_int("missing"), Some(9));
        assert_eq!(
            target.get_prop_str("label").as_ref().map(LString::as_str),
            Some("manual")
        );
    }

    #[test]
    fn style_sheet_recurses_into_readonly_property_objects() {
        let manager = style_manager();
        let registry = ClassRegistry::new();
        let mut sheet = StyleSheet::new();
        sheet.set_scope_id(11);
        sheet.set_style_names("base");

        let mut child = MapPropContainer::new();
        child.add_property(
            "count",
            PropSpec::writable("integer").with_default(),
            1.into(),
        );
        let child = PropObject::new(child);
        let child_handle = child.inner();
        let mut target = MapPropContainer::new();
        target.add_property(
            "child",
            PropSpec::read_only("object").with_default(),
            LVariant::Object(Box::new(child)),
        );

        assert_eq!(sheet.apply_style(&manager, &registry, &mut target), 1);
        assert_eq!(child_handle.borrow().get_prop_int("count"), Some(5));
    }

    #[test]
    fn style_reset_uses_resolved_value_and_restores_default_flag() {
        let manager = style_manager();
        let registry = ClassRegistry::new();
        let mut sheet = StyleSheet::new();
        sheet.set_scope_id(11);
        sheet.set_style_names("base");
        let mut target = MapPropContainer::new();
        target.add_property(
            "count",
            PropSpec::writable("integer").with_default(),
            1.into(),
        );
        target.set_property(&"count".into(), &9.into());

        assert!(reset_property_to_style(
            &sheet,
            &manager,
            &registry,
            &mut target,
            &"count".into()
        ));
        assert_eq!(target.get_prop_int("count"), Some(3));
        assert!(target.is_prop_default(&"count".into()));
    }
}
