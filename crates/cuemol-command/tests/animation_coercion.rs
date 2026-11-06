use cuemol_command::{animation_element_detail, call_tool};
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};

fn fixture(kind: &str) -> (DesktopAppState, Camera, u64) {
    let mut state = DesktopAppState::new();
    state.new_scene("animation", true);
    let mut camera = Camera::new();
    let added = call_tool(
        &mut state,
        &mut camera,
        "add_anim",
        &json!({"type":kind,"name":"element"}),
    )
    .unwrap();
    (state, camera, added["elements"][0]["uid"].as_u64().unwrap())
}

fn write(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    tool: &str,
    prop: &str,
    value: &str,
) -> cuemol_core::Result<Value> {
    call_tool(
        state,
        camera,
        tool,
        &json!({"element":"1","prop":prop,"value":value}),
    )
}

#[test]
fn structured_animation_properties_parse_boolean_aliases_and_reject_invalid_values() {
    let (mut state, mut camera, uid) = fixture("show");
    for (text, expected) in [
        (" ON ", true),
        ("yes", true),
        ("1", true),
        ("TRUE", true),
        ("OFF", false),
        (" no ", false),
        ("0", false),
        (" false ", false),
    ] {
        write(&mut state, &mut camera, "set_anim_prop", "hide", text).unwrap();
        assert_eq!(
            animation_element_detail(&state, uid).unwrap()["properties"]["hide"],
            expected.to_string(),
            "{text}"
        );
    }
    for text in ["", "maybe", "2", "1.0"] {
        let before = state.clone();
        assert!(
            write(&mut state, &mut camera, "set_anim_prop", "hide", text).is_err(),
            "{text}"
        );
        assert!(before.same_undo_state(&state));
    }
}

#[test]
fn structured_animation_numbers_and_text_follow_command_conversion() {
    let (mut state, mut camera, uid) = fixture("spin");
    for (text, expected) in [
        ("0x80", 128.),
        ("0B11", 3.),
        ("0o10", 8.),
        (".5", 0.5),
        ("1.e2", 100.),
        ("\u{feff} 2 \u{feff}", 2.),
    ] {
        write(&mut state, &mut camera, "set_anim_prop", "angle", text).unwrap();
        let actual = animation_element_detail(&state, uid).unwrap()["properties"]["angle"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert_eq!(actual, expected, "{text}");
    }
    for text in ["", "+0x10", "1_000", "1e", "∞"] {
        let before = state.clone();
        assert!(
            write(&mut state, &mut camera, "set_anim_prop", "angle", text).is_err(),
            "{text}"
        );
        assert!(before.same_undo_state(&state));
    }
    let (mut state, mut camera, uid) = fixture("show");
    write(
        &mut state,
        &mut camera,
        "set_anim_prop",
        "rend",
        "  named renderer  ",
    )
    .unwrap();
    assert_eq!(
        animation_element_detail(&state, uid).unwrap()["properties"]["rend"],
        "named renderer"
    );
    write(
        &mut state,
        &mut camera,
        "set_anim_generic_prop",
        "rend",
        "  named renderer  ",
    )
    .unwrap();
    assert_eq!(
        animation_element_detail(&state, uid).unwrap()["properties"]["rend"],
        "named renderer"
    );
}

#[test]
fn generic_animation_properties_preserve_native_types() {
    let (mut state, mut camera, uid) = fixture("show");
    for (prop, text, expected) in [
        ("hide", "ON", "true"),
        ("hide", " false ", "false"),
        ("tgt_alpha", "0x1", "1"),
    ] {
        write(&mut state, &mut camera, "set_anim_generic_prop", prop, text).unwrap();
        assert_eq!(
            animation_element_detail(&state, uid).unwrap()["properties"][prop],
            expected
        );
    }
    for (prop, text) in [
        ("hide", "1"),
        ("hide", " yes "),
        ("hide", "maybe"),
        ("rend", "123"),
        ("rend", "false"),
        ("tgt_alpha", "true"),
        ("tgt_alpha", ""),
    ] {
        let before = state.clone();
        assert!(
            write(&mut state, &mut camera, "set_anim_generic_prop", prop, text).is_err(),
            "{prop} {text:?}"
        );
        assert!(before.same_undo_state(&state));
    }
}

#[test]
fn spin_axis_near_zero_writes_keep_the_previous_direction() {
    let (mut state, mut camera, uid) = fixture("spin");
    write(&mut state, &mut camera, "set_anim_prop", "axis", "1 0 0").unwrap();
    for text in ["0 0 0", "0 0.00001 0"] {
        write(&mut state, &mut camera, "set_anim_prop", "axis", text).unwrap();
        assert_eq!(
            animation_element_detail(&state, uid).unwrap()["properties"]["axis"],
            "(1,0,0)"
        );
    }
    write(
        &mut state,
        &mut camera,
        "set_anim_prop",
        "axis",
        "0 0.0001 0",
    )
    .unwrap();
    assert_eq!(
        animation_element_detail(&state, uid).unwrap()["properties"]["axis"],
        "(0,1,0)"
    );
    write(&mut state, &mut camera, "set_anim_prop", "axis", "0x2 0 0").unwrap();
    assert_eq!(
        animation_element_detail(&state, uid).unwrap()["properties"]["axis"],
        "(1,0,0)"
    );
}

#[test]
fn animation_command_values_match_current_reference_conversions() {
    let (Ok(cases), Ok(oracle)) = (
        std::env::var("CUEMOL_ANIM_VALUE_CASES"),
        std::env::var("CUEMOL_ANIM_VALUE_ORACLE"),
    ) else {
        eprintln!("Skipped reference animation values: CUEMOL_ANIM_VALUE_CASES/ORACLE are unset");
        return;
    };
    let cases: Vec<Value> = serde_json::from_reader(std::fs::File::open(cases).unwrap()).unwrap();
    let mut failures = Vec::new();
    let mut count = 0;
    for result in serde_json::Deserializer::from_reader(std::fs::File::open(oracle).unwrap())
        .into_iter::<Value>()
    {
        let result = result.unwrap();
        let case = &cases[result["index"].as_u64().unwrap() as usize];
        let (mut state, mut camera, uid) = fixture(case["kind"].as_str().unwrap());
        let prop = case["prop"].as_str().unwrap();
        if prop == "axis" {
            write(&mut state, &mut camera, "set_anim_prop", "axis", "1 0 0").unwrap();
        }
        let before = state.clone();
        let outcome = write(
            &mut state,
            &mut camera,
            case["tool"].as_str().unwrap(),
            prop,
            case["input"].as_str().unwrap(),
        );
        if outcome.is_ok() != result["success"].as_bool().unwrap() {
            failures.push(format!(
                "{case}: expected {}, got {outcome:?}",
                result["success"]
            ));
        } else if outcome.is_err() {
            assert!(before.same_undo_state(&state), "{case}");
        } else {
            let expected = result["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["name"] == prop)
                .unwrap();
            let detail = animation_element_detail(&state, uid).unwrap();
            let actual = detail["properties"][prop].as_str().unwrap();
            let matches = match expected["type"].as_str().unwrap() {
                "boolean" => {
                    actual.parse::<bool>().unwrap() == expected["value"].as_bool().unwrap()
                }
                "real" => actual.parse::<f64>().unwrap() == expected["value"].as_f64().unwrap(),
                "object<Vector>" => {
                    let actual = cuemol_core::Vector4::from_script_string(actual).unwrap();
                    let expected = cuemol_core::Vector4::from_script_string(
                        expected["value"].as_str().unwrap(),
                    )
                    .unwrap();
                    (actual - expected).length() < 1e-5
                }
                _ => expected["value"] == actual,
            };
            if !matches {
                failures.push(format!(
                    "{case}: expected {}, got {actual}",
                    expected["value"]
                ));
            }
        }
        count += 1;
    }
    assert_eq!(count, cases.len());
    assert!(count > 80);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
