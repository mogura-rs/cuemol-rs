const HTML: &str = include_str!("../tauri-ui/index.html");
const CSS: &str = include_str!("../tauri-ui/styles.css");
const JS: &str = include_str!("../tauri-ui/app.js");

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
    assert_eq!(HTML.matches("data-ribbon-panel=").count(), 3);
    assert!(HTML.contains("data-ribbon-select=\"animation\""));
    assert!(CSS.contains("--viewport: #000"));
    assert!(CSS.contains("grid-template-columns: var(--panel-width) 6px minmax(0, 1fr)"));
    assert!(!HTML.contains("NAVIGATOR"));
    assert!(!HTML.contains("INSPECTOR"));
}

#[test]
fn tauri_reference_panels_are_bound_to_shared_core_data() {
    for id in [
        "molstruct-tree",
        "coloring-type",
        "symmetry-content",
        "density-content",
        "sequence-view",
        "measure-readout",
    ] {
        assert!(HTML.contains(&format!("id=\"{id}\"")), "missing panel {id}");
    }
    for command in [
        "set_object_coloring",
        "set_display_option",
        "set_density_level",
        "set_background",
        "set_selection",
    ] {
        assert!(JS.contains(command), "missing IPC binding {command}");
    }
}

#[test]
fn tauri_main_menu_matches_the_reference_inventory() {
    for menu in [
        "File",
        "Edit",
        "Rendering",
        "Scene",
        "View",
        "Tools",
        "Window",
        "Help",
    ] {
        assert!(
            HTML.contains(&format!("<summary>{menu}</summary>")),
            "missing menu {menu}"
        );
    }
    for item in [
        "POV-Ray rendering",
        "Animation rendering",
        "Molecular superposition",
        "Mol bond editor",
        "Interaction",
        "Mol surface generation",
        "APBS elepot calculation",
        "Restore default panel location",
        "Check for updates",
    ] {
        assert!(HTML.contains(item), "missing menu item {item}");
    }
    for command in [
        "undo",
        "redo",
        "clear_undo",
        "delete_atoms",
        "change_chain",
        "renumber_residues",
        "pick_atom",
        "select_screen_rect",
        "add_animation_object",
        "animation_control",
        "tick_animation",
    ] {
        assert!(JS.contains(command), "missing menu binding {command}");
    }
    assert!(HTML.contains("data-interaction=\"rotate\""));
    assert!(HTML.contains("data-interaction=\"rect-select\""));
}
