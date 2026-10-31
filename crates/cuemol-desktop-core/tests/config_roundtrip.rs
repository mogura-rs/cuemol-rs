use cuemol_desktop_core::DesktopAppState;

#[test]
fn config_files_and_browser_text_preserve_values_exactly() {
    let mut original = DesktopAppState::new();
    for (key, value) in [
        ("empty", ""),
        ("spaces", "  selection  "),
        ("path", r"models\new\test.pdb"),
        ("literal", r"\n\r\t\\"),
        ("lines", "first\r\nsecond\nthird\tfourth"),
        ("unicode", "αβγ"),
        ("comment", "# value"),
    ] {
        original.put_config_value(&format!("probe:{key}"), value);
    }
    for key in [
        "#theme",
        " #theme",
        "probe:tab\tkey",
        "probe:path\\key",
        " ",
        "\n",
    ] {
        original.put_config_value(key, "");
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.tsv");
    original.save_config_file(&path).unwrap();
    let mut from_file = DesktopAppState::new();
    from_file.load_config_file(path).unwrap();
    let mut from_text = DesktopAppState::new();
    from_text.load_config_text(&original.config_text()).unwrap();
    assert_eq!(from_file.config_entries(), original.config_entries());
    assert_eq!(from_text.config_entries(), original.config_entries());
}

#[test]
fn versioned_config_recovers_unescaped_comment_keys() {
    let mut state = DesktopAppState::new();
    let input = "# cuemol-config-v2\n# comment\n#theme\tblue\n #theme\tgreen\n";
    assert_eq!(state.load_config_text(input).unwrap(), 2);
    assert_eq!(state.config_value("#theme"), "blue");
    assert_eq!(state.config_value(" #theme"), "green");
}

#[test]
fn legacy_config_keeps_empty_values_spaces_and_newline_escapes() {
    let mut state = DesktopAppState::new();
    let input = "# Legacy preferences\r\nprobe:empty\t\r\nprobe:text\t line1\\nline2 \r\n";
    assert_eq!(state.load_config_text(input).unwrap(), 2);
    assert_eq!(state.config_value("probe:empty"), "");
    assert_eq!(state.config_value("probe:text"), " line1\nline2 ");
}

#[test]
fn invalid_config_does_not_apply_partial_preferences() {
    for input in [
        "probe:value\tchanged\ninvalid-row",
        "# cuemol-config-v2\nprobe:value\tchanged\nprobe:bad\t\\x",
        "# cuemol-config-v2\nprobe:value\tchanged\nprobe:bad\t\\",
        "probe:value\tchanged\n\tmissing-key",
    ] {
        let mut state = DesktopAppState::new();
        state.put_config_value("probe:value", "original");
        let before = state.config_entries();
        assert!(state.load_config_text(input).is_err());
        assert_eq!(state.config_entries(), before);
    }
}
