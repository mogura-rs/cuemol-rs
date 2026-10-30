use cuemol_command::{prepare_render, render_settings};
use cuemol_desktop_core::DesktopAppState;
use serde_json::json;
use std::{env, path::PathBuf, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() != 3 {
        return Err("usage: render_bench SCENE OUTPUT_DIR REPETITIONS".into());
    }
    let input = PathBuf::from(&arguments[0]);
    let output = PathBuf::from(&arguments[1]);
    let repetitions: usize = arguments[2]
        .to_str()
        .ok_or("invalid repetitions")?
        .parse()?;
    if repetitions == 0 {
        return Err("repetitions must be positive".into());
    }
    std::fs::create_dir_all(&output)?;
    let started = Instant::now();
    let mut state = DesktopAppState::new();
    state.open_scene_file(input, None)?;
    let load_ms = started.elapsed().as_secs_f64() * 1000.;
    let objects: Vec<_> = state.object_summaries(state.active_scene().ok_or("scene missing")?)
        .into_iter().map(|object| json!({"name":object.name,"atoms":object.atom_count,"bonds":object.bond_count})).collect();
    let camera = state
        .active_scene_saved_camera()
        .ok_or("saved camera missing")?;
    let settings = render_settings(&state)?;
    let mut samples = Vec::new();
    for repetition in 0..repetitions {
        let path = output.join(format!("render-{repetition}.png"));
        let started = Instant::now();
        let prepared = prepare_render(&state, &camera, "render_image", &json!({"fileName":path}))?;
        let prepare_ms = started.elapsed().as_secs_f64() * 1000.;
        let started = Instant::now();
        let result = prepared.run()?;
        samples.push(json!({"prepare_ms":prepare_ms,"render_ms":started.elapsed().as_secs_f64()*1000.,"result":result}));
    }
    println!(
        "RENDER_RESULT {}",
        json!({"load_ms":load_ms,"objects":objects,"settings":settings,"samples":samples})
    );
    Ok(())
}
