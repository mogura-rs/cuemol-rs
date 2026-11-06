use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SavedAtoms {
    atoms: Vec<(usize, Atom)>,
    bonds: Vec<(i32, i32, Bond)>,
    residue_props: BTreeMap<ResidueKey, BTreeMap<String, String>>,
    pivots: Vec<(ResidueKey, Arc<str>)>,
    links: Option<BTreeMap<ResidueKey, ResidueKey>>,
}

fn identity(atom: &Atom) -> (&str, i32, Option<char>, &str, Option<char>) {
    (
        &atom.chain_id,
        atom.res_id,
        atom.ins_code,
        &atom.name,
        atom.alt_loc,
    )
}

impl SavedAtoms {
    pub(crate) fn capture(molecule: &MolCoord, indices: &BTreeSet<usize>) -> Result<Self> {
        let atoms = indices
            .iter()
            .filter_map(|&index| molecule.atoms.get(index).map(|atom| (index, atom.clone())))
            .collect::<Vec<_>>();
        let ids = atoms
            .iter()
            .map(|(_, atom)| atom.serial)
            .collect::<HashSet<_>>();
        let names = atoms
            .iter()
            .map(|(_, atom)| identity(atom))
            .collect::<HashSet<_>>();
        if ids.len() != atoms.len() || names.len() != atoms.len() {
            return Err(CueMolError::invalid_argument(
                "edited atoms must have unique IDs and names",
            ));
        }
        let residues = atoms
            .iter()
            .map(|(_, atom)| residue_key(atom))
            .collect::<BTreeSet<_>>();
        let bonds = molecule
            .bonds
            .iter()
            .filter_map(|bond| {
                if !indices.contains(&bond.atom1) && !indices.contains(&bond.atom2) {
                    return None;
                }
                Some((
                    molecule.atoms.get(bond.atom1)?.serial,
                    molecule.atoms.get(bond.atom2)?.serial,
                    bond.clone(),
                ))
            })
            .collect();
        let residue_props = residues
            .iter()
            .filter_map(|key| {
                molecule
                    .residue_props
                    .get(key)
                    .map(|properties| (key.clone(), properties.clone()))
            })
            .collect();
        let pivots = residues
            .iter()
            .filter_map(|key| {
                molecule
                    .residue_pivots
                    .get(&key.chain_id)?
                    .get(&(key.res_id, key.ins_code))
                    .map(|name| (key.clone(), name.clone()))
            })
            .collect();
        let links = molecule.residue_links.as_ref().map(|links| {
            links
                .iter()
                .filter(|(first, second)| residues.contains(first) || residues.contains(second))
                .map(|(first, second)| (first.clone(), second.clone()))
                .collect()
        });
        Ok(Self {
            atoms,
            bonds,
            residue_props,
            pivots,
            links,
        })
    }

    pub(crate) fn remove(&self, molecule: &mut MolCoord) -> bool {
        let ids = self
            .atoms
            .iter()
            .map(|(_, atom)| atom.serial)
            .collect::<HashSet<_>>();
        let indices = molecule
            .atoms
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| ids.contains(&atom.serial).then_some(index))
            .collect();
        molecule.delete_atom_indices(&indices);
        molecule.apply_topology();
        molecule.fire_atoms_moved();
        true
    }

    pub(crate) fn restore(&self, molecule: &mut MolCoord) -> bool {
        let ids = self
            .atoms
            .iter()
            .map(|(_, atom)| atom.serial)
            .collect::<HashSet<_>>();
        let names = self
            .atoms
            .iter()
            .map(|(_, atom)| identity(atom))
            .collect::<HashSet<_>>();
        if molecule
            .atoms
            .iter()
            .any(|atom| ids.contains(&atom.serial) || names.contains(&identity(atom)))
        {
            return false;
        }
        let mut atoms = Vec::with_capacity(molecule.atom_count() + self.atoms.len());
        let mut remap = vec![0; molecule.atom_count()];
        let mut bond_positions: HashMap<i32, Option<usize>> = self
            .bonds
            .iter()
            .flat_map(|(first, second, _)| [(*first, None), (*second, None)])
            .collect();
        let mut append = |atom: &Atom| {
            let index = atoms.len();
            if let Some(position) = bond_positions.get_mut(&atom.serial) {
                *position = Some(index);
            }
            atoms.push(atom.clone());
            index
        };
        let mut previous = molecule.atoms.iter().enumerate().peekable();
        let mut written = 0;
        for (position, saved) in &self.atoms {
            while written < *position {
                let Some((index, atom)) = previous.next() else {
                    break;
                };
                remap[index] = append(atom);
                written += 1;
            }
            append(saved);
            written += 1;
        }
        for (index, atom) in previous {
            remap[index] = append(atom);
        }
        let mut bonds = molecule
            .bonds
            .iter()
            .map(|bond| Bond {
                atom1: remap[bond.atom1],
                atom2: remap[bond.atom2],
                ..bond.clone()
            })
            .collect::<Vec<_>>();
        for (first, second, saved) in &self.bonds {
            if let (Some(Some(atom1)), Some(Some(atom2))) =
                (bond_positions.get(first), bond_positions.get(second))
            {
                bonds.push(Bond {
                    atom1: *atom1,
                    atom2: *atom2,
                    ..saved.clone()
                });
            }
        }
        molecule.next_atom_serial = molecule.next_serial().max(
            self.atoms
                .iter()
                .map(|(_, atom)| i64::from(atom.serial) + 1)
                .max()
                .unwrap_or(1),
        );
        molecule.residue_count.take();
        molecule.atoms = Arc::new(atoms);
        molecule.bonds = Arc::new(bonds);
        for (key, properties) in &self.residue_props {
            Arc::make_mut(&mut molecule.residue_props)
                .entry(key.clone())
                .or_default()
                .extend(properties.clone());
        }
        for (key, name) in &self.pivots {
            insert_residue_pivot(
                Arc::make_mut(&mut molecule.residue_pivots),
                key,
                name.clone(),
            );
        }
        if let Some(saved) = &self.links {
            let residues = molecule
                .atoms
                .iter()
                .map(residue_key)
                .collect::<BTreeSet<_>>();
            let links = molecule.residue_links.get_or_insert_with(Arc::default);
            Arc::make_mut(links).extend(
                saved
                    .iter()
                    .filter(|(first, second)| residues.contains(first) && residues.contains(second))
                    .map(|(first, second)| (first.clone(), second.clone())),
            );
        }
        molecule.apply_topology();
        molecule.fire_atoms_moved();
        true
    }
}

impl MolCoord {
    pub(crate) fn removal_set(&self, selected: &BTreeSet<usize>) -> BTreeSet<usize> {
        let base_names = selected
            .iter()
            .filter_map(|&index| self.atoms.get(index))
            .filter(|atom| atom.alt_loc.is_none())
            .map(|atom| (&atom.chain_id, atom.res_id, atom.ins_code, &atom.name))
            .collect::<HashSet<_>>();
        let mut result = selected.clone();
        if !base_names.is_empty() {
            result.extend(self.atoms.iter().enumerate().filter_map(|(index, atom)| {
                (atom.alt_loc.is_some()
                    && base_names.contains(&(
                        &atom.chain_id,
                        atom.res_id,
                        atom.ins_code,
                        &atom.name,
                    )))
                .then_some(index)
            }));
        }
        result
    }
}
