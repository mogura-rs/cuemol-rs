const HTML: &str = include_str!("../tauri-ui/index.html");
const CSS: &str = include_str!("../tauri-ui/styles.css");

#[test]
fn tauri_shell_matches_the_reference_xul_dock_order() {
    let menu = HTML.find("class=\"application-menu\"").unwrap();
    let ribbon = HTML.find("class=\"ribbon\"").unwrap();
    let content = HTML.find("class=\"content-holder\"").unwrap();
    let status = HTML.find("class=\"statusbar\"").unwrap();
    assert!(menu < ribbon && ribbon < content && content < status);

    for surface in [
        "Workspace",
        "MolStruct",
        "Selection",
        "Coloring",
        "Symmetry",
        "Density map",
        "View",
        "Animation",
        "Output",
        "Sequence",
    ] {
        assert!(
            HTML.contains(surface),
            "missing reference GUI surface {surface}"
        );
    }
}

#[test]
fn tauri_shell_uses_the_native_light_chrome_and_cuemol_viewport() {
    assert!(HTML.contains("name=\"color-scheme\" content=\"light\""));
    assert_eq!(HTML.matches("class=\"side-card open\"").count(), 5);
    assert_eq!(HTML.matches("data-ribbon-panel=").count(), 2);
    assert!(!HTML.contains("data-ribbon=\"animation\""));
    assert!(CSS.contains("--viewport: #000"));
    assert!(CSS.contains("grid-template-columns: var(--panel-width) 6px minmax(0, 1fr)"));
    assert!(!HTML.contains("NAVIGATOR"));
    assert!(!HTML.contains("INSPECTOR"));
}
