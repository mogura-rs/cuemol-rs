#![cfg(feature = "scripting")]

use std::process::Command;

#[test]
fn offline_skill_and_preferences_do_not_execute_startup() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("startup.js"),
        "scene.setName('startup-scene'); print('startup-marker');",
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_cuemol"))
            .args(args)
            .env("CUEMOL_CONFIG_DIR", directory.path())
            .env("CUEMOL_CONFIG_FILE", directory.path().join("config.tsv"))
            .output()
            .unwrap()
    };
    for args in [
        vec!["skill"],
        vec!["config", "path"],
        vec!["config", "set", "gui:theme", "dark"],
        vec!["config", "get", "gui:theme"],
    ] {
        let output = run(&args);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    assert_eq!(run(&["config", "get", "gui:theme"]).stdout, b"dark\n");
    for (key, value) in [
        ("probe:value", ""),
        ("probe:value", r"models\new\test.pdb"),
        ("probe:value", "  selection  "),
        ("probe:value", "first\r\nsecond\t"),
        ("#theme", "custom-value"),
        (" #theme", ""),
        (" ", ""),
    ] {
        let saved = run(&["config", "set", key, value]);
        assert!(saved.status.success(), "{saved:?}");
        let loaded = run(&["config", "get", key]);
        assert!(loaded.status.success(), "{loaded:?}");
        assert_eq!(loaded.stdout, format!("{value}\n").as_bytes());
        assert!(saved.stderr.is_empty() && loaded.stderr.is_empty());
    }
    assert!(!run(&["skill", "unexpected"]).status.success());
    let script = run(&["-c", "print(scene.name);"]);
    assert!(script.status.success(), "{script:?}");
    assert_eq!(script.stdout, b"startup-scene\n");
    assert!(String::from_utf8_lossy(&script.stderr).contains("startup-marker"));
}
