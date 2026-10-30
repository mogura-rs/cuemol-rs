use cuemol_core::Vector4;
use cuemol_desktop_core::{DesktopAppState, GuiSequenceCache};
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
    let cached = args.next().as_deref() == Some("cached");
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
    let mut cache = GuiSequenceCache::default();
    let started = Instant::now();
    let first = app.active_sequence_view("resid 1:10");
    let first_ms = started.elapsed().as_secs_f64() * 1000.;
    let residues: usize = first.iter().map(|row| row.residues.len()).sum();
    let selected = first
        .iter()
        .flat_map(|row| &row.residues)
        .filter(|residue| residue.selected)
        .count();
    let digest = checksum(&first);
    if fixture.is_none() {
        assert_eq!(first.len(), 1);
        assert_eq!(residues, atoms.div_ceil(4));
        assert_eq!(selected, 10.min(residues));
    }
    drop(first);
    if cached {
        std::hint::black_box(cache.view(&app, "resid 1:10", 0));
    }
    let started = Instant::now();
    for _ in 0..frames {
        if cached {
            std::hint::black_box(cache.view(&app, "resid 1:10", 0));
        } else {
            std::hint::black_box(app.active_sequence_view("resid 1:10"));
        }
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
    if cached {
        assert_eq!(checksum(cache.rows()), digest);
    } else {
        assert_eq!(checksum(&app.active_sequence_view("resid 1:10")), digest);
    }
    println!("{{\"cached\":{cached},\"atoms\":{atoms},\"residues\":{residues},\"selected\":{selected},\"digest\":\"{digest:016x}\",\"frames\":{frames},\"first_ms\":{first_ms:.6},\"elapsed_ms\":{elapsed_ms:.6},\"ms_per_frame\":{:.6}}}", elapsed_ms / frames as f64);
    Ok(())
}

fn checksum(rows: &[cuemol_desktop_core::GuiSeqChain]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for row in rows {
        for residue in &row.residues {
            for field in [
                row.chain_id.as_str(),
                &residue.residue_index,
                &residue.res_name,
                if residue.letter == "X" {
                    "*"
                } else {
                    &residue.letter
                },
                if residue.selected { "1" } else { "0" },
            ] {
                for byte in field.as_bytes().iter().copied().chain(std::iter::once(0)) {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                }
            }
        }
    }
    hash
}
