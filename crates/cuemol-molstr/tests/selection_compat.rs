use cuemol_molstr::SelCommand;

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
