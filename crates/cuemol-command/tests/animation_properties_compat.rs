use cuemol_anim::AnimMotion;
use cuemol_core::{parse_ldom2_xml, write_ldom2_xml, LDom2Node, Vector4};
use serde_json::Value;

fn motion(kind: &str) -> AnimMotion {
    let mut node = LDom2Node::new();
    node.set_tag_name("motion");
    node.set_type_name(kind);
    AnimMotion::from_ldom2_node(&node).unwrap()
}

#[test]
fn animation_defaults_roundtrip_without_losing_explicit_equal_values() {
    let mut spin = motion("SimpleSpin");
    assert!(spin.is_property_default("axis"));
    assert!(spin.is_property_default("angle"));
    let AnimMotion::Spin(value) = &mut spin else {
        unreachable!()
    };
    value.set_angle(0.);
    value.set_axis(Vector4::new3(2., 0., 0.));
    assert!(!spin.is_property_default("angle"));
    assert!(!spin.is_property_default("axis"));
    let original = spin.to_ldom2_node();
    let xml = write_ldom2_xml(&original.clone().into());
    assert!(xml.contains("angle=\"0\""));
    assert!(!xml.contains("quadric="));
    let restored = AnimMotion::from_ldom2_node(parse_ldom2_xml(&xml).unwrap().top()).unwrap();
    assert_eq!(restored.to_ldom2_node(), original);
    spin.reset_property("angle").unwrap();
    spin.reset_property("axis").unwrap();
    assert!(spin.is_property_default("angle"));
    assert!(spin.is_property_default("axis"));
    assert!(spin.reset_property("name").is_err());
    let xml = write_ldom2_xml(&spin.to_ldom2_node().into());
    assert!(!xml.contains("angle="));
    assert!(!xml.contains("axis="));
    let restored = AnimMotion::from_ldom2_node(parse_ldom2_xml(&xml).unwrap().top()).unwrap();
    assert!(restored.is_property_default("angle"));
    assert!(restored.is_property_default("axis"));
}

#[test]
fn animation_property_metadata_matches_the_current_reference() {
    let Ok(path) = std::env::var("CUEMOL_ANIM_PROPERTY_ORACLE") else {
        eprintln!("Skipped reference animation properties: CUEMOL_ANIM_PROPERTY_ORACLE is unset");
        return;
    };
    let file = std::fs::File::open(path).unwrap();
    let mut cases = 0;
    for item in serde_json::Deserializer::from_reader(file).into_iter::<Value>() {
        let item = item.unwrap();
        let kind = item["type"].as_str().unwrap();
        let mut current = motion(kind);
        let defaults = current
            .property_specs()
            .filter(|s| s.default.is_some())
            .map(|s| s.name)
            .collect::<Vec<_>>();
        if item["phase"] != "initial" {
            let mut node = current.to_ldom2_node();
            for name in &defaults {
                node.find_child_mut(name).unwrap().set_default_flag(false);
            }
            current = AnimMotion::from_ldom2_node(&node).unwrap();
        }
        if item["phase"] == "reset" {
            for name in &defaults {
                current.reset_property(name).unwrap();
            }
        }
        let props = item["properties"].as_array().unwrap();
        assert_eq!(props.len(), current.property_specs().count(), "{kind}");
        let node = current.to_ldom2_node();
        for prop in props {
            let name = prop["name"].as_str().unwrap();
            let spec = current
                .property_spec(name)
                .unwrap_or_else(|| panic!("missing {kind}.{name}"));
            assert_eq!(prop["type"], spec.type_name, "{kind}.{name}");
            assert_eq!(prop["readonly"], spec.read_only, "{kind}.{name}");
            assert_eq!(prop["hasdefault"], spec.default.is_some(), "{kind}.{name}");
            if spec.default.is_some() {
                assert_eq!(
                    prop["isdefault"],
                    current.is_property_default(name),
                    "{kind}.{name} {}",
                    item["phase"]
                );
            }
            if matches!(name, "uid" | "start" | "end" | "absStart" | "absEnd") {
                continue;
            }
            let actual = node.str_attr(name);
            match spec.type_name {
                "real" => assert_eq!(
                    actual.as_str().parse::<f64>().unwrap(),
                    prop["value"].as_f64().unwrap(),
                    "{kind}.{name}"
                ),
                "boolean" => assert_eq!(
                    actual.as_str().parse::<bool>().unwrap(),
                    prop["value"].as_bool().unwrap(),
                    "{kind}.{name}"
                ),
                "object<Vector>" => assert_eq!(
                    Vector4::from_script_string(actual.as_str()).unwrap(),
                    Vector4::from_script_string(prop["value"].as_str().unwrap()).unwrap(),
                    "{kind}.{name}"
                ),
                _ => assert_eq!(
                    actual.as_str(),
                    prop["value"].as_str().unwrap(),
                    "{kind}.{name}"
                ),
            }
        }
        cases += 1;
    }
    assert_eq!(cases, 24);
}
