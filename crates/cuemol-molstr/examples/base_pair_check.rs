use cuemol_molstr::{read_pdb_file, read_qdf_mol_bytes, MolCoord, ResidueKey};
use std::{collections::BTreeMap, path::PathBuf};

fn pairs(molecule: &MolCoord) -> BTreeMap<ResidueKey, String> {
    molecule
        .residue_props()
        .iter()
        .filter_map(|(key, properties)| {
            properties
                .get("basepair")
                .map(|value| (key.clone(), value.clone()))
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if arguments.len() != 2 {
        return Err("usage: base_pair_check INPUT_PDB REFERENCE_QDF".into());
    }
    let mut molecule = read_pdb_file(&arguments[0])?;
    let reference = read_qdf_mol_bytes(&std::fs::read(&arguments[1])?)?;
    let count = molecule.calc_base_pair(3.7, 30.);
    assert_eq!(molecule.atom_count(), reference.atom_count());
    assert_eq!(pairs(&molecule), pairs(&reference));
    println!(
        "atoms={} paired_residues={} pairs={count}",
        molecule.atom_count(),
        pairs(&reference).len()
    );
    Ok(())
}
