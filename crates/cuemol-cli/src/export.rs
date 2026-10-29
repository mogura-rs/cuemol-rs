use cuemol_command::call_tool;
use cuemol_desktop_core::{DesktopAppState, GuiPovSceneOptions};
use cuemol_molvis::scene_style_manager;
use cuemol_qsys::Camera;
use cuemol_render::{
    tessellate_snapshot, LuxCoreSceneExporter, LuxRendSceneExporter, MqoSceneExporter,
    StlSceneExporter, WbpSceneExporter,
};
use serde_json::json;
use std::path::Path;

pub fn run(input: &str, output: &str) -> Result<(), String> {
    let (mut state, mut camera) = crate::runtime::state()?;
    call_tool(
        &mut state,
        &mut camera,
        "load_file",
        &json!({"path": input}),
    )
    .map_err(|error| error.to_string())?;
    write_scene(&mut state, &mut camera, output)
}

fn write_scene(
    state: &mut DesktopAppState,
    camera: &mut Camera,
    output: &str,
) -> Result<(), String> {
    let extension = Path::new(output)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "png" {
        call_tool(state, camera, "render_image", &json!({"fileName": output}))
            .map_err(|error| error.to_string())?;
        eprintln!("Rendered scene to {output}");
        return Ok(());
    }
    if extension == "pov" {
        state
            .export_active_scene_pov_with_camera(output, "", camera, GuiPovSceneOptions::default())
            .map_err(|error| error.to_string())?;
        eprintln!("Exported scene to {output} (+ INC companion)");
        return Ok(());
    }
    let snapshot = state
        .active_render_snapshot("")
        .map_err(|error| error.to_string())?;
    let scene_uid = state.active_scene().ok_or("no active scene")?;
    let scene = state
        .scene_manager()
        .scene(scene_uid)
        .ok_or("scene not found")?;
    let name = if scene.name().is_empty() {
        "cuemol"
    } else {
        scene.name()
    };
    let background = state
        .active_background_color()
        .map_err(|error| error.to_string())?;
    let background = [background[0], background[1], background[2]];
    let result = match extension.as_str() {
        "cfg" => {
            let mut exporter = LuxCoreSceneExporter::new();
            exporter.handler_mut().set_path(output);
            exporter.write_snapshot_files_with_camera(name, &snapshot, camera, 640, 480)
        }
        "wbp" => {
            let mut exporter = WbpSceneExporter::new();
            exporter.handler_mut().set_path(output);
            exporter.write_snapshot_files_with_camera(name, &snapshot, camera, background)
        }
        "lxs" => {
            let styles = scene_style_manager(scene);
            let mut exporter = LuxRendSceneExporter::new();
            exporter.handler_mut().set_path(output);
            exporter.set_preamble(styles.get_config("lux", "preamble", scene_uid.get()));
            exporter.set_world_preamble(styles.get_config("lux", "worldpreamble", scene_uid.get()));
            let materials = std::iter::once("default")
                .chain(
                    snapshot
                        .transparency_groups
                        .iter()
                        .map(|group| group.material.as_str()),
                )
                .collect::<std::collections::BTreeSet<_>>();
            for material in materials {
                let definition = styles.get_material_string(material, "lux", scene_uid.get());
                if !definition.is_empty() {
                    exporter.set_material_definition(material, definition);
                }
            }
            exporter.write_snapshot_file_with_camera(name, &snapshot, camera, background)
        }
        "stl" | "obj" | "mqo" => {
            let bytes = match extension.as_str() {
                "stl" => StlSceneExporter::new()
                    .write_binary_stl_bytes_with_camera(&tessellate_snapshot(&snapshot), camera),
                "obj" => tessellate_snapshot(&snapshot)
                    .to_obj_string(name)
                    .into_bytes(),
                _ => MqoSceneExporter::new()
                    .write_snapshot_string_with_camera(name, &snapshot, camera)
                    .into_bytes(),
            };
            std::fs::write(output, bytes).map_err(Into::into)
        }
        _ => {
            return Err(format!(
                "unsupported output format (use .stl/.obj/.pov/.png/.mqo/.lxs/.cfg/.wbp): {output}"
            ))
        }
    };
    result.map_err(|error| error.to_string())?;
    eprintln!("Exported scene to {output}");
    Ok(())
}
