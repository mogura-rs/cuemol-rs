use cuemol_command::selection;
use cuemol_desktop_core::DesktopAppState;
use serde_json::json;
use std::{env, path::PathBuf, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err(
            "usage: selection_bench MOLECULE ITERATIONS EXPRESSION [TARGET_MOLECULE]".into(),
        );
    }
    let path = PathBuf::from(&args[0]);
    let iterations: usize = args[1].parse()?;
    if iterations == 0 {
        return Err("iterations must be positive".into());
    }
    let expression = &args[2];
    let mut state = DesktopAppState::new();
    state.open_bytes(
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid filename")?,
        &std::fs::read(&path)?,
    )?;
    let object = state.active_object_summaries()[0].uid;
    if let Some(target) = args.get(3) {
        let scene = state.active_scene().ok_or("missing scene")?;
        let scene = state.scene_manager_mut().scene_mut(scene).unwrap();
        scene.object_mut(object).unwrap().set_name("Probe");
        scene.add_molecule("Target", cuemol_molstr::read_pdb_file(target)?);
    }
    let query = || -> Result<usize, Box<dyn std::error::Error>> {
        let result = selection::evaluate(&state, Some(object), expression);
        result.count.ok_or_else(|| {
            result
                .error
                .unwrap_or_else(|| "missing count".into())
                .into()
        })
    };
    let start = Instant::now();
    let count = query()?;
    let cold_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    for _ in 0..iterations {
        if std::hint::black_box(query()?) != count {
            return Err("selection count changed between repeated queries".into());
        }
    }
    let query_ms = start.elapsed().as_secs_f64() * 1000. / iterations as f64;
    let molecule = state
        .scene_manager()
        .object(object)
        .ok_or("missing molecule")?
        .mol();
    let indices = molecule.select_indices(&state.parse_object_selector(object, expression)?);
    if indices.len() != count {
        return Err("selection mask differs from count".into());
    }
    let digest = indices
        .iter()
        .flat_map(|index| (*index as u64).to_le_bytes())
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    println!(
        "SELECTION_RESULT {}",
        json!({
            "expression":expression,"iterations":iterations,"atoms":molecule.atoms().len(),
            "count":count,"digest":format!("{digest:016x}"),"cold_ms":cold_ms,"query_ms":query_ms,
        })
    );
    Ok(())
}
