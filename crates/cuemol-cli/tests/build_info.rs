use std::process::Command;

#[test]
fn build_information_is_available_without_loading_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.tsv");
    std::fs::write(&config, "invalid config").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cuemol"))
        .arg("--build-info")
        .env("CUEMOL_CONFIG_FILE", &config)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(info["target"]
        .as_str()
        .is_some_and(|target| !target.is_empty()));
    let features = info["features"].as_array().unwrap();
    assert_eq!(
        features.iter().any(|feature| feature == "gui"),
        cfg!(feature = "gui")
    );
    assert_eq!(
        features.iter().any(|feature| feature == "bundled-web"),
        cfg!(feature = "bundled-web")
    );
    assert_eq!(
        features.iter().any(|feature| feature == "scripting"),
        cfg!(feature = "scripting")
    );
    assert!(info["version"].is_string());
}
