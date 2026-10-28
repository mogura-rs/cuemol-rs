use cuemol_molstr::{label_format::LabelFormat, read_pdb_file};
use std::{fs, path::PathBuf, process::Command};

#[test]
fn labels_match_cpp_fields_specifiers_and_rejected_formats() {
    let Some(oracle) = std::env::var_os("CUEMOL_LABEL_ORACLE") else {
        eprintln!("skipping label oracle: run scripts/check-reference-labels.py");
        return;
    };
    let reference = PathBuf::from(std::env::var_os("CUEMOL2_REF_DIR").unwrap());
    let config = std::env::var_os("CUEMOL_LABEL_CONFIG").unwrap();
    let pdb = reference.join("tests/test_data/1CRN.pdb");
    let mut molecule = read_pdb_file(&pdb).unwrap();
    let first = molecule.atoms()[0].clone();
    molecule
        .set_residue_prop_str(
            &first.chain_id,
            &first.res_id.to_string(),
            "phase",
            "helix λ界",
        )
        .unwrap();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/label-reference");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("formats.txt");
    let output = directory.join("output.txt");
    let mut formats = vec![
        "{name}|{resn}|{resi}|{chain}|{elem}|{alt}|{aid}|{molname}".into(),
        "{{{name}}}: {rprop.phase} {rprop.absent:>20} {aprop.missing:.2f}".into(),
        "{aprop.flag:d} {aprop.score:.3f} {aprop.flag:.2f} {aprop.score:s}".into(),
    ];
    for spec in [
        "", "s", ">8", "*^8", ".1", "_>6.1s", "?", "d", "+", "#", "05", "{}",
    ][..]
        .iter()
    {
        formats.push(format!("{{name:{spec}}}"));
    }
    for spec in [
        "", "d", "+06d", "#08x", "#X", "#b", "#B", "#o", "*>9d", "^10", ".2", "f", "s", "c", "+c",
    ] {
        formats.push(format!("{{aid:{spec}}}"));
        formats.push(format!("{{aprop.flag:{spec}}}"));
    }
    for spec in [
        "", ".0", ".3", "f", "F", ".2f", "+010.2f", " #12.0f", "e", "E", ".0e", ".3E", "g", "G",
        ".0g", ".6g", "#.6g", "a", "A", ".0a", ".3a", "#.0A", "d", "s", ".f", "5.3.2", "0>12.2f",
    ] {
        formats.push(format!("{{bfac:{spec}}}"));
    }
    formats.extend(
        [
            "{",
            "}",
            "{}",
            "{unknown}",
            "{aprop.}",
            "{rprop.}",
            "{{text}}",
            "{resi:d}",
        ]
        .into_iter()
        .map(str::to_string),
    );
    for spec in [
        "L", "14", ">14", ".7", ".8", ".9", ".10", ".0?", ".1?", ".7?", ".9?", ".11?", "14?",
    ] {
        formats.push(format!("{{rprop.phase:{spec}}}"));
    }
    fs::write(&input, formats.join("\n") + "\n").unwrap();
    let mut comparisons = 0;
    for bfactor in [12.345_f32, 0., -0., -1234.5, 0.00001, 1e20, 9.99999] {
        let result = Command::new(&oracle)
            .args([
                config.as_os_str(),
                pdb.as_os_str(),
                input.as_os_str(),
                output.as_os_str(),
            ])
            .arg(f64::from(bfactor).to_string())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let expected = fs::read_to_string(&output).unwrap();
        let mut lines = expected.lines();
        let mut atom = first.clone();
        atom.serial = lines.next().unwrap().parse().unwrap();
        atom.b_factor = bfactor;
        atom.occupancy = 0.75;
        atom.set_int_prop("flag", -7);
        atom.set_real_prop("score", 1.25);
        for (format, expected) in formats.iter().zip(lines) {
            let expected = if let Some(hex) = expected.strip_prefix("ok ") {
                let bytes = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect::<Vec<_>>();
                format!(
                    "ok {}",
                    String::from_utf8_lossy(&bytes)
                        .bytes()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                )
            } else {
                expected.to_string()
            };
            let actual = match LabelFormat::new(format) {
                Ok(format) => format!(
                    "ok {}",
                    format
                        .apply(&atom, &molecule, "sample")
                        .bytes()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                ),
                Err(_) => "error".into(),
            };
            assert_eq!(actual, expected, "bfac={bfactor}, format={format}");
            comparisons += 1;
        }
    }
    eprintln!("Compared {comparisons} label formats with the C++ reference");
}
