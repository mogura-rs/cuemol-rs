use cuemol_qsys::StyleManager;

#[test]
fn current_overlay_preserves_base_fallback_and_scope_precedence() {
    let mut styles = StyleManager::new();
    styles.load_builtin_default_styles(0);
    let base_detail = styles.style_attr(0, "DefaultRibbon", "axialdetail");
    let base_type = styles.style_child_attr(0, "DefaultRibbon", "helix", "type");
    let uid = styles.load_builtin_cuemol3_styles(0);
    assert_ne!(uid, 0);
    assert_eq!(styles.load_builtin_cuemol3_styles(0), 0);
    let set = styles.get_style_set(uid).unwrap();
    assert!(set.read_only());
    assert!(!set.modified());
    assert_eq!(
        styles.get_str_data("palette", "default_paint", 42),
        "okabeito"
    );
    assert_eq!(
        styles
            .style_child_attr(42, "DefaultRibbon", "helix", "width")
            .as_deref(),
        Some("0.18")
    );
    assert_eq!(
        styles
            .style_child_attr(42, "DefaultRibbon", "helix", "tuber")
            .as_deref(),
        Some("6.5")
    );
    assert_eq!(
        styles
            .style_child_attr(42, "DefaultRibbon", "coil", "width")
            .as_deref(),
        Some("0.2")
    );
    assert_eq!(
        styles.style_child_attr(42, "DefaultRibbon", "helix", "type"),
        base_type
    );
    assert_eq!(
        styles.style_attr(42, "DefaultRibbon", "axialdetail"),
        base_detail
    );
    assert_eq!(
        styles.style_attr(42, "DefaultNucl", "base_size").as_deref(),
        Some("0.3")
    );
    assert_eq!(
        styles
            .style_child_attr(42, "DefaultNucl", "section", "type")
            .as_deref(),
        Some("roundsquare")
    );
    styles.load_style_set_from_xml(
        0,
        "user",
        r#"<styles><style id="DefaultRibbon"><helix width="0.25"/></style></styles>"#,
        false,
    );
    assert_eq!(
        styles
            .style_child_attr(42, "DefaultRibbon", "helix", "width")
            .as_deref(),
        Some("0.25")
    );
    styles.load_style_set_from_xml(
        42,
        "local",
        r#"<styles><style id="DefaultRibbon"><helix width="0.4"/></style></styles>"#,
        false,
    );
    assert_eq!(
        styles
            .style_child_attr(42, "DefaultRibbon", "helix", "width")
            .as_deref(),
        Some("0.4")
    );
    assert_eq!(
        styles
            .style_child_attr(42, "DefaultRibbon", "helix", "tuber")
            .as_deref(),
        Some("6.5")
    );
}
