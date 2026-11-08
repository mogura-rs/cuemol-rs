use cuemol_molstr::{read_pdb_file, write_qdf_mol_bytes};
use std::{collections::hash_map::DefaultHasher, hash::Hasher, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        return Err("usage: molecular_edit_bench INPUT_PDB SNAPSHOTS".into());
    }
    let repetitions: usize = arguments[1].to_str().ok_or("invalid count")?.parse()?;
    if repetitions == 0 {
        return Err("snapshot count must be positive".into());
    }
    let mut molecule = read_pdb_file(&arguments[0])?;
    molecule.calc_base_pair(3.7, 30.);
    let expected = write_qdf_mol_bytes(&molecule);
    let mut snapshots = Vec::with_capacity(repetitions);
    let start = Instant::now();
    for _ in 0..repetitions {
        snapshots.push(molecule.clone());
        molecule.calc_base_pair(3.7, 30.);
        assert!(!molecule.remove_residue_prop_str("missing", "1", "absent")?);
    }
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
    for snapshot in snapshots.iter().chain([&molecule]) {
        assert_eq!(write_qdf_mol_bytes(snapshot), expected);
    }
    let mut hash = DefaultHasher::new();
    hash.write(&expected);
    println!(
        "{{\"atoms\":{},\"snapshots\":{repetitions},\"edit_ms\":{elapsed_ms},\"hash\":\"{:016x}\"}}",
        molecule.atom_count(), hash.finish()
    );
    Ok(())
}
