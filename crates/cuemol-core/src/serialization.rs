use crate::{
    ClassRegistry, CueMolError, LChar, LDom2Node, LPropSupport, LString, LVariant, PropObject,
    PropSpec, Result,
};

pub trait LSerializable {
    fn write_to2(&self, node: &mut LDom2Node) -> Result<()>;
    fn read_from2(&mut self, node: &mut LDom2Node, registry: &ClassRegistry) -> Result<()>;
}

impl<T> LSerializable for T
where
    T: LPropSupport + ?Sized,
{
    fn write_to2(&self, node: &mut LDom2Node) -> Result<()> {
        write_properties_to_dom(self, node)
    }

    fn read_from2(&mut self, node: &mut LDom2Node, registry: &ClassRegistry) -> Result<()> {
        read_properties_from_dom(self, node, registry)
    }
}

pub fn write_properties_to_dom(
    object: &(impl LPropSupport + ?Sized),
    node: &mut LDom2Node,
) -> Result<()> {
    for name in object.prop_names() {
        let Some(spec) = object.get_prop_spec_impl(&name) else {
            continue;
        };
        if spec.no_persist || (spec.read_only && spec.type_name.as_str() == "array") {
            continue;
        }
        let is_default = spec.has_default && object.is_prop_default(&name);
        let Some(value) = object.get_property(&name) else {
            continue;
        };
        if value.is_null() {
            continue;
        }

        let mut child = LDom2Node::new();
        child.set_tag_name(name.to_string());
        setup_node_by_variant(&mut child, &value)?;
        child.set_read_only(spec.read_only);
        child.set_default_flag(is_default);
        node.append_child_node(child);
    }
    Ok(())
}

pub fn read_properties_from_dom(
    object: &mut (impl LPropSupport + ?Sized),
    node: &mut LDom2Node,
    registry: &ClassRegistry,
) -> Result<()> {
    for child in node.children_mut() {
        let name = LString::from(child.tag_name());
        let Some(spec) = object.get_prop_spec_impl(&name) else {
            continue;
        };
        if spec.no_persist {
            continue;
        }

        if spec.read_only {
            let Some(value) = object.get_property(&name) else {
                continue;
            };
            let Ok(prop_object) = value.get_object::<PropObject>() else {
                continue;
            };
            read_properties_from_dom(&mut *prop_object.inner().borrow_mut(), child, registry)?;
            continue;
        }

        let value = match variant_from_node(child, &spec, registry) {
            Ok(value) => value,
            Err(error) => {
                child.append_err_msg(error.to_string());
                continue;
            }
        };
        if !object.set_property_impl(&name, &value) {
            child.append_err_msg(format!("cannot set property {name}"));
            continue;
        }

        if child.is_leaf() && child.child_count() != 0 {
            if let Some(value) = object.get_property(&name) {
                if let Ok(prop_object) = value.get_object::<PropObject>() {
                    read_properties_from_dom(
                        &mut *prop_object.inner().borrow_mut(),
                        child,
                        registry,
                    )?;
                }
            }
        }
        object.set_default_prop_flag(&name, false);
        child.set_consumed(true);
    }
    Ok(())
}

/// Convert one DOM property node using the supplied property specification.
pub fn variant_from_dom_node(
    node: &LDom2Node,
    spec: &PropSpec,
    registry: &ClassRegistry,
) -> Result<LVariant> {
    variant_from_node(&mut node.clone(), spec, registry)
}

fn setup_node_by_variant(node: &mut LDom2Node, value: &LVariant) -> Result<()> {
    match value {
        LVariant::Object(object) | LVariant::SmartPtr(object) => {
            if let Some(prop_object) = object.as_ref().as_any().downcast_ref::<PropObject>() {
                if prop_object.class_name().is_empty() {
                    return Err(CueMolError::invalid_argument(
                        "property object class name is empty",
                    ));
                }
                node.set_type_name(prop_object.class_name().to_string());
                node.set_attr_flag(false);
                return write_properties_to_dom(&*prop_object.inner().borrow(), node);
            }
            node.set_value(object.to_string());
            node.set_attr_flag(true);
            Ok(())
        }
        LVariant::Array(_) | LVariant::List(_) | LVariant::Dict(_) => Err(
            CueMolError::invalid_argument("collection variants are not string-convertible"),
        ),
        _ => {
            node.set_value(value.to_string_value().to_string());
            node.set_attr_flag(true);
            Ok(())
        }
    }
}

fn variant_from_node(
    node: &mut LDom2Node,
    spec: &PropSpec,
    registry: &ClassRegistry,
) -> Result<LVariant> {
    if !node.is_leaf() {
        let class = registry.get_class(node.type_name())?;
        let object = class.create_scr_obj()?;
        let prop_object = object
            .as_ref()
            .as_any()
            .downcast_ref::<PropObject>()
            .cloned()
            .ok_or_else(|| {
                CueMolError::invalid_argument(format!(
                    "class {} is not a property object",
                    node.type_name()
                ))
            })?;
        read_properties_from_dom(&mut *prop_object.inner().borrow_mut(), node, registry)?;
        return Ok(LVariant::Object(Box::new(prop_object)));
    }

    scalar_from_string(node.value(), spec, registry)
}

fn scalar_from_string(value: &str, spec: &PropSpec, registry: &ClassRegistry) -> Result<LVariant> {
    match spec.type_name.as_str() {
        "boolean" => match value.to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" => Ok(true.into()),
            "false" | "off" | "no" => Ok(false.into()),
            _ => Err(CueMolError::invalid_argument(format!(
                "cannot cast string ({value}) to boolean"
            ))),
        },
        "integer" => LChar::to_int(value, 0).map(LVariant::from).ok_or_else(|| {
            CueMolError::invalid_argument(format!("cannot cast {value} to integer"))
        }),
        "real" => LChar::to_double(value)
            .map(LVariant::from)
            .ok_or_else(|| CueMolError::invalid_argument(format!("cannot cast {value} to real"))),
        "string" | "" => Ok(LVariant::String(value.into())),
        "enum" => {
            if spec
                .enum_def
                .as_ref()
                .is_some_and(|definitions| !definitions.contains_key(&LString::from(value)))
            {
                return Err(CueMolError::invalid_argument(format!(
                    "unknown enum value {value}"
                )));
            }
            Ok(LVariant::Enum(value.into()))
        }
        "array" | "list" | "dict" => Err(CueMolError::invalid_argument(format!(
            "cannot deserialize {} from a string",
            spec.type_name
        ))),
        class_name => {
            let class = registry.get_class(class_name)?;
            let object = class.create_scr_obj_from_string(&LString::from(value))?;
            let prop_object = object
                .as_ref()
                .as_any()
                .downcast_ref::<PropObject>()
                .cloned()
                .ok_or_else(|| {
                    CueMolError::invalid_argument(format!(
                        "class {class_name} is not a property object"
                    ))
                })?;
            Ok(LVariant::Object(Box::new(prop_object)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        parse_ldom2_xml, write_ldom2_xml, EnumDef, LClass, LPropSupport, LVarArray,
        MapPropContainer,
    };

    fn child_container() -> MapPropContainer {
        let mut child = MapPropContainer::new();
        child.add_property(
            "count",
            PropSpec::writable("integer").with_default(),
            1.into(),
        );
        child
    }

    fn root_container() -> MapPropContainer {
        let mut enums = EnumDef::new();
        enums.insert("foo".into(), 1);
        enums.insert("bar".into(), 2);
        let mut root = MapPropContainer::new();
        root.add_property(
            "enabled",
            PropSpec::writable("boolean").with_default(),
            true.into(),
        );
        root.add_property(
            "title",
            PropSpec::writable("string").with_default(),
            "default title".into(),
        );
        root.add_property(
            "mode",
            PropSpec::writable("enum").with_enum(enums).with_default(),
            LVariant::Enum("foo".into()),
        );
        root.add_property(
            "transient",
            PropSpec::writable("integer").no_persist(),
            99.into(),
        );
        root.add_property(
            "child",
            PropSpec::read_only("object"),
            LVariant::Object(Box::new(PropObject::new_typed("Child", child_container()))),
        );
        root.add_property(
            "replacement",
            PropSpec::writable("object"),
            LVariant::Object(Box::new(PropObject::new_typed("Child", child_container()))),
        );
        root.add_property(
            "readonly_array",
            PropSpec::read_only("array"),
            LVariant::Array(LVarArray::with_size(2)),
        );
        root
    }

    fn set_child_value(root: &MapPropContainer, property: &str, value: i32) {
        let child = root
            .get_property(&LString::from(property))
            .unwrap()
            .get_object::<PropObject>()
            .unwrap()
            .clone();
        assert!(child.inner().borrow_mut().set_prop_int("count", value));
    }

    fn child_value(root: &MapPropContainer, property: &str) -> i32 {
        let child = root
            .get_property(&LString::from(property))
            .unwrap()
            .get_object::<PropObject>()
            .unwrap()
            .clone();
        let value = child.inner().borrow().get_prop_int("count").unwrap();
        value
    }

    #[test]
    fn property_objects_roundtrip_scalars_enums_and_nested_objects() {
        let mut source = root_container();
        source.set_prop_bool("enabled", false);
        source.set_property(&"mode".into(), &LVariant::Enum("bar".into()));
        set_child_value(&source, "child", 7);
        set_child_value(&source, "replacement", 11);

        let mut root_node = LDom2Node::new();
        root_node.set_tag_name("root");
        source.write_to2(&mut root_node).unwrap();
        let xml = write_ldom2_xml(&crate::LDom2Tree::from(root_node));
        assert!(xml.contains("enabled=\"false\""));
        assert!(xml.contains("mode=\"bar\""));
        assert!(xml.contains("<child count=\"7\""));
        assert!(!xml.contains("<child type=\"Child\""));
        assert!(xml.contains("<replacement type=\"Child\""));
        assert!(!xml.contains("default title"));
        assert!(!xml.contains("transient"));
        assert!(!xml.contains("readonly_array"));

        let mut parsed = parse_ldom2_xml(&xml).unwrap();
        let mut target = root_container();
        let mut registry = ClassRegistry::new();
        registry.register_class(LClass::with_factory::<PropObject, _>("Child", || {
            Ok(PropObject::new_typed("Child", child_container()))
        }));
        target.read_from2(parsed.top_mut(), &registry).unwrap();

        assert_eq!(target.get_prop_bool("enabled"), Some(false));
        assert_eq!(
            target
                .get_property(&"mode".into())
                .unwrap()
                .get_enum_value()
                .unwrap()
                .as_str(),
            "bar"
        );
        assert_eq!(
            target.get_prop_str("title").unwrap().as_str(),
            "default title"
        );
        assert_eq!(target.get_prop_int("transient"), Some(99));
        assert_eq!(child_value(&target, "child"), 7);
        assert_eq!(child_value(&target, "replacement"), 11);
        assert!(!target.is_prop_default(&"enabled".into()));
        assert!(target.is_prop_default(&"title".into()));
    }

    #[test]
    fn deserialization_ignores_unknown_nopersist_and_invalid_enum_nodes() {
        let mut root = root_container();
        let mut node = LDom2Node::new();
        node.set_tag_name("root");
        node.set_str_attr("unknown", "value");
        node.set_str_attr("transient", "123");
        node.set_str_attr("mode", "invalid");
        let registry = ClassRegistry::new();

        root.read_from2(&mut node, &registry).unwrap();
        assert_eq!(root.get_prop_int("transient"), Some(99));
        assert_eq!(
            root.get_property(&"mode".into())
                .unwrap()
                .get_enum_value()
                .unwrap()
                .as_str(),
            "foo"
        );
        assert!(!node.find_child("unknown").unwrap().is_consumed());
        assert!(!node.find_child("transient").unwrap().is_consumed());
        assert!(!node.find_child("mode").unwrap().is_consumed());
        assert!(!node.find_child("mode").unwrap().error_msgs().is_empty());
    }
}
