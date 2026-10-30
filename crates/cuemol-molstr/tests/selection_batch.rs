use cuemol_core::Vector4;
use cuemol_molstr::selection::DistanceOp;
use cuemol_molstr::{parse_selector, Atom, BondOrder, MolCoord, Selector};
use std::collections::BTreeMap;

fn atom(index: usize, position: Vector4) -> Atom {
    Atom {
        serial: index as i32 + 1,
        name: ["N", "CA", "C", "CB"][index % 4].into(),
        alt_loc: index.is_multiple_of(9).then_some('A'),
        res_name: "ALA".into(),
        chain_id: if index % 16 < 8 { "A" } else { "B" }.into(),
        res_id: (index / 4) as i32,
        ins_code: index.is_multiple_of(3).then_some('A'),
        position,
        occupancy: 1.,
        b_factor: index as f32,
        element: if index.is_multiple_of(4) { "N" } else { "C" }.into(),
        formal_charge: None,
        int_props: BTreeMap::new(),
        real_props: BTreeMap::new(),
    }
}

fn assert_scalar_equivalent(molecule: &MolCoord, selector: &Selector) {
    let expected: Vec<_> = molecule
        .atoms()
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| {
            selector
                .matches_in_molecule(atom, molecule)
                .then_some(index)
        })
        .collect();
    assert_eq!(molecule.select_indices(selector), expected, "{selector}");
    assert_eq!(
        molecule.atom_sel_count(selector),
        expected.len(),
        "{selector}"
    );
}

#[test]
fn batch_selection_preserves_nested_residues_properties_and_coordinates() {
    let mut molecule = MolCoord::with_atoms(
        (0..280)
            .map(|index| {
                atom(
                    index,
                    Vector4::new3(
                        (index % 8) as f64 * 1.2 - 4.8,
                        ((index / 8) % 7) as f64 * 1.3 - 3.9,
                        (index / 56) as f64 * 1.4 - 2.8,
                    ),
                )
            })
            .collect(),
    );
    molecule
        .set_residue_prop_str("A", "0A", "test", "yes")
        .unwrap();
    for expression in [
        "name CA",
        "byres (name CA)",
        "byres (chain 'A' and name CB)",
        "byres (rprop test=yes)",
        "not (byres (name CA))",
        "byres (name CA around 1.5)",
        "(name CA) around 1.5",
        "(name CA) expand 1.5",
        "(name CA) around 1.5 and (elem N or bfac > 200)",
        "(none around 2) and (name CA around 2)",
        "(* expand 2) or (name CA around 2)",
        "(name CA around 1.5) around 1.5",
        "(name CA around 1.5) expand 1.5",
        "bymainch (byres name CB)",
        "bysidech (name CA expand 2)",
        "(name CA) around 3 [other]",
        "(name CA) around 0",
        "(name CA) expand 0",
        "none around 3",
        "* around 3",
        "* expand 3",
    ] {
        assert_scalar_equivalent(&molecule, &parse_selector(expression).unwrap());
    }
    let selector = parse_selector("byres ((name CA) around 1.5)").unwrap();
    molecule.atoms_mut()[2].position.x = 1000.;
    molecule.atoms_mut()[7].res_id = 100;
    assert_scalar_equivalent(&molecule, &selector);
    assert!(MolCoord::new().select_indices(&selector).is_empty());
}

#[test]
fn sparse_distance_queries_keep_strict_boundaries_and_live_coordinates() {
    let mut atoms: Vec<_> = (0..20)
        .map(|index| atom(index, Vector4::new3(index as f64 * 10., 0., 0.)))
        .collect();
    for (index, x) in [
        f64::from_bits(3_f64.to_bits() - 1),
        3.,
        -3.,
        -f64::from_bits(3_f64.to_bits() - 1),
        5.,
        1000.,
    ]
    .into_iter()
    .enumerate()
    {
        atoms.push(atom(20 + index, Vector4::new3(x, 0., 0.)));
    }
    let mut molecule = MolCoord::with_atoms(atoms);
    let around = parse_selector("(aid 1:20) around 3").unwrap();
    let expand = parse_selector("(aid 1:20) expand 3").unwrap();
    assert_eq!(molecule.select_indices(&around), [20, 23]);
    assert_eq!(
        molecule.select_indices(&expand),
        (0..20).chain([20, 23]).collect::<Vec<_>>()
    );
    molecule.atoms_mut()[20].position.x = 3.1;
    assert_eq!(molecule.select_indices(&around), [23]);
    assert_scalar_equivalent(&molecule, &expand);
}

#[test]
fn distance_queries_preserve_nonfinite_and_extreme_input_behavior() {
    let mut atoms: Vec<_> = (0..40)
        .map(|index| atom(index, Vector4::new3(index as f64, 0., 0.)))
        .collect();
    for (index, x) in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
    ]
    .into_iter()
    .enumerate()
    {
        atoms.push(atom(40 + index, Vector4::new3(x, 0., 0.)));
    }
    let molecule = MolCoord::with_atoms(atoms);
    for expression in ["aid 1:20", "aid 1,41:46", "none"] {
        for distance in [
            0.,
            -1.,
            3.,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::NAN,
            f64::INFINITY,
        ] {
            for op in [DistanceOp::Around, DistanceOp::Expand] {
                let selector = Selector::Distance {
                    op,
                    inner: Box::new(parse_selector(expression).unwrap()),
                    distance,
                    molecule: None,
                };
                assert_scalar_equivalent(&molecule, &selector);
            }
        }
    }
}

#[test]
fn graph_queries_preserve_cycles_depth_and_duplicate_identity_behavior() {
    let mut atoms: Vec<_> = (0..9)
        .map(|index| atom(index, Vector4::new3(index as f64, 0., 0.)))
        .collect();
    atoms[8] = atoms[0].clone();
    atoms[8].set_int_prop("marked", 1);
    let mut molecule = MolCoord::with_atoms(atoms);
    for (first, second) in [(0, 1), (1, 2), (2, 3), (3, 0), (3, 4), (5, 6), (7, 8)] {
        molecule.add_bond(first, second, BondOrder::Single);
    }
    assert_eq!(
        molecule.select_indices(&parse_selector("(aid 1) neighbor 1").unwrap()),
        [1, 3, 7]
    );
    assert_eq!(
        molecule.select_indices(&parse_selector("(aid 1) extend 1.1").unwrap()),
        [0, 1, 2, 3, 4, 7, 8]
    );
    for inner in ["aid 1", "aprop marked=1", "name CA", "none", "*"] {
        for distance in [0., 1., 1.1, 10., f64::INFINITY] {
            for op in [DistanceOp::Neighbor, DistanceOp::Extend] {
                let selector = Selector::Distance {
                    op,
                    inner: Box::new(parse_selector(inner).unwrap()),
                    distance,
                    molecule: None,
                };
                assert_scalar_equivalent(&molecule, &selector);
            }
        }
    }
}

#[test]
fn scene_distance_grid_matches_world_space_scan_with_extreme_coordinates() {
    use cuemol_core::Matrix4;
    use cuemol_molstr::MolSceneManager;

    let mut atoms: Vec<_> = (0..280)
        .map(|index| {
            atom(
                index,
                Vector4::new3((index % 8) as f64 * 1.2, (index / 8) as f64 * 1.3, 0.),
            )
        })
        .collect();
    for value in [f64::NAN, f64::INFINITY, f64::MAX, f64::MIN] {
        atoms.push(atom(atoms.len(), Vector4::new3(value, 0., 0.)));
    }
    let mut manager = MolSceneManager::new();
    let scene_id = manager.new_scene("scene");
    let scene = manager.scene_mut(scene_id).unwrap();
    let probe = scene.add_molecule("Probe", MolCoord::with_atoms(atoms.clone()));
    let target = scene.add_molecule("Target", MolCoord::with_atoms(atoms));
    let mut transform = Matrix4::IDENTITY;
    transform.set_trans_part(Vector4::new3(-1.7, 2.9, 0.));
    scene
        .object_mut(target)
        .unwrap()
        .set_xform_matrix(transform);
    let probe = scene.object(probe).unwrap();
    let target = scene.object(target).unwrap();
    for cutoff in [0., 1.3, 3., f64::MIN_POSITIVE, f64::MAX, f64::INFINITY] {
        let selector = Selector::Distance {
            op: DistanceOp::Around,
            inner: Box::new(parse_selector("name CA").unwrap()),
            distance: cutoff,
            molecule: Some("Target".into()),
        };
        let expected: Vec<_> = probe
            .mol()
            .atoms()
            .iter()
            .enumerate()
            .filter_map(|(index, atom)| {
                target
                    .mol()
                    .atoms()
                    .iter()
                    .filter(|other| other.name == "CA")
                    .any(|other| {
                        let mut position = other.position;
                        transform.xform3d(&mut position);
                        (atom.position - position).length3d() < cutoff
                    })
                    .then_some(index)
            })
            .collect();
        let bound = selector.bind_to_scene(probe, scene);
        assert_eq!(probe.mol().select_indices(&bound), expected, "{cutoff}");
        assert_eq!(probe.mol().atom_sel_count(&bound), expected.len());
    }
}
