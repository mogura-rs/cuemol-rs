use cuemol_core::Vector4;
use cuemol_desktop_core::DesktopAppState;
use cuemol_molstr::{read_pdb_str, MolCoord};
use std::hash::Hasher;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source = args.next().ok_or("expected memory or a trajectory path")?;
    let atoms: usize = args.next().ok_or("expected atom count")?.parse()?;
    let iterations: usize = args.next().as_deref().unwrap_or("32").parse()?;
    if atoms == 0 || atoms > i32::MAX as usize || iterations == 0 {
        return Err("atom count and iteration count must be positive".into());
    }
    let template = read_pdb_str(
        "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n",
    )?
    .atoms()[0]
        .clone();
    let mut molecule = MolCoord::new();
    for index in 0..atoms {
        let mut atom = template.clone();
        atom.serial = index as i32 + 1;
        molecule.push_atom(atom);
    }
    let mut app = DesktopAppState::new();
    let scene = app.ensure_active_scene();
    let object = app
        .scene_manager_mut()
        .scene_mut(scene)
        .ok_or("missing scene")?
        .add_molecule("trajectory", molecule);
    let started = Instant::now();
    if source == "memory" {
        app.set_object_trajectory(
            object,
            (0..4)
                .map(|frame| {
                    (0..atoms)
                        .map(|index| Vector4::new3(index as f64 * 0.125, frame as f64, -0.25))
                        .collect()
                })
                .collect(),
        );
    } else {
        app.append_trajectory_block(object, &source, 1)?;
    }
    let load_ms = started.elapsed().as_secs_f64() * 1000.;
    let frames = app.trajectory_len(object);
    let mut elapsed = Duration::ZERO;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for iteration in 0..iterations {
        let index = (iteration * 7) % frames;
        let started = Instant::now();
        app.apply_trajectory_frame(object, index)?;
        elapsed += started.elapsed();
        let molecule = app
            .scene_manager()
            .object(object)
            .ok_or("missing object")?
            .mol();
        for atom in molecule.atoms() {
            for value in [
                atom.position.x,
                atom.position.y,
                atom.position.z,
                atom.position.w,
            ] {
                hash.write_u64(value.to_bits());
            }
        }
    }
    println!(
        "{{\"atoms\":{atoms},\"frames\":{frames},\"iterations\":{iterations},\"load_ms\":{load_ms},\"apply_ms\":{},\"hash\":\"{:016x}\"}}",
        elapsed.as_secs_f64() * 1000.,
        hash.finish()
    );
    Ok(())
}
