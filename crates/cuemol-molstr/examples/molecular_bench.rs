use cuemol_molstr::{read_pdb_str, write_qdf_mol_bytes, MolCoord};
use std::{collections::hash_map::DefaultHasher, hash::Hasher, time::Instant};

fn hash(molecule: &MolCoord) -> String {
    let mut hash = DefaultHasher::new();
    hash.write(&write_qdf_mol_bytes(molecule));
    format!("{:016x}", hash.finish())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !(2..=3).contains(&arguments.len()) {
        return Err("usage: molecular_bench INPUT_PDB REPETITIONS [OUTPUT_QDF]".into());
    }
    let repetitions: usize = arguments[1]
        .to_str()
        .ok_or("invalid repetitions")?
        .parse()?;
    if repetitions == 0 {
        return Err("repetitions must be positive".into());
    }
    let input = std::fs::read_to_string(&arguments[0])?;
    let mut expected = None;
    for iteration in 0..repetitions {
        let start = Instant::now();
        let mut molecule = read_pdb_str(&input)?;
        let read_ms = start.elapsed().as_secs_f64() * 1000.;
        let bytes = write_qdf_mol_bytes(&molecule);
        if let Some(expected) = &expected {
            assert_eq!(&bytes, expected, "repeated reads changed the molecule");
        } else {
            if let Some(output) = arguments.get(2) {
                std::fs::write(output, &bytes)?;
            }
            expected = Some(bytes);
        }
        let read_hash = hash(&molecule);
        let start = Instant::now();
        molecule.apply_topology();
        let topology_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        molecule.calc_prot_2ndry(-500., false);
        let secondary_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        molecule.calc_base_pair(3.7, 30.);
        let basepair_ms = start.elapsed().as_secs_f64() * 1000.;
        let residues = molecule.residue_summaries();
        let start = Instant::now();
        for residue in &residues {
            molecule.set_residue_prop_str(
                &residue.key.chain_id,
                &residue.key.script_index(),
                "benchmark",
                "value",
            )?;
        }
        let properties_ms = start.elapsed().as_secs_f64() * 1000.;
        println!(
            "{{\"iteration\":{iteration},\"atoms\":{},\"residues\":{},\"bonds\":{},\"read_ms\":{read_ms},\"topology_ms\":{topology_ms},\"secondary_ms\":{secondary_ms},\"basepair_ms\":{basepair_ms},\"properties_ms\":{properties_ms},\"read_hash\":\"{read_hash}\",\"final_hash\":\"{}\"}}",
            molecule.atom_count(), residues.len(), molecule.bond_count(), hash(&molecule)
        );
    }
    Ok(())
}
