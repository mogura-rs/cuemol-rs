use cuemol_core::Vector4;
use cuemol_desktop_core::DesktopAppState;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().unwrap_or_else(|| "100000".into());
    let fixture = input.parse::<usize>().is_err().then_some(input.clone());
    let mut atoms: usize = input.parse().unwrap_or(1);
    let frames: usize = args.next().as_deref().unwrap_or("60").parse()?;
    if atoms == 0 || frames == 0 || atoms > i32::MAX as usize {
        return Err("positive atom and frame counts are required".into());
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
        atom.res_id = (index / 4) as i32 + 1;
        atom.name = format!("C{}", index % 4);
        atom.position = Vector4::new3(index as f64, 0., 0.);
        molecule.push_atom(atom);
    }
    if let Some(path) = &fixture {
        molecule = cuemol_molstr::read_pdb_file(path)?;
        atoms = molecule.atom_count();
    }
    let mut app = DesktopAppState::new();
    let scene = app.ensure_active_scene();
    app.scene_manager_mut()
        .scene_mut(scene)
        .unwrap()
        .add_molecule("molecule", molecule);
    let first_started = Instant::now();
    let first = app.active_object_summaries();
    let first_ms = first_started.elapsed().as_secs_f64() * 1000.;
    let residues: usize = first.iter().map(|object| object.residue_count).sum();
    assert_eq!(
        first.iter().map(|object| object.atom_count).sum::<usize>(),
        atoms
    );
    if fixture.is_none() {
        assert_eq!(residues, atoms.div_ceil(4));
    }
    let started = Instant::now();
    for _ in 0..frames {
        std::hint::black_box(app.active_object_summaries());
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
    assert_eq!(first, app.active_object_summaries());
    println!(
        "{}",
        serde_json::json!({"atoms":atoms,"residues":residues,"frames":frames,"first_ms":first_ms,"elapsed_ms":elapsed_ms,"ms_per_frame":elapsed_ms / frames as f64})
    );
    Ok(())
}
