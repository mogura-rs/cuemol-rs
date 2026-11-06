use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn chimerax_scripts_and_stdin_share_operations_and_preserve_prior_edits_on_error() {
    let temp = tempfile::tempdir().unwrap();
    let pdb = temp.path().join("input.pdb");
    std::fs::write(&pdb,"ATOM      1  CA  ALA A   1       1.000   2.000   3.000  1.00 10.00           C\nATOM      2  N   ALA A   1       2.000   3.000   4.000  1.00 10.00           N\nEND\n").unwrap();
    let source =
        "open input.pdb\nselect @CA\ninfo atoms sel\nsave output.data format pdb models #1\n";
    std::fs::write(temp.path().join("input.cxc"), source).unwrap();
    for args in [
        vec!["run", "input.cxc"],
        vec!["run", "--dialect", "chimerax", "input.cxc"],
        vec!["console", "--dialect", "chimerax", "-c", source],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_cuemol"))
            .args(args)
            .current_dir(temp.path())
            .env("CUEMOL_CONFIG_DIR", temp.path().join("config"))
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        let output = String::from_utf8(result.stdout).unwrap();
        assert!(output.contains("1 atoms selected"), "{output}");
        assert!(output.contains("atom id #1/A:1@CA"), "{output}");
        assert!(std::fs::read_to_string(temp.path().join("output.data"))
            .unwrap()
            .contains("ATOM"));
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args(["console", "--dialect", "chimerax"])
        .current_dir(temp.path())
        .env("CUEMOL_CONFIG_DIR", temp.path().join("config"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"open input.pdb\nselect @CA\nundo\ninfo atoms sel\nredo\ninfo atoms sel\n")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{result:?}");
    let output = String::from_utf8(result.stdout).unwrap();
    assert_eq!(output.matches("atom id #1/A:1@CA").count(), 1, "{output}");
}

#[test]
fn pymol_script_extension_and_bad_dialect_are_handled_without_javascript() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("probe.pml");
    std::fs::write(&path, "help count_atoms\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args(["run", path.to_str().unwrap()])
        .env("CUEMOL_CONFIG_DIR", temp.path().join("config"))
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let result = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .args(["console", "--dialect", "unknown", "-c", "help"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Unknown dialect"));
}
