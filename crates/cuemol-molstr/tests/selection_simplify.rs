use cuemol_molstr::{parse_selector, read_pdb_str, MolCoord, SelCommand};

fn command(expression: &str) -> SelCommand {
    let mut command = SelCommand::new();
    command
        .compile_result(expression)
        .unwrap_or_else(|error| panic!("{expression}: {error}"));
    command
}

#[test]
fn reference_simplification_rules_preserve_canonical_text() {
    for (input, expected) in [
        ("(name CA or elem N) or name CB", "name CA,CB or elem N"),
        ("resn ALA and *", "resn ALA"),
        ("resn ALA and none", "none"),
        ("not not resn ALA", "resn ALA"),
        (
            "(resn ALA and chain A) or (chain A and resn ALA)",
            "resn ALA and chain A",
        ),
        ("chain A or not chain A", "*"),
        ("chain A and not chain A", "none"),
        ("chain A or (chain A and resn ALA)", "chain A"),
        ("chain A and (chain A or resn ALA)", "chain A"),
        (
            "(chain A or resn ALA) and not resn ALA",
            "chain A and not resn ALA",
        ),
        (
            "(chain A and not resn ALA) or resn ALA",
            "chain A or resn ALA",
        ),
        (
            "(chain A and not resn GLY) or (chain A and resn GLY)",
            "chain A",
        ),
        ("aid 5 or aid 9 or aid 1:3", "aid 1:3,5,9"),
        ("resi 1:10 or resi 5:20", "resi 1:20"),
        ("A.12.* or A.13.* or A.20.*", "A.12,13,20.*"),
        ("byres byres resn ALA", "byres resn ALA"),
        (
            "((resn ALA) around 3.0000001) or ((resn ALA) around 3.0000002)",
            "((resn ALA) around 3.0000001) or ((resn ALA) around 3.0000002)",
        ),
    ] {
        assert_eq!(command(input).simplify(), expected, "{input}");
    }
}

fn molecule() -> MolCoord {
    let mut pdb = String::new();
    for (number, insertion, name) in [
        (12, ' ', "ALA"),
        (12, 'A', "GLY"),
        (13, ' ', "ALA"),
        (20, ' ', "HOH"),
    ] {
        for atom in ["N", "CA", "CB"] {
            let serial = pdb.lines().count() + 1;
            pdb.push_str(&format!("ATOM  {serial:5} {atom:^4} {name:3} A{number:4}{insertion}   {:8.3}{:8.3}{:8.3}  1.00 10.00           {:>2}\n",
                serial as f64, 0., 0., if atom == "N" { "N" } else { "C" }));
        }
    }
    let mut molecule = read_pdb_str(&pdb).unwrap();
    molecule
        .set_residue_prop_str("A", "12A", "tag", "alpha beta")
        .unwrap();
    molecule
}

#[test]
fn simplification_preserves_atoms_and_insertion_codes() {
    let molecule = molecule();
    for input in [
        "A.12.* or A.13.*",
        "resi 12 or resi 13",
        "resi 12A or resi 13",
        "resi 12:13 or resi 12A:20",
        "(chain A and not resn GLY) or (chain A and resn GLY and name CA)",
        "not (name CA or name CB) and not resn HOH",
        "bfac > 0.5 or occ < 2",
        "name null or name CA",
        "alt null",
        "null.12.*",
        "rprop tag=\"alpha beta\" or resn HOH",
        "none or ((name CA or name N) and not resn GLY)",
        "(name CA and chain A) or (resn ALA and chain A) or (name CA and not chain A)",
    ] {
        let before = parse_selector(input).unwrap();
        let text = command(input).simplify();
        let after = parse_selector(&text).unwrap();
        assert_eq!(
            molecule.select_indices(&before),
            molecule.select_indices(&after),
            "{input} => {text}"
        );
    }
    let selected = parse_selector(&command("resi 12 or resi 13").simplify()).unwrap();
    assert!(molecule
        .select_indices(&selected)
        .iter()
        .all(|index| molecule.atoms()[*index].ins_code.is_none()));
}

#[test]
fn repeated_visibility_and_style_changes_do_not_grow_selections() {
    let mut current = "chain A".to_string();
    for _ in 0..100 {
        current = command(&format!("({current}) or (resn ALA)")).simplify();
        assert_eq!(current, "chain A or resn ALA");
        current = command(&format!("({current}) and not (resn ALA)")).simplify();
        assert_eq!(current, "chain A and not resn ALA");
    }
    let mut stick = "chain A".to_string();
    for _ in 0..100 {
        let sphere = command(&format!("(resn GLY) and ({stick})")).simplify();
        stick = command(&format!("({stick}) and not (resn GLY)")).simplify();
        stick = command(&format!("({stick}) or ((resn GLY) and ({sphere}))")).simplify();
        assert_eq!(stick, "chain A");
    }
}

#[test]
fn context_edits_use_outer_operators_and_leave_typed_expression_intact() {
    let original = "not chain A or not chain B";
    let selection = command(original);
    let inverted = selection.invert_simplified();
    assert_eq!(inverted, "not (not chain A or not chain B)");
    assert_eq!(command(&inverted).invert_simplified(), original);
    assert_eq!(selection.to_string(), original);
    let selection = command("chain A or bysidech chain B");
    let side = selection.toggle_sidechain();
    assert_eq!(side, "bysidech (chain A or (bysidech chain B))");
    assert_eq!(
        command(&side).toggle_sidechain(),
        "chain A or (bysidech chain B)"
    );
    assert_eq!(
        command("byres resn ALA around 3").rewrite_around(5., false),
        "(resn ALA) around 5.0"
    );
    assert_eq!(
        command("(resn ALA) around 3 [other]").rewrite_around(7., true),
        "byres ((resn ALA) around 7.0)"
    );
    assert_eq!(SelCommand::new().simplify(), "");
    assert_eq!(SelCommand::new().invert_simplified(), "*");
}

#[test]
fn written_selection_keeps_properties_references_and_float_precision() {
    for input in [
        "rprop tag=\"alpha beta\"",
        "rprop tag=\"\"",
        "rprop tag=null",
        "rprop tag=\"001\"",
        "alt null",
        "focus",
        "none",
        "bfac > 10.0000001",
        "occ < 0.0000011",
        "(chain A) around 3.0000001 [\"other molecule\"]",
    ] {
        let selection = parse_selector(input).unwrap();
        let text = selection.expression();
        assert_eq!(
            selection,
            parse_selector(&text).unwrap(),
            "{input} => {text}"
        );
        let simplified = command(input).simplify();
        assert_eq!(
            selection,
            parse_selector(&simplified).unwrap(),
            "{input} => {simplified}"
        );
    }
    let molecule = molecule();
    assert!(molecule
        .select_indices(&parse_selector("bfac = 10.0000001").unwrap())
        .is_empty());
    assert!(
        molecule
            .select_indices(&parse_selector("bfac < 10.0000001").unwrap())
            .len()
            == molecule.atom_count()
    );
}

#[test]
fn rewriting_and_serialization_match_current_cpp_reference() {
    use std::{
        fs,
        io::Write,
        path::PathBuf,
        process::{Command, Stdio},
    };
    let Some(oracle) = std::env::var_os("CUEMOL_SELECTION_ORACLE") else {
        eprintln!("skipping selection oracle: run scripts/check-reference-selection.py");
        return;
    };
    let config = std::env::var_os("CUEMOL_SELECTION_CONFIG").unwrap();
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.cache/selection-reference/output.tsv");
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    let mut inputs: Vec<String> = [
        "*",
        "none",
        "alt null",
        "null.12.*",
        "alt A,B,C",
        "name CA,N,CB",
        "A.12.* or A.13.*",
        "resi 12 or resi 13",
        "resi 12A or resi 12B",
        "resi 1:10 or resi 5:20",
        "chain A or not chain A",
        "(name CA or elem N) or name CB",
        "byres byres resn ALA",
        "not chain A or not chain B",
        "chain A or bysidech chain B",
        "byres resn ALA around 3",
        "(resn ALA) around 3 [other]",
        "(name CA) around 3.0000001",
        "rprop tag=\"alpha beta\"",
        "rprop tag=\"001\"",
        "rprop tag=\"\"",
        "rprop tag=null",
        "bfac > 10.0000001",
        "occ < 0.0000011",
        "(resn ALA and chain A) or (chain A and resn ALA)",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for a in ["name CA", "chain A", "resn GLY", "aid 3", "resi 12A"] {
        for b in ["name CB", "chain B", "resn ALA", "aid 5", "resi 13"] {
            inputs.extend([
                format!("({a}) or ({b}) or ({a})"),
                format!("({a}) and (({a}) or ({b}))"),
                format!("(({a}) and not ({b})) or (({a}) and ({b}))"),
                format!("(({a}) or ({b})) and not ({b})"),
            ]);
        }
    }
    let mut child = Command::new(oracle)
        .arg(config)
        .arg(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(inputs.join("\n").as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let expected = fs::read_to_string(output).unwrap();
    let lines: Vec<_> = expected.lines().collect();
    assert_eq!(lines.len(), inputs.len());
    for (input, expected) in inputs.iter().zip(lines) {
        let selection = command(input);
        let actual = [
            selection.simplify(),
            selection.dump_nodes(),
            selection.invert_simplified(),
            selection.toggle_sidechain(),
            selection.rewrite_around(5., false),
        ]
        .join("\t");
        assert_eq!(actual, expected, "{input}");
    }
    eprintln!(
        "compared {} expressions and {} rewrite/serialization results",
        inputs.len(),
        inputs.len() * 5
    );
}
