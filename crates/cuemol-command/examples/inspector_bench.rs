use cuemol_command::call_tool;
use cuemol_core::{write_ldom2_xml, LDom2Node, LDom2Tree};
use cuemol_desktop_core::DesktopAppState;
use cuemol_molstr::LoadedStyle;
use cuemol_qsys::Camera;
use serde_json::json;
use std::{env, path::PathBuf, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: inspector_bench MOLECULE ITERATIONS STYLE_COLORS".into());
    }
    let iterations: usize = args[1].to_str().ok_or("invalid iterations")?.parse()?;
    let colors: usize = args[2].to_str().ok_or("invalid color count")?.parse()?;
    if iterations == 0 {
        return Err("iterations must be positive".into());
    }
    let path = PathBuf::from(&args[0]);
    let mut state = DesktopAppState::new();
    state.open_bytes(
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid filename")?,
        &std::fs::read(&path)?,
    )?;
    let object = state.active_object_summaries()[0].uid;
    let sid = state.active_scene().ok_or("scene missing")?;
    let mut styles = LDom2Node::new();
    styles.set_tag_name("styles");
    styles.set_str_attr("id", "benchmark");
    for index in 0..colors {
        let mut color = LDom2Node::new();
        color.set_tag_name("color");
        color.set_str_attr("id", format!("bench_{index}"));
        color.set_value("#4098b7");
        styles.append_child_node(color);
    }
    let data = write_ldom2_xml(&LDom2Tree::from(styles.clone()));
    state
        .scene_manager_mut()
        .scene_mut(sid)
        .unwrap()
        .add_loaded_style(LoadedStyle { node: styles, data });
    let renderer = state
        .scene_manager_mut()
        .object_mut(object)
        .unwrap()
        .add_renderer("ribbon", "bench");
    for key in ["defaultcolor", "egcolor", "helix_backcol", "sheet_sidecol"] {
        state.set_renderer_property(
            object,
            renderer,
            key,
            if colors == 0 { "#4098b7" } else { "bench_0" },
        )?;
    }
    let request = json!({"nodeType":"renderer","nodeId":renderer.get()});
    let mut camera = Camera::new();
    let start = Instant::now();
    let first = call_tool(&mut state, &mut camera, "list_node_props", &request)?;
    let cold_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    let mut last = first.clone();
    for _ in 0..iterations {
        last = std::hint::black_box(call_tool(
            &mut state,
            &mut camera,
            "list_node_props",
            &request,
        )?);
    }
    let query_ms = start.elapsed().as_secs_f64() * 1000. / iterations as f64;
    if last != first {
        return Err("read-only queries changed their output".into());
    }
    let rows = first["properties"].as_array().ok_or("properties missing")?;
    for key in ["defaultcolor", "egcolor", "helix_backcol", "sheet_sidecol"] {
        let row = rows
            .iter()
            .find(|row| row["key"] == key)
            .ok_or("color missing")?;
        if row["rgba"] != json!([64, 152, 183, 255]) {
            return Err(format!("incorrect resolved color for {key}: {row}").into());
        }
    }
    let canonical = json!({"properties": rows.iter().filter(|row|!matches!(row["key"].as_str(),Some("uid"|"scene_uid"))).collect::<Vec<_>>(),"sections":first["sections"]});
    let bytes = serde_json::to_vec(&canonical)?;
    let digest = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    println!(
        "INSPECTOR_RESULT {}",
        json!({"iterations":iterations,"style_colors":colors,"properties":rows.len(),"response_bytes":bytes.len(),"cold_ms":cold_ms,"query_ms":query_ms,"digest":format!("{digest:016x}")})
    );
    Ok(())
}
