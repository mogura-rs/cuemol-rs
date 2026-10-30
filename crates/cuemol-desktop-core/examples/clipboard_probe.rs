use cuemol_desktop_core::{DesktopAppState, GuiPastedNode};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() != 2 {
        return Err("usage: clipboard_probe INPUT_DIR OUTPUT_DIR".into());
    }
    let input = PathBuf::from(&arguments[0]);
    let output = PathBuf::from(&arguments[1]);
    fs::create_dir_all(&output)?;
    let mut state = DesktopAppState::new();
    let GuiPastedNode::Object(object) = state.paste_clipboard_bytes(
        "object",
        "single",
        &fs::read(input.join("object.bin"))?,
        None,
        None,
    )?
    else {
        return Err("object was not pasted".into());
    };
    let GuiPastedNode::Renderers { uids, .. } = state.paste_clipboard_bytes(
        "renderer",
        "single",
        &fs::read(input.join("renderer.bin"))?,
        Some(object),
        Some("object"),
    )?
    else {
        return Err("renderer was not pasted".into());
    };
    let GuiPastedNode::Camera(name) = state.paste_clipboard_bytes(
        "camera",
        "single",
        &fs::read(input.join("camera.bin"))?,
        None,
        None,
    )?
    else {
        return Err("camera was not pasted".into());
    };
    let scene = state.active_scene().unwrap();
    fs::write(
        output.join("object.bin"),
        state.copy_workspace_object(object)?.to_payload()?.bytes,
    )?;
    fs::write(
        output.join("renderer.bin"),
        state
            .copy_workspace_renderer(object, uids[0])?
            .to_payload()?
            .bytes,
    )?;
    fs::write(
        output.join("camera.bin"),
        state
            .copy_workspace_camera(scene, &name)?
            .to_payload()?
            .bytes,
    )?;
    let molecule = state.scene_manager().object(object).unwrap();
    println!(
        "{}",
        serde_json::json!({"atoms":molecule.mol().atom_count(),"name":molecule.name(),"renderers":molecule.renderers().iter().map(|renderer|renderer.name()).collect::<Vec<_>>(),"camera":name})
    );
    Ok(())
}
