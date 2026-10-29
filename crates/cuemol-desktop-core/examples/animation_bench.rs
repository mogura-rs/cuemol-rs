use cuemol_anim::{AnimMgr, AnimMotion, SimpleSpin};
use cuemol_core::{Vector4, NANOS_PER_SECOND};
use cuemol_desktop_core::DesktopAppState;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let atoms: usize = args.next().as_deref().unwrap_or("100000").parse()?;
    let frames: u32 = args.next().as_deref().unwrap_or("60").parse()?;
    if atoms == 0 || frames == 0 {
        return Err("atom and frame counts must be positive".into());
    }
    let template = cuemol_molstr::read_pdb_str(
        "ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n",
    )?
    .atoms()[0]
        .clone();
    let mut molecule = cuemol_molstr::MolCoord::new();
    for index in 0..atoms {
        let mut atom = template.clone();
        atom.serial = index as i32 + 1;
        atom.res_id = index as i32 + 1;
        atom.position = Vector4::new3(
            (index % 100) as f64,
            (index / 100 % 100) as f64,
            (index / 10000) as f64,
        );
        molecule.push_atom(atom);
    }
    let mut app = DesktopAppState::new();
    let created = app.new_scene("benchmark", true);
    let scene = created.result_scene;
    let view = created.result_view.unwrap();
    let object = app
        .scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("molecule", molecule);
    app.new_renderer(scene, object, "cpk", "atoms")?;
    let mut manager = AnimMgr::new();
    manager.set_loop(true);
    let mut spin = SimpleSpin::new();
    spin.set_angle(180.);
    spin.base_mut().set_rel_end(NANOS_PER_SECOND);
    manager.append_motion(AnimMotion::Spin(spin));
    app.set_scene_animation(scene, Some(&manager))?;
    app.play_scene_animation(scene, view)?;
    for _ in 0..2 {
        app.advance_scene_animation_by(scene, Duration::from_millis(16))?;
    }
    let started = Instant::now();
    for _ in 0..frames {
        std::hint::black_box(app.advance_scene_animation_by(scene, Duration::from_millis(16))?);
    }
    let elapsed = started.elapsed();
    println!(
        "atoms={atoms} frames={frames} elapsed_ms={:.3} ms_per_frame={:.6}",
        elapsed.as_secs_f64() * 1000.,
        elapsed.as_secs_f64() * 1000. / f64::from(frames)
    );
    let camera = app.active_view_camera().unwrap();
    println!("final_rotation={:?}", camera.rotation());
    Ok(())
}
