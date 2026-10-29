#![cfg(feature = "scripting")]

use std::process::Command;

#[test]
fn script_file_and_eval_receive_arguments_without_shell_reinterpretation() {
    let temp = tempfile::tempdir().unwrap();
    let source = "print(getCmdArgs(-1)); print(getCmdArgs(0)); print(getCmdArgs(1)); print('['+getCmdArgs(2)+']');";
    let script = temp.path().join("arguments.js");
    std::fs::write(&script, source).unwrap();
    for (command, input) in [("run", script.to_str().unwrap()), ("-c", source)] {
        let output = Command::new(env!("CARGO_BIN_EXE_cuemol"))
            .args([command, input, "日本語 with spaces", "$(literal);'\"", ""])
            .env("XDG_CONFIG_HOME", temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "3\n日本語 with spaces\n$(literal);'\"\n[]\n"
        );
    }
}
