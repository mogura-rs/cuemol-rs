use cuemol_molstr::SelCommand;

#[test]
fn incomplete_arguments_match_published_reference_validation() {
    let mut command = SelCommand::new();
    for expression in ["chain (", "(", "resid nope", "chain A and"] {
        assert!(!command.compile(expression, 0), "{expression}");
    }
    for expression in ["chain 'A'", "'A'.-2:-1.*", "not ('A'.-2:-1.*)"] {
        assert!(command.compile(expression, 0), "{expression}");
    }
}

#[test]
fn residue_insertion_selection_matches_published_reference() {
    let mut pdb = String::new();
    for (index, (residue, insertion)) in
        [(-1, ' '), (1, ' '), (1, 'A'), (1, 'B'), (2, ' '), (2, 'A')]
            .into_iter()
            .enumerate()
    {
        pdb.push_str(&format!(
            "ATOM  {:5}  CA  GLY A{residue:4}{insertion}   {:8.3}{:8.3}{:8.3}  1.00 10.00           C  \n",
            index + 1, index as f64, 0., 0.
        ));
    }
    let molecule = cuemol_molstr::read_pdb_str(&pdb).unwrap();
    for (expression, expected) in [
        ("'A'.1.*", vec![1]),
        ("'A'.1A.*", vec![2]),
        ("resi 1", vec![1]),
        ("resi 1A", vec![2]),
        ("resi 1:2", vec![1, 2, 3, 4]),
        ("resi 1A:2", vec![2, 3, 4]),
        ("resi 1:1B", vec![1, 2, 3]),
        ("resi 1B:2A", vec![3, 4, 5]),
        ("resi -1:1A", vec![0, 1, 2]),
        ("resi 1B,2A", vec![3, 5]),
        ("resi 2A:1B", vec![3, 4, 5]),
    ] {
        let selection = cuemol_molstr::parse_selector(expression).unwrap();
        assert_eq!(
            molecule.select_indices(&selection),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn altconf_selection_dump_matches_reference_cases() {
    let mut cmd = SelCommand::new();
    for (expression, expected) in [
        ("alt A", "alt A"),
        ("alt A,B,C", "alt C,B,A"),
        ("alt null", "alt (empty)"),
        ("alt A,null,B", "alt B,(empty),A"),
        ("alt /^A/", "alt /^A/"),
        ("alt A:B,C", "alt C,A:B"),
        ("alt 1,2", "alt 2,1"),
        ("alt \"1A\"", "alt \"1A\""),
    ] {
        assert!(
            cmd.compile(expression, 0),
            "{expression}: {}",
            cmd.error_msg
        );
        assert_eq!(cmd.dump_nodes(), expected, "{expression}");
    }
}

#[test]
fn hierarchical_selection_dump_matches_reference_cases() {
    let mut cmd = SelCommand::new();
    for (expression, expected) in [
        ("A.1.CA", "A.1.CA"),
        ("A.1.CA,N,O", "A.1.O,N,CA"),
        ("*.1.CA", "*.1.CA"),
        ("A.*.CA", "A.*.CA"),
        ("A,B,C.1:10.CA", "C,B,A.1:10.CA"),
        ("\"A\",\"B\".1.\"CA\",\"N\"", "\"B\",\"A\".1.\"N\",\"CA\""),
        ("null.1.CA", "(empty).1.CA"),
        ("A . 1 . CA", "A.1.CA"),
        ("A.10A,10B.CA", "A.10A:10B.CA"),
    ] {
        assert!(
            cmd.compile(expression, 0),
            "{expression}: {}",
            cmd.error_msg
        );
        assert_eq!(cmd.dump_nodes(), expected, "{expression}");
    }
}

#[test]
fn around_and_unary_selection_dump_matches_reference_cases() {
    let mut cmd = SelCommand::new();
    for (expression, expected) in [
        ("name CA around 3", "(name CA) around 3.000000"),
        ("name CA a;3.5", "(name CA) around 3.500000"),
        (
            "chain A & name CA around 3",
            "(chain A) and ((name CA) around 3.000000)",
        ),
        (
            "name CA around 3 & chain A",
            "((name CA) around 3.000000) and (chain A)",
        ),
        (
            "name CA around 3 [mol1]",
            "(name CA) around 3.000000 [\"mol1\"]",
        ),
        (
            "name CA around [mol1] 3",
            "(name CA) around 3.000000 [\"mol1\"]",
        ),
        (
            "byres name CA around 3",
            "byres ((name CA) around 3.000000)",
        ),
        (
            "bysidech name CA expand 3",
            "bysidech ((name CA) expand 3.000000)",
        ),
    ] {
        assert!(
            cmd.compile(expression, 0),
            "{expression}: {}",
            cmd.error_msg
        );
        assert_eq!(cmd.dump_nodes(), expected, "{expression}");
    }
}

#[test]
fn rprop_selection_dump_matches_reference_cases() {
    let mut cmd = SelCommand::new();
    for (expression, expected) in [
        ("rprop secondary=helix", "rprop secondary=helix"),
        ("rprop secondary = helix", "rprop secondary=helix"),
        (
            "rprop propname = \"value name\"",
            "rprop propname=value name",
        ),
        ("rprop secondary=/^hel/", "rprop secondary=^hel"),
        ("rprop count=1.23", "rprop count=1.230000"),
        ("rprop index=-5.6", "rprop index=-5.600000"),
    ] {
        assert!(
            cmd.compile(expression, 0),
            "{expression}: {}",
            cmd.error_msg
        );
        assert_eq!(cmd.dump_nodes(), expected, "{expression}");
    }
}
