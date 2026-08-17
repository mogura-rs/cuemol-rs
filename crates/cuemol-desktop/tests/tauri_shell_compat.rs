const HTML: &str = include_str!("../tauri-ui/index.html");
const CSS: &str = include_str!("../tauri-ui/styles.css");
const JS: &str = include_str!("../tauri-ui/app.js");
const CAPABILITIES: &str = include_str!("../capabilities/default.json");
const NATIVE_GUI_RS: &str = include_str!("../src/gui.rs");
const TAURI_APP_RS: &str = include_str!("../src/tauri_app.rs");
const DESKTOP_LIB_RS: &str = include_str!("../src/lib.rs");

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
    assert!(HTML.contains("aria-label=\"Resize bottom panel\""));
    assert!(CSS.contains("var(--bottom-panel-height, 148px)"));
    assert!(JS.contains("function setupBottomPanelSplitter()"));
    assert!(JS.contains("BOTTOM_PANEL_HEIGHT_STORAGE_KEY"));
    assert!(NATIVE_GUI_RS.contains("bottom_panel:height"));
    assert!(NATIVE_GUI_RS.contains("bottom_panel:visible"));
    assert!(NATIVE_GUI_RS.contains("bottom_panel:tab"));
    assert!(NATIVE_GUI_RS.contains("Collapse bottom panel"));
    assert!(NATIVE_GUI_RS.contains("Expand bottom panel"));
    assert!(JS.contains("startHeight + startY - moveEvent.clientY"));
    assert!(JS.contains("expandedBottomPanelHeight"));
    assert!(JS.contains("bottomPanelHeight === 0 ? expandedBottomPanelHeight"));
    assert!(CSS.contains(".main-view.bottom-panel-collapsed .bottom-panel"));
    assert!(HTML.contains("aria-label=\"Resize side panel\""));
    assert!(JS.contains("function setupSidePanelSplitter()"));
    assert!(JS.contains("SIDE_PANEL_WIDTH_STORAGE_KEY"));
    assert!(JS.contains("startWidth + moveEvent.clientX - startX"));
    assert!(CSS.contains(".content-holder.side-panel-collapsed .side-panel"));
    assert!(JS.contains("RIBBON_COLLAPSED_STORAGE_KEY"));
    assert!(JS.contains("function setRibbonCollapsed(collapsed, persist = false)"));
    assert!(JS.contains("setRibbonCollapsed(false, true)"));
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
        "selection-object",
        "selection-history",
        "selection-editor-list",
        "selection-editor-add",
        "selection-editor-delete",
        "selection-editor-clear",
        "coloring-target",
        "coloring-type",
        "coloring-deck",
        "symmetry-content",
        "density-content",
        "sequence-view",
        "measure-readout",
        "trajectory-slider",
        "trajectory-block-strip",
        "trajectory-block-add",
        "trajectory-block-remove",
    ] {
        assert!(HTML.contains(&format!("id=\"{id}\"")), "missing panel {id}");
    }
    for command in [
        "set_coloring",
        "set_display_option",
        "set_density_renderer_properties",
        "redraw_density_renderer",
        "show_unit_cell",
        "set_background",
        "molecule_selection_action",
        "sequence_action",
        "get_molecule_details",
        "open_initial_paths",
        "open_scene",
    ] {
        assert!(JS.contains(command), "missing IPC binding {command}");
    }
    for id in [
        "molstruct-select",
        "molstruct-center",
        "molstruct-zoom",
        "molstruct-properties",
    ] {
        assert!(HTML.contains(&format!("id=\"{id}\"")), "missing {id}");
    }
    for label in ["Select atoms", "Center at", "Zoom at", "Properties ..."] {
        assert!(
            NATIVE_GUI_RS.contains(label),
            "missing native MolStruct action {label}"
        );
    }
    for label in [
        "Paint coloring",
        "Solid coloring",
        "CPK coloring",
        "Bfac/Occ coloring",
        "Rainbow coloring",
        "Electrostatic potential",
        "Multi-gradient coloring",
        "Reset to default style",
    ] {
        assert!(
            NATIVE_GUI_RS.contains(label),
            "missing native Coloring action {label}"
        );
    }
    for label in [
        "Use sigma contour level",
        "Use absolute contour level",
        "Solid coloring",
        "Multi gradient",
        "Redraw",
        "Cell",
        "Transp:",
        "Level:",
        "Extent:",
    ] {
        assert!(
            NATIVE_GUI_RS.contains(label),
            "missing native Density map control {label}"
        );
    }
    for label in [
        "Rotation",
        "RotX:",
        "RotY:",
        "RotZ:",
        "Translation",
        "TraX:",
        "TraY:",
        "TraZ:",
        "Zoom / Slab",
        "Zoom:",
        "Slab:",
        "Dist:",
        "Projection",
        "Perspective:",
        "Center mark:",
        "Crosshair",
    ] {
        assert!(
            NATIVE_GUI_RS.contains(label),
            "missing native View fake-dial control {label}"
        );
    }
    assert!(!NATIVE_GUI_RS.contains(".button(surface_label)"));
    assert!(!NATIVE_GUI_RS.contains(".button(clip_label)"));
    assert!(!CSS.contains("#selection-editor-add-kind"));
    assert!(JS.contains("selectedMolstructExpression()"));
    assert!(JS.contains("event.shiftKey"));
    assert!(JS.contains("composeSelectionEditorExpression()"));
    assert!(JS.contains("selectionActiveTab = element.dataset.selectionTab"));
    assert!(HTML.contains("data-selection-tab=\"command\" aria-label=\"Input command\""));
    assert!(HTML.contains("data-selection-tab=\"editor\" aria-label=\"Editor\""));
    assert!(JS.contains("byres ((${selection}) around ${item.args})"));
    assert!(JS.contains("await applySelectionCommand()"));
    assert!(!HTML.contains("id=\"selection-editor-add-kind\""));
    assert!(HTML.contains("id=\"selection-editor-add\" type=\"button\" aria-label=\"Add selection item\" aria-haspopup=\"menu\""));
    assert!(JS.contains("selectionEditorAdd.addEventListener(\"click\""));
    assert!(JS.contains("label: \"Around/Expand\""));
    assert!(JS.contains("selectionHistoryEntries.map((entry) => ({"));
    assert!(JS.contains("const MAX_SELECTION_HISTORY = 10"));
    assert!(JS.contains("localStorage.setItem(SELECTION_HISTORY_STORAGE_KEY"));
    assert!(JS.contains("[{ label: \"(no history)\", disabled: true }]"));
    assert!(NATIVE_GUI_RS.contains("enum NativeSelectionEditorItem"));
    assert!(NATIVE_GUI_RS.contains("fn compose_native_selection_editor("));
    assert!(NATIVE_GUI_RS.contains("\"Around/Expand\""));
    assert!(NATIVE_GUI_RS.contains("egui::Button::new(\"(no history)\")"));
    assert!(NATIVE_GUI_RS.contains("const MAX_SELECTION_HISTORY: usize = 10"));
    assert!(NATIVE_GUI_RS.contains("fn load_selection_history("));
    assert!(NATIVE_GUI_RS.contains("fn persist_selection_history("));
    assert!(NATIVE_GUI_RS.contains("selection_history:{index}"));
    assert!(!NATIVE_GUI_RS.contains("ui.label(\"Whole molecule\")"));
    assert!(JS.contains("item.supportsColoring"));
    assert!(JS.contains("PaintColoring"));
    assert!(JS.contains("CPKColoring"));
    assert!(JS.contains("RainbowColoring"));
    assert!(JS.contains("BfacColoring"));
    assert!(JS.contains(r#"extensions: ["dcd", "xtc", "trr", "nc"]"#));
}

#[test]
fn tauri_densitymap_panel_matches_reference_renderer_controls() {
    for label in [
        "Use sigma contour level",
        "Use absolute contour level",
        "Solid coloring",
        "Multi gradient",
        "Redraw",
        "Cell",
        "Edit color...",
        "Transp:",
        "Level:",
        "Extent:",
    ] {
        assert!(
            JS.contains(label),
            "missing Tauri Density map control {label}"
        );
    }
    for command in [
        "set_density_renderer_properties",
        "redraw_density_renderer",
        "show_unit_cell",
    ] {
        assert!(
            JS.contains(command),
            "missing Density map command {command}"
        );
        assert!(TAURI_APP_RS.contains(command));
    }
    assert!(JS.contains("[\"contour\", \"isosurf\", \"gpu_mapmesh\", \"gpu_mapvol\"]"));
    assert!(JS.contains("renderer.specific.use_abslevel"));
    assert!(JS.contains("renderer.specific.maxExtent"));
    assert!(CSS.contains(".density-target-row"));
    assert!(CSS.contains(".density-options-popup"));
    assert!(CSS.contains(".density-range"));
    assert!(!HTML.contains("id=\"show-density-contour\""));
    assert!(!HTML.contains("id=\"density-level\""));
}

#[test]
fn tauri_view_panel_matches_reference_fakedials() {
    assert!(HTML.contains("id=\"view-content\""));
    for label in [
        "Rotation",
        "Rot${axis}:",
        "Translation",
        "Tra${axis}:",
        "Zoom / Slab",
        "Zoom:",
        "Slab:",
        "Dist:",
        "Perspective:",
        "Center mark:",
        "Crosshair",
    ] {
        assert!(JS.contains(label), "missing Tauri View control {label}");
    }
    for action in [
        "rotate-x",
        "rotate-y",
        "rotate-z",
        "translate-x",
        "translate-y",
        "translate-z",
        "set-center-x",
        "adjust-zoom",
        "set-slab",
        "set-distance",
    ] {
        assert!(JS.contains(action), "missing View action {action}");
        assert!(TAURI_APP_RS.contains(action));
    }
    assert!(JS.contains("function viewDialWheel("));
    assert!(JS.contains("wheel.setPointerCapture(event.pointerId)"));
    assert!(JS.contains("viewPanelCamera = {"));
    assert!(TAURI_APP_RS.contains("center: [f64; 3]"));
    assert!(TAURI_APP_RS.contains("zoom: self.camera.zoom()"));
    assert!(CSS.contains(".view-dial-row"));
    assert!(CSS.contains("cursor: ew-resize"));
    let view_body = HTML
        .split("<div class=\"side-body view-body\">")
        .nth(1)
        .and_then(|source| source.split("</section>").next())
        .expect("View panel body");
    assert!(!view_body.contains("Background"));
    assert!(!view_body.contains("Fit view"));
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
        "reload_scene",
        "delete_atoms",
        "change_chain",
        "renumber_residues",
        "pick_atom",
        "pick_atom_identifier",
        "select_screen_rect",
        "add_animation_object",
        "animation_control",
        "tick_animation",
        "tick_animation_frame",
        "superpose_objects",
        "edit_bond",
        "get_nonstandard_bonds",
        "remove_nonstandard_bonds",
        "calculate_contacts",
        "assign_secondary_structure",
        "create_molecular_surface",
        "cut_surface",
        "start_apbs_calculation",
        "poll_apbs_calculation",
        "cancel_apbs_calculation",
        "start_pov_render",
        "poll_pov_render",
        "cancel_pov_render",
        "save_pov_render_image",
        "clear_pov_render_result",
        "append_morph_frame",
        "remove_morph_frame",
        "apply_morph_frame",
        "clear_log",
        "execute_script",
        "execute_script_file",
        "export_pov",
        "start_animation_render",
        "start_animation_movie",
        "poll_animation_render",
        "cancel_animation_render",
        "animation_render_preview",
        "open_pdb",
        "merge_molecules",
        "save_object",
        "save_view",
        "load_view",
        "load_trajectory",
        "set_trajectory_frame",
        "set_trajectory_frame_viewport",
        "append_trajectory_block",
        "remove_trajectory_block",
        "move_trajectory_block",
    ] {
        assert!(JS.contains(command), "missing menu binding {command}");
    }
    assert!(JS.contains("title: \"Select a File\""));
    assert!(JS.contains("name: \"Internal Javascript (*.js)\""));
    assert!(!JS.contains("dialogTextArea(\n      \"JavaScript\""));
    assert!(HTML.contains("data-interaction=\"rotate\""));
    assert!(HTML.contains("data-interaction=\"rect-select\""));
    assert!(JS.contains("bondList.className = \"bond-list\""));
    assert!(JS.contains("button(\"Delete selected\""));
    assert!(JS.contains("JSON.parse(json)"));
    assert!(JS.contains("atomPickerTarget = { input, object }"));
    assert!(JS.contains("referenceDialog.show()"));
    assert!(CSS.contains(".bond-list-row"));
    assert!(CSS.contains(".viewport.atom-picking"));
}

#[test]
fn tauri_window_menu_toggles_reference_panels_and_lists_windows() {
    assert!(HTML.contains("<span>Panels</span>"));
    assert!(HTML.contains("<span>Windows</span>"));
    assert_eq!(HTML.matches("data-panel-menu=").count(), 8);
    assert!(HTML.contains("id=\"windows-menu\""));
    assert!(CSS.contains(".window-menu-popup"));
    assert!(CSS.contains(".application-menu {"));
    assert!(CSS.contains("z-index: 40"));
    assert!(JS.contains("card.hidden = !card.hidden"));
    assert!(JS.contains("function closeSidePanelCard(card)"));
    assert!(JS.contains("event.stopPropagation()"));
    assert!(JS.contains("sidePanel.hidden = !anyVisible"));
    assert!(CSS.contains(".content-holder.no-visible-side-panels"));
    assert!(JS.contains("SIDE_PANEL_STATE_STORAGE_KEY"));
    assert!(JS.contains("function saveSidePanelState()"));
    assert!(JS.contains("function restoreSidePanelState()"));
    assert!(JS.contains("function setupSidePanelReordering()"));
    assert!(JS.contains("SIDE_PANEL_DEFAULT_ORDER"));
    assert!(JS.contains("applySidePanelOrder(SIDE_PANEL_DEFAULT_ORDER)"));
    assert!(JS.contains("state.__order.push(name)"));
    assert!(JS.contains("height: Number(card.dataset.panelHeight) || null"));
    assert!(JS.contains("sidePanel.insertBefore(sourceCard"));
    assert!(JS.contains("function setupSidePanelHeightSplitters()"));
    assert!(JS.contains("setSidePanelCardHeight(card, null)"));
    assert!(CSS.contains(".side-card.panel-drop-before::before"));
    assert!(CSS.contains(".side-panel-splitter"));
    assert!(CSS.contains("height: var(--side-panel-height, auto)"));
    assert!(JS.contains("for (const name of openPanels) renderSidePanel(name)"));
    assert!(NATIVE_GUI_RS.contains("left_panel:{}:visible"));
    assert!(NATIVE_GUI_RS.contains("left_panel:{}:collapsed"));
    assert!(NATIVE_GUI_RS.contains("left_panel:order"));
    assert!(NATIVE_GUI_RS.contains("left_panel:width"));
    assert!(NATIVE_GUI_RS.contains("left_panel:{}:height"));
    assert!(NATIVE_GUI_RS.contains("left-panel-drag"));
    assert!(NATIVE_GUI_RS.contains("fn left_panel_resize_handle"));
    assert!(NATIVE_GUI_RS.contains(".small_button(\"×\")"));
    assert!(NATIVE_GUI_RS.contains("Close panel"));
    assert!(JS.contains("getAllWebviewWindows"));
    assert!(JS.contains("await appWindow.setFocus()"));
    assert!(JS.contains("WINDOW_GEOMETRY_STORAGE_KEY"));
    assert!(JS.contains("function setupWindowGeometryPersistence()"));
    assert!(JS.contains("await appWindow.setSize("));
    assert!(JS.contains("await appWindow.setPosition("));
    assert!(JS.contains("await persistWindowGeometry(appWindow)"));
    assert!(NATIVE_GUI_RS.contains("fn load_native_window_geometry"));
    assert!(NATIVE_GUI_RS.contains("fn record_native_window_geometry"));
    assert!(NATIVE_GUI_RS.contains("WindowEvent::Moved(_)"));
    assert!(CAPABILITIES.contains(r#""cuemol-*""#));
    assert!(CAPABILITIES.contains("core:webview:allow-create-webview-window"));
    assert!(CAPABILITIES.contains("core:window:allow-set-position"));
    assert!(CAPABILITIES.contains("core:window:allow-set-size"));
}

#[test]
fn tauri_open_recent_matches_the_reference_mru_contract() {
    assert!(HTML.contains("id=\"recent-menu\""));
    assert!(HTML.contains("aria-label=\"Open Recent\""));
    assert!(!HTML.contains("disabled>Open recent</button>"));
    assert!(CSS.contains(".menu-submenu:hover > .nested-menu"));
    assert!(JS.contains("const MAX_RECENT_FILES = 10"));
    assert!(JS.contains("entry.path !== path"));
    assert!(JS.contains("recentFiles.unshift({ path, ftype })"));
    assert!(JS.contains("entry.ftype === \"script\""));
    assert!(JS.contains("addRecentFile(path, \"script\")"));
    assert!(JS.contains("localStorage.setItem(RECENT_STORAGE_KEY"));
    assert!(JS.contains("button(\"Clear Menu\""));
    assert!(JS.contains("await openSelectedPaths([entry.path])"));
    assert!(JS.contains("multiple: false"));
    assert!(JS.contains("await invoke(\"get_file_renderer_setup\", { path })"));
    assert!(JS.contains("const setup = await showRendererSetup(defaults)"));
    assert!(JS.contains("readerOptions: setup.readerOptions"));
    assert!(JS.contains("mutateWithViewport(\"open_path_with_renderer_setup\""));
    assert!(JS.contains("mutateWithViewport(\"open_paths\", { paths: directPaths })"));
    assert!(JS.contains("snapshot?.openedPaths"));
    assert!(NATIVE_GUI_RS.contains("const MAX_RECENT_PATHS: usize = 10"));
    assert!(NATIVE_GUI_RS.contains("fn load_recent_paths("));
    assert!(NATIVE_GUI_RS.contains("fn persist_recent_paths("));
    assert!(NATIVE_GUI_RS.contains("fn is_script_file("));
    assert!(NATIVE_GUI_RS.contains("self.add_recent_path(path)"));
    assert!(NATIVE_GUI_RS.contains("mru:{index}:path"));
    assert!(NATIVE_GUI_RS.contains(".filter(|path| path.is_file())"));
    assert!(NATIVE_GUI_RS.contains("egui::Button::new(\"(none)\")"));
}

#[test]
fn tauri_md_trajectory_open_defers_io_until_renderer_setup() {
    let start = JS
        .find("} else if (title === \"Open MD Trajectory\")")
        .unwrap();
    let end = JS[start + 1..]
        .find("} else if (title === \"Open PDB\")")
        .map(|offset| start + 1 + offset)
        .unwrap();
    let dialog = &JS[start..end];

    assert!(dialog.contains("multiple: true"));
    assert!(dialog.contains("const chosenTrajectories = [...trajectoryPaths]"));
    assert!(dialog.contains("await invoke(\"get_file_renderer_setup\""));
    assert!(dialog.contains("const setup = await showRendererSetup(defaults)"));
    assert!(dialog.contains("rendererSetup: setup.rendererSetup"));
    assert!(dialog.contains("await mutate(\"load_trajectory\""));
    assert!(!dialog.contains("Initial renderer"));
    assert!(
        dialog.find("showRendererSetup(defaults)").unwrap()
            < dialog.find("mutate(\"load_trajectory\"").unwrap()
    );
}

#[test]
fn tauri_file_open_matches_reference_molecule_reader_options() {
    for label in [
        "PDB options",
        "mmCIF options",
        "Ignore multilpe models",
        "Ignore anisotropic U",
        "Ignore alternate conformation",
        "Load SEGID as chain name",
        "Calculate protein secondary structure",
        "Auto-generate non-standard topology",
    ] {
        assert!(JS.contains(label), "missing molecule reader option {label}");
    }
    for binding in [
        "ignoreMultipleModels: readerControls.ignoreMultipleModels.checked",
        "ignoreAnisou: readerControls.ignoreAnisou.checked",
        "ignoreAltConf: readerControls.ignoreAltConf.checked",
        "calculateSecondaryStructure:",
        "autoGenerateTopology: readerControls.autoGenerateTopology.checked",
    ] {
        assert!(
            JS.contains(binding),
            "missing molecule reader binding {binding}"
        );
    }
}

#[test]
fn tauri_file_save_uses_reference_workspace_writer_filters() {
    let start = JS.find("} else if (title === \"Save File As\")").unwrap();
    let end = JS[start + 1..]
        .find("} else if (title === \"Save Current View\")")
        .map(|offset| start + 1 + offset)
        .unwrap();
    let dialog = &JS[start..end];

    for writer in [
        "PDB coordinates (*.pdb)",
        "MOL/SDF Coordinates (*.mol;*.sdf)",
        "CueMol coordinates file (*.qdf)",
        "XYZR coordinates(*.xyzr)",
        "PQR coordinates(*.pqr)",
        "CueMol density map file (*.qdf)",
        "CueMol elepot file (*.qdf)",
        "CueMol molsurf file (*.qdf)",
        "CueMol light-weight obj file (*.qdf)",
    ] {
        assert!(
            JS.contains(&format!("name: \"{writer}\"")),
            "missing object writer {writer}"
        );
    }
    assert!(dialog.contains("selectedWorkspaceObjectForSave()"));
    assert!(dialog.contains("filters: objectSaveFilters(object)"));
    assert!(JS.contains("object.className === \"ElePotMap\""));
    assert!(dialog.contains("objectKind: object.objectKind"));
    assert!(dialog.contains("saveOptions.defaultPath = objectSaveDefaultPath(object)"));
    assert!(JS.contains("return directory + \"copy_of_\" + fileName"));
    assert!(JS.contains("!source.startsWith(\"datachunk:\")"));
    assert!(dialog.contains("await mutateWithoutViewport("));
    assert!(dialog.contains("\"save_object\""));
    assert!(TAURI_APP_RS.contains("save_workspace_object_as(kind"));
    assert!(TAURI_APP_RS.contains("GuiWorkspaceObjectKind::Lightweight"));
}

#[test]
fn tauri_save_current_view_uses_reference_camera_filter() {
    let start = JS
        .find("} else if (title === \"Save Current View\")")
        .unwrap();
    let end = JS[start + 1..]
        .find("} else if (title === \"Merge molecule\")")
        .map(|offset| start + 1 + offset)
        .unwrap();
    let dialog = &JS[start..end];

    assert!(dialog.contains("\"current_view.cam\""));
    assert!(dialog.contains("\"Camera setting (*.cam)\""));
    assert!(dialog.contains("[\"cam\"]"));
    assert!(!dialog.contains("\"view\""));
}

#[test]
fn tauri_workspace_exposes_lightweight_objects_and_renderers() {
    for binding in [
        "lwObjects",
        "lwObject",
        "lwRenderer",
        "selectedLwObject",
        "selectedLwRenderer",
        "set_lw_object_properties",
        "set_lw_renderer_properties",
        "dataId",
        "copyWorkspaceRenderer(object.uid, renderer.uid)",
        "pasteWorkspaceRenderers(object.uid)",
        "Lightweight renderer properties",
        "[\"Common\", \"Lightweight\"]",
    ] {
        assert!(
            JS.contains(binding),
            "missing lightweight binding {binding}"
        );
    }
    for binding in [
        "lw_objects: Vec<LwObjectDto>",
        "SetLwObjectProperties",
        "SetLwRendererProperties",
        "AddLwRenderer",
        "set_lw_renderer_properties",
        "destroy_lw_object",
        "destroy_lw_renderer",
    ] {
        assert!(
            TAURI_APP_RS.contains(binding),
            "missing lightweight backend {binding}"
        );
    }
}

#[test]
fn tauri_output_prompt_executes_javascript_with_reference_history() {
    assert!(HTML.contains("id=\"output-command\""));
    assert!(HTML.contains("Js&gt;"));
    assert!(HTML.contains("aria-label=\"JavaScript command\""));
    assert!(JS.contains("COMMAND_HISTORY_STORAGE_KEY"));
    assert!(JS.contains("MAX_COMMAND_HISTORY = 100"));
    assert!(JS.contains("function executeOutputCommand()"));
    assert!(JS.contains("mutate(\"execute_script\", { script })"));
    assert!(JS.contains("event.key === \"Enter\""));
    assert!(JS.contains("event.key === \"ArrowUp\""));
    assert!(JS.contains("event.key === \"ArrowDown\""));
    assert!(TAURI_APP_RS.contains("ModelRequest::ExecuteScript(script)"));
    assert!(TAURI_APP_RS.contains("model.log_lines.push(format!(\"> {script}\"))"));
    assert!(CSS.contains("grid-template-columns: 24px 1fr"));
    for binding in [
        "ui.monospace(\"Js>\")",
        "fn execute_console_input(",
        "fn recall_command_history(",
        "cmd_prompt_history:length",
        "egui::Key::ArrowUp",
        "egui::Key::ArrowDown",
    ] {
        assert!(
            NATIVE_GUI_RS.contains(binding),
            "missing native Output prompt binding {binding}"
        );
    }
    let native_bottom = NATIVE_GUI_RS
        .split("fn bottom_panel(")
        .nth(1)
        .and_then(|source| source.split("fn output_tab(").next())
        .expect("native bottom dock");
    assert!(
        native_bottom.find("self.output_tab(ui)").unwrap()
            < native_bottom.find("BottomTab::Output, \"Output\"").unwrap(),
        "native bottom tabs must follow the content"
    );
    let native_output = NATIVE_GUI_RS
        .split("fn output_tab(")
        .nth(1)
        .and_then(|source| source.split("fn load_command_history(").next())
        .expect("native Output panel");
    assert!(!native_output.contains("button(\"Clear\")"));
    assert!(!native_output.contains("button(\"Save As\")"));
}

#[test]
fn tauri_sequence_panel_matches_reference_mouse_and_context_operations() {
    for binding in [
        "function runSequenceAction(",
        "sequence_action",
        "range-toggle",
        "range-select",
        "toggle-center",
        "setSequenceMarker(cell)",
        "previewSequenceRange(sequenceDrag.start, cell)",
        "navigator.clipboard.writeText(sequence)",
        "function createSequenceRuler(columnCount)",
        "function sequenceColumnCount(chains)",
        "sequenceResidueColumn(residue.residueIndex)",
        "SEQUENCE_NAME_WIDTH_STORAGE_KEY",
        "setSequenceNameWidth(sequenceNameWidth, true)",
        "splitter.setPointerCapture(event.pointerId)",
    ] {
        assert!(JS.contains(binding), "missing Sequence binding {binding}");
    }
    for label in [
        "Center here",
        "Toggle sel",
        "Around Byresid",
        "Around",
        "Unselect all",
        "Invert sel",
        "Copy sequence",
    ] {
        assert!(JS.contains(label), "missing Sequence action {label}");
    }
    assert!(JS.contains("event.shiftKey && sequenceMarker"));
    assert!(JS.contains("residue.residueIndex"));
    assert!(CSS.contains(".sequence-cell.marked"));
    assert!(CSS.contains(".sequence-cell.range-preview"));
    assert!(CSS.contains(".sequence-ruler"));
    assert!(CSS.contains("var(--sequence-name-width, 140px) max-content"));
    assert!(CSS.contains(".sequence-name-splitter"));
    assert!(CSS.contains("width: 10px;"));
    assert!(CSS.contains("position: sticky;"));
    assert!(TAURI_APP_RS.contains("fn apply_sequence_action("));
    assert!(TAURI_APP_RS.contains("molecule_residue_range_selection_text"));
    for binding in [
        "NativeSequenceAction",
        "native_sequence_range_action",
        "response.context_menu",
        "sequence_position_in_drag_range",
    ] {
        assert!(
            NATIVE_GUI_RS.contains(binding),
            "missing native Sequence binding {binding}"
        );
    }
    let native_sequence = NATIVE_GUI_RS
        .split("fn sequence_tab(")
        .nth(1)
        .and_then(|source| source.split("fn trajectory_tab(").next())
        .expect("native Sequence panel");
    assert!(native_sequence.contains("egui::ScrollArea::both()"));
    assert!(native_sequence.contains("NATIVE_SEQUENCE_RULER_HEIGHT"));
    assert!(native_sequence.contains("egui::vec2(name_width, NATIVE_SEQUENCE_ROW_HEIGHT)"));
    assert!(native_sequence.contains("sequence_residue_column"));
    assert!(native_sequence.contains("native_sequence_ruler"));
    assert!(native_sequence.contains("native_sequence_names"));
    assert!(native_sequence.contains("native_sequence_grid"));
    assert!(native_sequence.contains("scroll_offset(scroll_offset)"));
    assert!(native_sequence.contains("splitter.drag_delta().x"));
    assert!(native_sequence.contains("sequence_panel:name_width"));
    assert!(!native_sequence.contains("horizontal_wrapped"));
}

#[test]
fn paint_coloring_columns_are_resizable_and_persistent() {
    for binding in [
        "COLORING_PAINT_COLUMN_WIDTH_STORAGE_KEY",
        "function coloringPaintColumnHeader(primaryLabel = \"Selection\")",
        "setColoringPaintSelectionColumnWidth(",
        "resetColoringPaintSelectionColumnWidth",
        "splitter.setPointerCapture(event.pointerId)",
        "colorCell.append(swatch, color)",
    ] {
        assert!(
            JS.contains(binding),
            "missing Paint column binding {binding}"
        );
    }
    assert!(CSS.contains("var(--coloring-paint-selection-width, minmax(60px, 1fr))"));
    assert!(CSS.contains(".coloring-paint-column-splitter"));
    assert!(CSS.contains(".coloring-paint-color-cell"));
    assert!(JS.contains("list.append(coloringPaintColumnHeader(\"Param\"))"));
    assert_eq!(JS.matches("colorCell.append(swatch, color)").count(), 2);
    let native_paint = NATIVE_GUI_RS
        .split("fn object_paint_coloring_ui(")
        .nth(1)
        .and_then(|source| source.split("fn electrostatic_coloring_ui(").next())
        .expect("native Paint coloring panel");
    assert!(native_paint.contains("RichText::new(\"Selection\").strong()"));
    assert!(native_paint.contains("RichText::new(\"Color\").strong()"));
    assert!(native_paint.contains("coloring_paint:selection_width"));
    assert!(native_paint.contains("splitter.drag_delta().x"));
    assert!(native_paint.contains("ui.monospace(color)"));
    let native_gradient = NATIVE_GUI_RS
        .split("fn multi_gradient_coloring_ui(")
        .nth(1)
        .and_then(|source| source.split("fn default_coloring_details(").next())
        .expect("native Multi-gradient coloring panel");
    assert!(native_gradient.contains("RichText::new(\"Param\").strong()"));
    assert!(native_gradient.contains("RichText::new(\"Color\").strong()"));
    assert!(native_gradient.contains("coloring_gradient:param_width"));
    assert!(native_gradient.contains("splitter.drag_delta().x"));
    assert!(native_gradient.contains("ui.monospace(color)"));
}

#[test]
fn tauri_file_and_scene_open_use_reference_reader_categories() {
    assert_eq!(HTML.matches("data-action=\"open-scene\"").count(), 2);

    let object_open = JS
        .split("async function openFiles()")
        .nth(1)
        .and_then(|source| source.split("async function openScene()").next())
        .expect("object file picker");
    for reader in [
        "PDB Coordinates (*.pdb;*.ent;*.pdb.gz)",
        "mmCIF Coordinates (*.cif;*.cif.gz)",
        "MOL/SDF Coordinates (*.mol;*.sdf)",
        "SYBYL Mol2 Coordinates (*.mol2)",
        "GROMACS Coordinates (*.gro)",
        "CHARMM/NAMD PSF topology (*.psf)",
        "AMBER topology file (*.parm7;*.prmtop)",
        "NAMD binary coordinates (*.coor)",
        "DCD binary trajectory (*.dcd)",
        "GROMACS TRR trajectory (*.trr)",
        "GROMACS XTC trajectory (*.xtc)",
        "AMBER NetCDF trajectory (*.nc)",
        "APBS Potential Map(*.dx)",
        "mmCIF Map Coeff (*.cif;*.cif.gz)",
        "CCP4 Density Map(*.map;*.ccp4;*.mrc;*.ccp4.gz)",
        "XPLOR/CNS Density Map(*.map;*.cns)",
        "BRIX Density Map(*.brix;*.omap)",
        "MTZ Structure Factor (*.mtz)",
        "MSMS Surface (*.vert;*.face)",
        "PLY polygon file (*.ply)",
    ] {
        assert!(
            object_open.contains(&format!("name: \"{reader}\"")),
            "missing object reader {reader}"
        );
    }
    for hidden_extension in [
        "qdfpdb", "qdf", "mmcif", "top", "obj", "view", "cam", "qsc", "pse",
    ] {
        assert!(
            !object_open.contains(&format!("\"{hidden_extension}\"")),
            "hidden extension leaked into object picker: {hidden_extension}"
        );
    }
    assert!(JS.contains("showReferenceDialog(\"Open MD Trajectory\", trajectoryPaths)"));

    let scene_open = JS
        .split("async function openScene()")
        .nth(1)
        .and_then(|source| source.split("async function openSelectedPaths").next())
        .expect("scene file picker");
    assert!(scene_open.contains("title: \"Open Scene\""));
    assert!(scene_open.contains("name: \"CueMol Scene\""));
    assert!(scene_open.contains("extensions: [\"qsc\"]"));
    assert!(scene_open.contains("name: \"PyMOL Session\""));
    assert!(scene_open.contains("extensions: [\"pse\"]"));
    assert!(scene_open.contains("await openSelectedPaths(paths)"));
    assert!(JS.contains("mutateWithViewport(\"open_scene\", { path })"));
    assert!(JS.contains("/\\.(qsc|pse)$/i.test(path)"));
    assert!(TAURI_APP_RS.contains("ModelRequest::OpenSceneViewport"));
    assert!(TAURI_APP_RS.contains("fn open_scene("));
}

#[test]
fn tauri_file_open_matches_reference_namd_coor_options() {
    for label in [
        "NAMD Coor file options",
        "PSF file:",
        "Change ...",
        "PSF file",
    ] {
        assert!(JS.contains(label), "missing NAMD Coor option {label}");
    }
    for binding in [
        "const namdDefaults = setup.namdCoorOptions || null",
        "namdCoorOptions: setup.namdCoorOptions",
        "topologyPath: namdControls.topologyPath.value.trim()",
        "extensions: [\"psf\"]",
    ] {
        assert!(JS.contains(binding), "missing NAMD Coor binding {binding}");
    }
}

#[test]
fn tauri_file_open_matches_reference_msms_surface_options() {
    for binding in [
        "const msmsDefaults = setup.msmsOptions || null",
        "MSMS surface file options",
        "Vertex file:",
        "msmsOptions: setup.msmsOptions",
        "vertexPath: msmsControls.vertexPath.value.trim()",
        "extensions: [\"vert\"]",
    ] {
        assert!(JS.contains(binding), "missing MSMS binding: {binding}");
    }
}

#[test]
fn tauri_file_open_matches_reference_ccp4_map_options() {
    for label in [
        "CCP4Map options",
        "Set map center",
        "Move view center",
        "Normalize by mean and stdev",
        "Truncate density lower than:",
        "Truncate density higher than:",
    ] {
        assert!(JS.contains(label), "missing CCP4 map option {label}");
    }
    for binding in [
        "setup.targetKind === \"densityMap\"",
        "targetKind: setup.targetKind",
        "ccp4MapOptions: setup.ccp4MapOptions",
        "normalize: ccp4Controls.normalize.checked",
        "truncateMin: ccp4Controls.truncateMin.enabled.checked",
        "truncateMax: ccp4Controls.truncateMax.enabled.checked",
    ] {
        assert!(JS.contains(binding), "missing CCP4 map binding {binding}");
    }
}

#[test]
fn tauri_file_open_matches_reference_mtz_map_options() {
    for label in [
        "MTZ options",
        "Amplitude (F):",
        "Phase:",
        "Weight:",
        "Max resolution (Å):",
        "Grid spacing:",
        "Fine (0.25)",
        "Coarse (0.33)",
    ] {
        assert!(JS.contains(label), "missing MTZ map option {label}");
    }
    for binding in [
        "const mtzDefaults = setup.mtzMapOptions || null",
        "mtzMapOptions: setup.mtzMapOptions",
        "columnF: mtzControls.amplitude.value",
        "usePhase: mtzControls.phase.enabled.checked",
        "useWeight: mtzControls.weight.enabled.checked",
        "gridSize: Number(mtzControls.gridSize.value)",
    ] {
        assert!(JS.contains(binding), "missing MTZ map binding {binding}");
    }
}

#[test]
fn tauri_reload_scene_uses_the_active_scene_source() {
    assert!(HTML.contains("data-local-action=\"reload-scene\""));
    assert!(JS.contains("scene?.modified"));
    assert!(JS.contains("await showConfirmReloadSceneDialog"));
    assert!(JS.contains("has unsaved changes. Reload from disk and discard changes?"));
    assert!(JS.contains("dialogTitle.textContent = \"Reload Scene\""));
    assert!(CSS.contains(".reference-dialog.confirm-reload-dialog"));
    assert!(NATIVE_GUI_RS.contains("fn reload_scene_dialog"));
    assert!(NATIVE_GUI_RS.contains("fn reload_current_path_unchecked"));
    assert!(JS.contains("await mutate(\"reload_scene\")"));
    assert!(!JS.contains("lastOpenedPaths"));
}

#[test]
fn tauri_save_and_save_as_use_distinct_reference_flows() {
    assert!(HTML.contains("data-action=\"save\">Save Scene</button>"));
    assert!(HTML.contains("data-action=\"save-as\">Save Scene As…</button>"));
    assert!(JS.contains("snapshot?.scenes.find((scene) => scene.active)?.source"));
    assert!(JS.contains("await saveScene(true)"));
    assert!(JS.contains("saveScene(event.shiftKey)"));
    assert!(JS.contains("mutateWithoutViewport(\"save_scene\""));
    assert!(JS.contains("function showSceneSaveOptions()"));
    for label in [
        "Scene options",
        "Embed possible",
        "Compatibility",
        "Ver 2.2 or later",
        "Ver 2.3 or later",
        "Compression",
        "Enable text encoding",
    ] {
        assert!(JS.contains(label), "missing scene option {label}");
    }
    assert!(JS.contains("const options = await showSceneSaveOptions()"));
    assert!(JS.contains("compress: advanced ? compress.value : \"none\""));
    assert!(JS.contains("base64: advanced && base64.checked"));
    assert!(CSS.contains(".reference-dialog.scene-options-dialog"));
    assert!(NATIVE_GUI_RS.contains("fn scene_save_options_dialog"));
    assert!(TAURI_APP_RS.contains("save_active_scene_qsc_with_options"));
}

#[test]
fn tauri_animation_frames_use_one_compact_ipc_round_trip() {
    assert!(JS.contains("invoke(\"tick_animation_frame\", { dt: 0.1, ...size })"));
    assert!(JS.contains("applyViewport(frame.viewport, sequence)"));
    assert!(!JS.contains("snapshot = await invoke(\"tick_animation\""));
}

#[test]
fn tauri_startup_uses_a_compact_critical_ipc_and_lazy_molecule_details() {
    assert!(JS.contains(r#"invoke("get_initial_state", viewportDimensions())"#));
    assert!(JS.contains("snapshot = initial.snapshot"));
    assert!(JS.contains("applyViewport(initial.viewport, sequence)"));
    assert!(JS.contains("if (snapshotHasRenderableContent()) void renderViewport()"));
    assert!(JS.contains(r#"invoke("get_molecule_details", {"#));
    assert!(JS.contains("const moleculeStructureCache = new Map()"));
    assert!(JS.contains("structureObjectUid: knownStructureObjectUid"));
    assert!(JS.contains("objectUid,"));
    assert!(JS.contains("cachedMoleculeStructure(objectUid)"));
    assert!(JS.contains("scheduleMoleculeDetailsRefresh(delay = 0, objectUid = null)"));
    assert!(JS.contains("scheduleMoleculeDetailsRefresh()"));
    assert!(JS.contains("includeSequence"));
    assert!(JS.contains("initial.startupPathsPending"));
    assert!(JS.contains("function scheduleSecondaryPanelRender()"));
    assert!(JS.contains("globalThis.requestIdleCallback(render, { timeout: 100 })"));
    assert!(JS.contains("function sidePanelIsExpanded(name)"));
    assert!(JS.contains("if (sidePanelIsExpanded(\"MolStruct\")) visibleObjectUids.push"));
    assert!(JS.contains("if (sidePanelIsExpanded(\"Selection\")) visibleObjectUids.push"));
    let render_shell = JS
        .split("function renderShell()")
        .nth(1)
        .and_then(|source| source.split("function renderUndoControls").next())
        .expect("renderShell function");
    assert!(render_shell.contains("scheduleSecondaryPanelRender()"));
    assert!(!render_shell.contains("renderColoring()"));
    assert!(!render_shell.contains("renderTrajectory()"));
    let secondary_render = JS
        .split("function scheduleSecondaryPanelRender()")
        .nth(1)
        .and_then(|source| source.split("function renderShell").next())
        .expect("secondary panel render function");
    assert!(secondary_render.contains("if (sidePanelIsExpanded(\"Symmetry\")) renderSymmetry()"));
    assert!(secondary_render.contains("[data-bottom-panel=\"trajectory\"]"));
    assert!(secondary_render.contains("[data-ribbon-panel=\"animation\"]"));
    assert!(JS.contains("void mutateWithViewport(\"open_initial_paths\")"));
    assert!(JS.contains("mutateWithViewport(\"open_paths\""));
    assert!(JS.contains("mutateWithViewport(\"open_pdb\""));
    assert!(JS.contains("pdbId.maxLength = 4"));
    assert!(JS.contains("/^[0-9][0-9A-Z]{3}$/"));
    assert!(JS.contains("applyViewport(update.viewport, sequence)"));
    assert!(JS.contains("if (!update.viewport.image && snapshotHasRenderableContent())"));
    assert!(JS.contains(
        "if (!update.viewport.image && snapshotHasRenderableContent()) void renderViewport()"
    ));
    assert!(JS.contains("if (chainNode.dataset.loaded) return"));
    assert!(JS.contains("if (residueNode.dataset.loaded) return"));
    assert!(JS.contains("if (chainNode.open) loadResidues()"));
    assert!(JS.contains("if (residueNode.open) loadAtoms()"));
    assert!(!JS.contains("snapshot.structure.map"));
    assert!(!JS.contains("snapshot.structure.find"));
    assert!(!JS.contains("snapshot.sequence.length"));
    assert!(!JS.contains("for (const chain of snapshot.sequence)"));
    assert!(!JS.contains(r#"snapshot = await invoke("get_snapshot");\n    renderShell();"#));
}

#[test]
fn desktop_startup_and_fetch_overlap_independent_initialization() {
    assert!(NATIVE_GUI_RS.contains("app: Option<GuiApplication>"));
    assert!(NATIVE_GUI_RS.contains("app: None"));
    let gpu_start = NATIVE_GUI_RS
        .find("self.gpu_init_receiver = Some(receiver)")
        .unwrap();
    let config_start = NATIVE_GUI_RS.find("cuemol-config-init").unwrap();
    let app_start = NATIVE_GUI_RS
        .find("self.app = Some(GuiApplication::new_with_config_source(config))")
        .unwrap();
    assert!(gpu_start < config_start);
    assert!(config_start < app_start);
    assert!(NATIVE_GUI_RS.contains("fs::read_to_string(&path)"));
    assert!(NATIVE_GUI_RS.contains("poll_config_init(event_loop)"));

    let tauri_config_start = TAURI_APP_RS.find("cuemol-tauri-config-init").unwrap();
    let tauri_desktop_start = TAURI_APP_RS
        .find("let mut desktop = DesktopAppState::new();")
        .unwrap();
    let tauri_config_join = TAURI_APP_RS
        .find("let config_source = if let Some((path, loader))")
        .unwrap();
    assert!(tauri_config_start < tauri_desktop_start);
    assert!(tauri_desktop_start < tauri_config_join);
    assert!(TAURI_APP_RS.contains("desktop.load_config_text(&input)"));
    assert!(!TAURI_APP_RS.contains("desktop.load_config_file(path)"));

    assert!(TAURI_APP_RS.contains("ModelRequest::StartRendererWarmup"));
    assert!(TAURI_APP_RS.contains("request_model(&state, ModelRequest::StartRendererWarmup)"));
    assert!(!TAURI_APP_RS.contains("spawn_blocking(shared_scene_renderer)"));
    assert!(JS.contains(r#"invoke("prefetch_pdb", args)"#));
    assert!(TAURI_APP_RS.contains("async fn prefetch_pdb("));
    assert!(NATIVE_GUI_RS.contains("cuemol-pdb-prefetch-"));

    let initial_paths = TAURI_APP_RS
        .split("fn open_initial_paths(")
        .nth(1)
        .and_then(|source| source.split("fn reload_scene(").next())
        .expect("open_initial_paths command");
    let warmup = initial_paths
        .find("ModelRequest::StartRendererWarmup")
        .expect("startup renderer warmup");
    let load = initial_paths
        .find("ModelRequest::OpenPathsViewport")
        .expect("startup path load");
    assert!(warmup < load);
}

#[test]
fn tauri_camera_interaction_uses_reduced_preview_pixels() {
    assert!(JS.contains("const CAMERA_PREVIEW_SCALE = 0.625"));
    assert!(JS.contains("renderScale: CAMERA_PREVIEW_SCALE"));
    assert!(JS.contains("scheduleRender(CAMERA_SETTLE_RENDER_DELAY)"));
    assert!(TAURI_APP_RS.contains("scaled_viewport_dimensions(width, height, render_scale)"));
}

#[test]
fn new_tab_dialog_matches_scene_and_inherited_view_flow() {
    for label in [
        "New Tab/Window",
        "Create new tab for:",
        "New Scene",
        "New View for ${currentSceneName}",
        "New View (no current scene)",
        "Inherit view props",
    ] {
        assert!(JS.contains(label), "missing new-tab behavior {label}");
    }
    assert!(JS.contains(r#"mutate("new_tab", result)"#));
    assert!(JS.contains(r#"mutate("activate_view", { viewUid: view.uid })"#));
    assert!(JS.contains("for (const view of snapshot.views || [])"));
    assert!(JS.contains("let defaultSceneName = `Untitled ${sceneIndex}`"));
    assert!(JS.contains("const label = `${scene.name}:${view.name}`"));
    assert!(CSS.contains(".reference-dialog.new-tab-dialog"));
    assert!(NATIVE_GUI_RS.contains("fn new_tab_dialog"));
    assert!(NATIVE_GUI_RS.contains("NativeNewTabMode::View"));
    assert!(TAURI_APP_RS.contains("ModelRequest::NewTab"));
    assert!(TAURI_APP_RS.contains("ModelRequest::ActivateView"));
}

#[test]
fn close_tab_matches_last_modified_view_confirmation_flow() {
    for label in [
        "Unsaved Changes",
        "is not saved. Save changes?",
        "Don't Save",
        "Save",
        "Cancel",
    ] {
        assert!(JS.contains(label), "missing close-tab behavior {label}");
    }
    assert!(JS.contains("scene.modified && viewCount === 1"));
    assert!(JS.contains("await showConfirmCloseTabDialog"));
    assert!(JS.contains("if (result === \"save\" && !(await saveScene()))"));
    assert!(JS.contains("await closeActiveTab()"));
    assert!(JS.contains("async function confirmApplicationExit()"));
    assert!(JS.contains("async function setupApplicationCloseGuard()"));
    assert!(JS.contains("appWindow.onCloseRequested"));
    assert!(JS.contains("event.preventDefault()"));
    assert!(JS.contains("applicationCloseAllowed = true"));
    assert!(JS.contains("await appWindow.close()"));
    assert!(CSS.contains(".reference-dialog.confirm-close-dialog"));
    assert!(NATIVE_GUI_RS.contains("fn close_tab_dialog"));
    assert!(NATIVE_GUI_RS.contains("close_after_scene_save"));
    assert!(NATIVE_GUI_RS.contains("fn request_application_exit"));
    assert!(NATIVE_GUI_RS.contains("fn application_exit_dialog"));
    assert!(NATIVE_GUI_RS.contains("Save All"));
    assert!(NATIVE_GUI_RS.contains("app.exit_ready"));
    assert!(NATIVE_GUI_RS.contains("app.persist_config_state()"));
    assert!(TAURI_APP_RS.contains(".close_active_view()"));
}

#[test]
fn atom_interaction_context_menu_uses_the_dedicated_list_editor() {
    for label in [
        "Edit interaction list:",
        "Interaction definitions",
        "(no interactions)",
        "Distance",
        "Angle",
        "Torsion",
    ] {
        assert!(
            JS.contains(label),
            "missing interaction-list behavior {label}"
        );
    }
    assert!(JS.contains("showEditInteractionListDialog(object, renderer)"));
    assert!(JS.contains("get_atom_intr_defs"));
    assert!(JS.contains("remove_atom_intr_defs"));
    assert!(CSS.contains(".reference-dialog.interaction-list-dialog"));
    assert!(NATIVE_GUI_RS.contains("fn atom_intr_edit_dialog"));
    assert!(NATIVE_GUI_RS.contains("remove_atom_intr_defs"));
    assert!(TAURI_APP_RS.contains("ModelRequest::GetAtomIntrDefs"));
    assert!(TAURI_APP_RS.contains("ModelRequest::RemoveAtomIntrDefs"));
}

#[test]
fn renderer_edit_style_uses_the_ordered_style_list_dialog() {
    for label in [
        "Apply Renderer Style",
        "Renderer: ",
        "Styles:",
        "(low priority)",
        "(high priority)",
        "(no styles applied)",
        "＋ Add",
        "Delete",
        "Up",
        "Down",
    ] {
        assert!(
            JS.contains(label),
            "missing renderer style editor label {label}"
        );
    }
    assert!(JS.contains("showApplyRendererStyleDialog(object, renderer, styleOptions)"));
    assert!(JS.contains("mutate(\"set_workspace_renderer_styles\""));
    assert!(JS.contains("styles.splice(insertAt, 0, name)"));
    assert!(CSS.contains(".reference-dialog.renderer-style-list-dialog"));
    assert!(NATIVE_GUI_RS.contains("fn renderer_style_edit_dialog"));
    assert!(NATIVE_GUI_RS.contains("open_renderer_style_edit(scene_uid, object_uid, renderer.uid)"));
    assert!(TAURI_APP_RS.contains("ModelRequest::SetWorkspaceRendererStyles"));
    assert!(TAURI_APP_RS.contains("fn set_workspace_renderer_styles("));
}

#[test]
fn style_set_editor_uses_live_structured_tabs() {
    for label in [
        "Style editor: ${properties.name}",
        "Color",
        "Selection",
        "Styles",
        "Named colors",
        "Named selections",
        "Style entries",
        "(no named colors)",
        "(no named selections)",
        "(no style entries)",
        "(read-only style set)",
        "New color name",
        "New selection name",
    ] {
        assert!(
            JS.contains(label),
            "missing structured style editor behavior {label}"
        );
    }
    assert!(JS.contains("new XMLSerializer().serializeToString(documentNode)"));
    assert!(JS.contains("snapshot = await invoke(\"set_style_properties\""));
    assert!(CSS.contains(".reference-dialog.style-set-editor-dialog"));
    assert!(!JS.contains("xmlTitle.textContent = \"Style XML\""));
    assert!(NATIVE_GUI_RS.contains("enum NativeStyleEditorTab"));
    assert!(NATIVE_GUI_RS.contains("fn native_style_set_contents"));
    assert!(NATIVE_GUI_RS.contains("fn apply_native_style_mutation"));
    assert!(NATIVE_GUI_RS.contains("(read-only style set)"));
}

#[test]
fn error_alert_requires_explicit_acknowledgement() {
    for id in [
        "id=\"error-alert-dialog\"",
        "id=\"error-alert-title\"",
        "id=\"error-alert-message\"",
    ] {
        assert!(HTML.contains(id), "missing error alert element {id}");
    }
    assert!(HTML.contains("<button type=\"submit\" autofocus>OK</button>"));
    assert!(JS.contains("errorAlertDialog.showModal()"));
    assert!(JS.contains("errorAlertMessage.textContent = message"));
    assert!(!JS.contains("toastTimer"));
    assert!(CSS.contains(".reference-dialog.error-alert-dialog"));
    assert!(NATIVE_GUI_RS.contains("fn error_alert_dialog"));
    assert!(NATIVE_GUI_RS.contains("self.error_alert = Some(message.clone())"));
}

#[test]
fn pdb_download_uses_cancellable_stream_progress() {
    for id in [
        "id=\"stream-progress-dialog\"",
        "id=\"stream-progress-title\"",
        "id=\"stream-progress-bar\"",
        "id=\"stream-progress-readout\"",
        "id=\"stream-progress-cancel\"",
    ] {
        assert!(HTML.contains(id), "missing stream progress element {id}");
    }
    assert!(JS.contains("waitForPdbDownload(`Downloading PDB ${id}`"));
    assert!(JS.contains("invoke(\"get_pdb_download_progress\", args)"));
    assert!(JS.contains("invoke(\"cancel_pdb_download\", args)"));
    assert!(JS.contains("streamProgressReadout.textContent = \"Canceling…\""));
    assert!(CSS.contains(".reference-dialog.stream-progress-dialog"));
    assert!(TAURI_APP_RS.contains("fn get_pdb_download_progress("));
    assert!(TAURI_APP_RS.contains("fn cancel_pdb_download("));
    assert!(DESKTOP_LIB_RS.contains("bytes_received: std::sync::atomic::AtomicU64"));
    assert!(DESKTOP_LIB_RS.contains("cancelled: std::sync::atomic::AtomicBool"));
    assert!(NATIVE_GUI_RS.contains("fn pdb_download_progress_dialog("));
}

#[test]
fn png_export_matches_the_reference_options_dialog() {
    for label in [
        "PNG options",
        "Resolution (DPI)",
        "Retain aspect ratio",
        "Transparent PNG",
        "Pixel size:",
    ] {
        assert!(JS.contains(label), "missing PNG option {label}");
    }
    for value in ["[72, 150, 300, 600]", "resolutionDpi", "transparent"] {
        assert!(JS.contains(value), "missing PNG option binding {value}");
    }
    assert_eq!(JS.matches("await showPngExportOptions(").count(), 1);
    assert!(CSS.contains(".reference-dialog.png-options-dialog"));
    assert!(CSS.contains("width: min(360px, calc(100vw - 48px))"));
    assert!(TAURI_APP_RS.contains("export_active_scene_png_with_options"));
    assert!(TAURI_APP_RS.contains("GuiPngExportOptions"));
}

#[test]
fn animation_render_uses_cancellable_progress_and_reference_frame_controls() {
    let start = JS
        .find("} else if (title === \"Animation rendering\")")
        .unwrap();
    let end = JS[start + 1..]
        .find("} else if (title === \"Save File As\")")
        .map(|offset| start + 1 + offset)
        .unwrap();
    let dialog = &JS[start..end];

    for label in [
        "Target scene",
        "Output dir",
        "Output base name",
        "Width (px)",
        "Height (px)",
        "Frame rate (fps)",
        "NCPU",
        "Loop",
        "Projection",
        "Main options",
        "Render options",
        "Movie options",
        "Preview",
        "Preset size",
        "Backend",
        "Povray exe file",
        "Povray inc dir",
        "Clipping plane",
        "Post-render alpha blending",
        "Shadow",
        "Edge lines",
        "Labels",
        "Radiosity",
        "Make movie",
        "FFmpeg exe file",
        "Output format",
        "Bitrate (kbps)",
        "Re-encode",
        "Progress",
        "Log",
    ] {
        assert!(
            dialog.contains(label),
            "missing animation render field {label}"
        );
    }
    for command in [
        "start_animation_render",
        "start_animation_movie",
        "poll_animation_render",
        "cancel_animation_render",
        "animation_render_preview",
    ] {
        assert!(
            dialog.contains(command),
            "missing animation render command {command}"
        );
    }
    assert!(dialog.contains("duplicateLastFrame: loop.checked"));
    assert!(dialog.contains("concurrency: Number(concurrency.value)"));
    assert!(dialog.contains("backend: backend.value"));
    assert!(dialog.contains("makeMovie: makeMovie.checked"));
    assert!(dialog.contains("setInterval(poll, 150)"));
    assert!(dialog.contains("if (running) event.preventDefault()"));
    assert!(TAURI_APP_RS.contains("struct TauriAnimationRenderJob"));
    assert!(TAURI_APP_RS.contains("restore: AppModelUndo"));
    assert!(TAURI_APP_RS.contains("spawn_tauri_animation_frame(task)"));
    assert!(TAURI_APP_RS.contains("spawn_tauri_animation_movie(task)"));
    assert!(TAURI_APP_RS.contains("run_pov_process_with_poll"));
    assert!(TAURI_APP_RS.contains("_frm_%04d.png"));
    assert!(NATIVE_GUI_RS.contains("fn load_animation_render_settings("));
    assert!(NATIVE_GUI_RS.contains("fn persist_animation_render_settings("));
    for key in [
        "animation_render:height",
        "animation_render:concurrency",
        "animation_render:duplicate_last_frame",
        "animation_render:pov_clip_plane",
        "animation_render:pov_post_blend",
        "animation_render:pov_shadow",
        "animation_render:pov_edge_lines",
        "animation_render:pov_labels",
        "animation_render:make_movie",
    ] {
        assert!(NATIVE_GUI_RS.contains(key), "missing native setting {key}");
    }
}

#[test]
fn animation_panel_uses_scene_backed_timeline_and_start_camera() {
    assert!(HTML.contains("id=\"animation-start-camera\""));
    assert!(JS.contains("snapshot.animationStartCamera || \"\""));
    assert!(JS.contains("renderReadonlyAnimationTime(animationDuration, total)"));
    assert!(JS.contains("function renderReadonlyAnimationTime(host, seconds)"));
    assert!(HTML.contains("<option value=\"\">(none)</option>"));
    assert!(JS.contains("mutate(\"set_animation_start_camera\""));
    assert!(TAURI_APP_RS.contains("fn set_animation_start_camera("));
    assert!(TAURI_APP_RS.contains("scene_animation(scene_uid)"));
    assert!(TAURI_APP_RS.contains("set_scene_animation(scene_uid, Some(animation))"));
    assert!(TAURI_APP_RS.contains("animation.motions()"));
    assert!(!TAURI_APP_RS.contains("animation_objects: Vec<AnimObject>"));
    assert!(JS.contains("workspacePropertyPage([\"Common\"])"));
    assert!(JS.contains("dialogGroup(\"Common settings\", commonPage)"));
    assert!(JS.contains("commonGroup.classList.add(\"animation-common-settings\")"));
    assert!(CSS.contains(".animation-camera-settings"));
    assert!(CSS.contains("grid-template-columns: repeat(2, minmax(0, 1fr))"));
    assert!(JS.contains("targetAlpha.step = \"0.1\""));
    assert!(NATIVE_GUI_RS.contains("ui.selectable_label(true, \"Common\")"));
    assert!(NATIVE_GUI_RS.contains("animation_camera_ignore_flags"));
    assert!(JS.contains("function dialogTimeField(label, seconds"));
    assert!(JS.contains("const increments = [3600, 60, 1, 0.1]"));
    assert!(CSS.contains(".animation-time-editor"));
    assert!(NATIVE_GUI_RS.contains("fn native_animation_time_ui("));
    assert!(NATIVE_GUI_RS.contains("native_animation_time_parts(*value)"));
    assert!(NATIVE_GUI_RS.contains("format_native_animation_time_editor(animation.length())"));
    assert!(!JS.contains("workspacePropertyPage([\"Common settings\", object.typeName])"));
    for label in [
        "Relative to",
        "Start time",
        "Duration",
        "Quadric",
        "Rotation angle",
        "Spin axis X",
        "Target camera",
        "Target renderers",
        "Show/Hide",
        "Target opacity",
        "Direction angle",
        "Direction preset",
        "Distance (W/2)",
        "Target MorphMol",
        "Start value",
        "End value",
    ] {
        assert!(JS.contains(label), "missing animation property {label}");
    }
    assert!(JS.contains("mutate(\"set_animation_object_properties\""));
    assert!(JS.contains("button(\"Reset all to default\""));
    assert!(JS.contains("function defaultAnimationObjectProperties(object)"));
    assert!(JS.contains("function newAnimationObjectDraft(kind, index)"));
    assert!(HTML.contains("id=\"animation-add\""));
    assert!(HTML.contains("aria-haspopup=\"menu\""));
    assert!(!HTML.contains("id=\"animation-kind\""));
    assert!(!JS.contains("const animationKind"));
    assert!(JS.contains("showWorkspaceContextMenu(event, animationAddKinds.map"));
    assert!(JS.contains("pendingAnimationAdd = {"));
    assert!(JS.contains("await showReferenceDialog(\"Animation object properties\")"));
    assert!(JS.contains("message: \"Reset all to default?\""));
    assert!(JS.contains("mutate(\"reset_animation_object_properties\""));
    assert!(HTML.contains("id=\"confirmation-dialog\""));
    assert!(JS.contains("Object.assign(input, { angle: 0, axis: [0, 1, 0] })"));
    assert!(JS.contains("String(object.quadric * 100)"));
    assert!(JS.contains("quadric: Number(quadric.value) / 100"));
    assert!(JS.contains("angle.max = \"360\""));
    assert!(JS.contains("distance.max = \"2\""));
    for label in ["X axis", "Y axis", "Z axis", "Cartesian"] {
        assert!(JS.contains(label));
        assert!(NATIVE_GUI_RS.contains(label));
    }
    for label in ["Left", "Up", "Right", "Down"] {
        assert!(JS.contains(&format!("\"{label}\"")));
        assert!(NATIVE_GUI_RS.contains(&format!("\"{label}\"")));
    }
    assert!(NATIVE_GUI_RS.contains("animation_slide_direction"));
    assert!(JS.contains("keepQuaternionPositive: true"));
    assert!(!JS.contains("Keep quaternion positive"));
    assert!(!JS.contains(
        "dialogField(\n          \"Property\",\n          object.propertyName || \"frame\""
    ));
    assert!(!NATIVE_GUI_RS.contains("\"Keep quaternion positive\","));
    assert!(JS.contains("fade: true"));
    assert!(JS.contains("distance: 1"));
    assert!(JS.contains("function formatAnimationObjectTime(seconds)"));
    assert!(JS.contains("function parseAnimationObjectTime(value)"));
    assert!(JS.contains("`${item.name} (${item.typeName})`"));
    assert!(JS.contains("[\"\", \"(absolute)\"]"));
    assert!(JS.contains("afterIndex: selectedAnimationIndex"));
    assert!(JS.contains("row.addEventListener(\"dblclick\""));
    assert!(JS.contains("let selectedAnimationIndices = new Set()"));
    assert!(JS.contains("event.shiftKey && selectedAnimationAnchor !== null"));
    assert!(JS.contains("mutate(\"delete_animation_objects\""));
    assert!(JS.contains("mutate(\"move_animation_objects\""));
    assert!(JS.contains("function animationContextItems()"));
    assert!(JS.contains("row.addEventListener(\"contextmenu\""));
    for label in ["Change...", "Delete", "Add...", "Move up", "Move down"] {
        assert!(JS.contains(&format!("label: \"{label}\"")));
    }
    assert!(TAURI_APP_RS.contains("fn set_animation_object_properties("));
    assert!(TAURI_APP_RS.contains("fn reset_animation_object_properties("));
    assert!(TAURI_APP_RS.contains("ResetAnimationObjectProperties"));
    assert!(TAURI_APP_RS.contains("AddConfiguredAnimationObject"));
    assert!(TAURI_APP_RS.contains("animation.insert_motion_before(insertion_index, motion)"));
    assert!(TAURI_APP_RS.contains("fn delete_animation_objects("));
    assert!(TAURI_APP_RS.contains("fn move_animation_objects("));
    assert!(TAURI_APP_RS.contains("animation.edit_motion_by_uid"));
    assert!(TAURI_APP_RS.contains("motion.base_mut().set_time_ref_name(name.clone())"));
    assert!(NATIVE_GUI_RS.contains("animation_selection: std::collections::BTreeSet<usize>"));
    assert!(NATIVE_GUI_RS.contains("fn delete_animation_motions("));
    assert!(NATIVE_GUI_RS.contains("fn move_animation_motions("));
    assert!(NATIVE_GUI_RS.contains("fn open_new_animation_properties("));
    assert!(NATIVE_GUI_RS.contains("draft.reset_defaultable()"));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Reset all to default?\")"));
    assert!(NATIVE_GUI_RS.contains("ui.button(\"Reset all to default\")"));
    assert!(NATIVE_GUI_RS.contains("response.context_menu(|ui|"));
    assert!(NATIVE_GUI_RS.contains("RichText::new(\"Name\").strong()"));
    assert!(NATIVE_GUI_RS.contains("RichText::new(\"Start\").strong()"));
    assert!(NATIVE_GUI_RS.contains("RichText::new(\"End\").strong()"));
    assert!(HTML.contains("data-animation-column-splitter=\"name\""));
    assert!(HTML.contains("data-animation-column-splitter=\"start\""));
    assert!(JS.contains("ANIMATION_COLUMN_WIDTHS_STORAGE_KEY"));
    assert!(JS.contains("function setupAnimationColumnSplitters()"));
    assert!(JS.contains("setAnimationColumnWidths(current.nameWidth"));
    assert!(CSS.contains("var(--animation-name-width, minmax(45px, 2fr))"));
    assert!(CSS.contains(".animation-column-splitter"));
    assert!(NATIVE_GUI_RS.contains("animation_panel:name_width"));
    assert!(NATIVE_GUI_RS.contains("animation_panel:start_width"));
    assert!(NATIVE_GUI_RS.contains("name_splitter.drag_delta().x"));
    assert!(NATIVE_GUI_RS.contains("start_splitter.drag_delta().x"));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Duration:\")"));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Start cam:\")"));
    assert!(
        NATIVE_GUI_RS.contains("ui.selectable_value(&mut start_camera, String::new(), \"(none)\")")
    );
    assert!(NATIVE_GUI_RS.contains("format!(\"{name} ({})\", motion.type_name())"));
}

#[test]
fn pov_render_matches_the_reference_options_and_cancellable_flow() {
    for label in [
        "Main options",
        "Preset size",
        "Same as view",
        "1200x1200@600dpi",
        "Projection",
        "Stereo depth",
        "Transp background",
        "Clipping plane",
        "Post-render alpha blending",
        "Edge lines",
        "Pix labels",
        "POV-Ray options",
        "Povray-exe-path",
        "Povray-inc-path",
        "Radiosity",
        "Use default settings",
        "Save image…",
        "Copy to clipboard",
        "Unzoom",
        "const zoomLevels = [10, 25, 50, 75, 100, 125, 150, 200, 300]",
        "`${scene.uid},${view.uid}`",
        "sceneUid",
        "viewUid",
        "Rendered image is not saved. Save image?",
        "Don't Save",
    ] {
        assert!(JS.contains(label), "missing POV-Ray option {label}");
    }
    for command in [
        "start_pov_render",
        "poll_pov_render",
        "cancel_pov_render",
        "save_pov_render_image",
        "clear_pov_render_result",
    ] {
        assert!(JS.contains(command), "missing POV-Ray flow {command}");
        assert!(
            TAURI_APP_RS.contains(command),
            "missing POV-Ray command {command}"
        );
    }
    assert!(NATIVE_GUI_RS.contains("fn start_pov_render"));
    assert!(NATIVE_GUI_RS.contains("fn poll_pov_render"));
    assert!(NATIVE_GUI_RS.contains("fn persist_pov_render_settings"));
    assert!(DESKTOP_LIB_RS.contains("run_pov_process"));
    assert!(TAURI_APP_RS.contains("overlay_pov_labels(&output.output_path, &job.labels)"));
    assert!(CSS.contains(".pov-preview-scroll"));
}

#[test]
fn merge_molecule_dialog_matches_the_reference_copy_move_flow() {
    for value in [
        "let mergeMoleculeSourceUid = null",
        "let mergeMoleculeTargetUid = null",
        "Copy (keep source atoms)",
        "Source and destination must be different molecules.",
        "merge.disabled",
        "rememberSelection(selection.value)",
    ] {
        assert!(
            JS.contains(value),
            "missing merge molecule behavior {value}"
        );
    }
    assert!(JS.contains("dialogCheckbox(\"Copy (keep source atoms)\", true, options)"));
    assert!(NATIVE_GUI_RS.contains("fn merge_molecule_dialog"));
    assert!(NATIVE_GUI_RS.contains("apply_merge_molecule_draft"));
    assert!(!NATIVE_GUI_RS.contains("NativeFileDialog::MergeObject"));
}

#[test]
fn tauri_viewport_render_requests_are_coalesced() {
    assert!(JS.contains("if (viewportRenderRequest)"));
    assert!(JS.contains("viewportRenderQueued = true"));
    assert!(JS.contains("do {"));
    assert!(JS.contains("} while (viewportRenderQueued)"));
    assert!(JS.contains("await viewportRenderRequest"));
    assert!(JS.contains("if (refreshViewport) void renderViewport();"));
    assert!(!JS.contains("if (refreshViewport) await renderViewport();"));
}

#[test]
fn tauri_viewport_selection_uses_one_compact_ipc_round_trip() {
    assert!(JS.contains("mutateWithViewport(\"pick_atom\""));
    assert!(JS.contains("mutateWithViewport(\"select_screen_rect\""));
    assert!(!JS.contains("mutate(\"pick_atom\""));
    assert!(!JS.contains("mutate(\"select_screen_rect\""));
}

#[test]
fn tauri_camera_updates_coalesce_pending_input() {
    assert!(JS.contains("const cameraQueue = []"));
    assert!(JS.contains("function enqueueCameraCommand(action, x, y)"));
    assert!(JS.contains("pending.x += x"));
    assert!(JS.contains("pending.x *= x"));
    assert!(JS.contains("while (cameraQueue.length)"));
    assert!(JS.contains("cameraRequest = flushCameraCommands()"));
}

#[test]
fn tauri_interaction_dialog_matches_reference_modes() {
    for label in [
        "Selection 1",
        "Mol 2:",
        "Molecule 2",
        "Sel 2:",
        "Selection 2",
        "Min distance (Å)",
        "Max distance (Å)",
        "Max labels",
        "Hydrogen bond (N, O) only",
        "Renderer",
    ] {
        assert!(JS.contains(label), "missing Interaction field {label}");
    }
    assert!(JS.contains("object2Uid: useObject2.checked"));
    assert!(JS.contains("selection2: useSelection2.checked"));
    assert!(JS.contains("rendererName: renderer.value"));
    assert!(JS.contains("item.rendererType === \"atomintr\""));
}

#[test]
fn tauri_symmetry_editor_is_lazy_and_matches_reference_fields() {
    for label in [
        "Change ...",
        "Symm mol",
        "20 Å",
        "50 Å",
        "100 Å",
        "200 Å",
        "Unit cell",
        "Crystal system",
        "Space Group",
        "Space Group Number",
        "Restrict by symmetry",
        "a (Å)",
        "α (°)",
    ] {
        assert!(JS.contains(label), "missing symmetry editor field {label}");
    }
    assert!(JS.contains("const spaceGroupOptionsCache = new Map()"));
    assert!(JS.contains("await invoke(\"get_space_groups\""));
    assert!(JS.contains("spaceGroupOptionsCache.set(lattice.value, groups)"));
    assert!(JS.contains("mutate(\"change_crystal_info\""));
    assert!(JS.contains("spaceGroupNumber: Number(spaceGroup.value)"));
    assert!(JS.contains("function symmetryTargets()"));
    assert!(JS.contains("target?.kind !== \"molecule\""));
    assert!(JS.contains("mutate(\"show_symmetry_molecules\""));
    assert!(JS.contains("mutate(\"show_unit_cell\""));
    assert!(JS.contains("The selected molecule has no crystal information. Create it?"));
    assert!(TAURI_APP_RS.contains("ModelRequest::ShowSymmetryMolecules"));
    assert!(TAURI_APP_RS.contains("ModelRequest::ShowUnitCell"));
    assert!(!HTML.contains("id=\"show-unit-cell\""));
    assert!(!HTML.contains("id=\"show-symmetry-mates\""));
    assert!(CSS.contains(".symmetry-cell"));
    assert!(CSS.contains(".symmetry-menu-popup"));
}

#[test]
fn tauri_surface_cutter_matches_reference_object_modes() {
    for label in [
        "Cut molecular surface by the current clipping plane:",
        "Target surface",
        "Cross section type",
        "Complete",
        "Separately",
        "Section only",
        "No section",
        "Section mesh density (/Å)",
    ] {
        assert!(JS.contains(label), "missing surface cutter field {label}");
    }
    assert!(JS.contains("mutate(\"cut_surface\""));
    assert!(JS.contains("surfaceUid: Number(surface.value)"));
    assert!(JS.contains("mode: mode.value"));
}

#[test]
fn tauri_open_pdb_matches_reference_coordinate_and_map_options() {
    for label in [
        "Download PDB",
        "Enter PDB ID:",
        "Coordinates",
        "Fetch pdb file",
        "RCSB (files.rcsb.org)/cif.gz",
        "RCSB (files.rcsb.org)/pdb.gz",
        "Density maps",
        "Fetch map file (2Fo-Fc)",
        "Fetch map file (Fo-Fc)",
        "RCSB (files.rcsb.org)",
        "EBI (www.ebi.ac.uk)",
        "Setup renderer",
        "Object:",
        "Renderer type:",
        "Renderer name:",
        "Selection:",
        "Recenter view",
    ] {
        assert!(JS.contains(label), "missing Open PDB field {label}");
    }
    for binding in [
        "pdbHistoryRequest = invoke(\"get_pdb_history\")",
        "const historyRequest = requestPdbHistory();",
        "const request = invoke(\"get_pdb_renderer_setup\", { pdbId })",
        "void requestPdbRendererSetup(args.pdbId).catch(() => {});",
        "await invoke(\"check_pdb_availability\", args)",
        "pdbHistoryList.id = \"open-pdb-history\"",
        "pdbId.setAttribute(\"list\", pdbHistoryList.id)",
        "fetchCoordinates: fetchCoordinates.checked",
        "coordinateSource: coordinateSource.value",
        "fetchTwoFofc: fetchTwoFofc.checked",
        "fetchFofc: fetchFofc.checked",
        "mapSource: mapSource.value",
        "rendererSetup",
    ] {
        assert!(JS.contains(binding), "missing Open PDB binding {binding}");
    }
    assert!(JS.contains("coordinateSource.disabled = !fetchCoordinates.checked"));
    assert!(JS.contains("mapSource.disabled = !mapsSelected"));
    assert!(JS.contains("Checking availability…"));
    let pdb_availability_flow = JS
        .split("const scheduleAvailabilityCheck = () => {")
        .nth(1)
        .expect("Open PDB availability flow");
    assert!(
        pdb_availability_flow
            .find("requestPdbRendererSetup(args.pdbId)")
            .unwrap()
            < pdb_availability_flow
                .find("availabilityTimer = setTimeout")
                .unwrap()
    );
    assert!(pdb_availability_flow.contains("}, 100);"));
    assert!(JS.contains("Neither pdb nor map selected"));
    assert!(JS.contains("setTimeout(async () =>"));
}

#[test]
fn tauri_workspace_chrome_reuses_unchanged_dom() {
    assert!(JS.contains("function workspaceChromeRenderKey()"));
    let workspace_key = JS
        .split("function workspaceChromeRenderKey()")
        .nth(1)
        .and_then(|source| source.split("function renderSceneTabs").next())
        .expect("Workspace render-key function");
    assert!(JS.contains("const workspaceRenderKey = workspaceChromeRenderKey();"));
    assert!(JS.contains("if (renderedSceneTabsKey === renderKey) return;"));
    assert!(JS.contains("if (renderedSceneTreeKey === renderKey) return;"));
    assert!(JS.contains("if (renderedInspectorKey === renderKey) return;"));
    assert!(JS.contains("renderSceneTabs(workspaceRenderKey)"));
    assert!(JS.contains("renderSceneTree(workspaceRenderKey)"));
    assert!(JS.contains("renderInspector(workspaceRenderKey)"));
    assert!(workspace_key.matches(".uiCollapsed").count() >= 5);
    assert!(!workspace_key.contains("snapshot.camera."));
    assert!(!workspace_key.contains("snapshot.densityLevel"));
}

#[test]
fn tauri_playback_reuses_static_animation_and_trajectory_dom() {
    assert!(JS.contains("let renderedAnimationListKey = \"\""));
    assert!(JS.contains("let renderedTrajectoryListKey = \"\""));
    assert!(JS.contains("let renderedTrajectoryBlocksKey = \"\""));
    assert!(JS.contains("if (renderedAnimationListKey !== listKey)"));
    assert!(JS.contains("if (renderedTrajectoryListKey !== listKey)"));
    assert!(JS.contains("if (renderedTrajectoryBlocksKey !== blocksKey)"));

    let animation_key = JS
        .split("const listKey = JSON.stringify([")
        .nth(1)
        .and_then(|source| source.split("if (renderedAnimationListKey").next())
        .expect("animation list render key");
    assert!(!animation_key.contains("animationElapsed"));
    let trajectory_blocks_key = JS
        .split("const blocksKey = JSON.stringify([")
        .nth(1)
        .and_then(|source| source.split("if (renderedTrajectoryBlocksKey").next())
        .expect("trajectory block render key");
    assert!(!trajectory_blocks_key.contains("currentFrame"));
    assert!(TAURI_APP_RS.contains(".apply_scene_animation_time("));
    assert!(TAURI_APP_RS.contains("animation_start_desktop"));
    assert!(!TAURI_APP_RS.contains("anim_step(&objects"));
}

#[test]
fn tauri_high_frequency_viewport_updates_reuse_measured_dimensions() {
    assert!(JS.contains("let cachedViewportDimensions = null"));
    assert!(JS.contains("let cachedViewportPixelRatio = null"));
    assert!(JS.contains("let renderedEmptyViewportOverlayKey = \"\""));
    assert!(JS.contains("function viewportDimensions(forceMeasure = false)"));
    assert!(JS.contains("cachedViewportPixelRatio === pixelRatio"));
    assert!(JS.contains("const size = viewportDimensions(true)"));
    assert!(JS.contains("async function seekTrajectory(frame, indicateBusy = true)"));
    assert!(JS.contains("await seekTrajectory(next, false)"));
    assert!(JS.contains("renderedEmptyViewportOverlayKey !== emptyOverlayKey"));
}

#[test]
fn tauri_workspace_preserves_renderer_group_hierarchy() {
    assert!(JS.contains("function groupedMoleculeRenderers(renderers)"));
    assert!(JS.contains("candidate.group === renderer.name"));
    assert!(JS.contains("workspaceSelectionKind === \"rendererGroup\""));
    assert!(JS.contains("workspaceProperties.addEventListener(\"click\", async () =>"));
    assert!(JS.contains("await renameWorkspaceRendererGroup(scene, \"rendererGroup\""));
    assert!(JS.contains("groupName,"));
    let add_renderer = JS
        .split("case \"add-renderer\": {")
        .nth(1)
        .and_then(|source| source.split("case \"delete-workspace-item\":").next())
        .expect("Workspace Add action");
    assert!(add_renderer.contains("await showNewWorkspaceRendererDialog("));
    assert!(add_renderer.contains("currentRenderer.rendererType === \"*group\""));
    assert!(!add_renderer.contains("mutate(\"add_renderer\""));
    assert!(JS.contains("function workspaceRendererGroupTwisty(object, renderer)"));
    assert!(
        JS.contains("async function openWorkspaceRendererProperty(scene, kind, object, renderer)")
    );
    assert!(JS.contains("if (renderer.rendererType === \"*group\")"));
    assert!(JS.contains("mutateWithoutViewport(\"set_workspace_renderer_collapsed\""));
    assert!(JS.contains("node.renderer.uiCollapsed"));
    assert!(JS.contains("function workspaceObjectTwisty(object)"));
    assert!(JS.contains("mutateWithoutViewport(\"set_workspace_object_collapsed\""));
    assert!(JS.contains("if (object.uiCollapsed) return []"));
    assert!(CSS.contains(".tree-renderer.renderer-group"));
    assert!(CSS.contains(".tree-renderer.group-child"));
    assert!(CSS.contains(".workspace-group-twisty"));
    assert!(CSS.contains(".workspace-object-twisty"));
    assert!(NATIVE_GUI_RS.contains("fn set_workspace_renderer_group_collapsed("));
    assert!(NATIVE_GUI_RS.contains("fn set_workspace_object_collapsed("));
    assert!(NATIVE_GUI_RS.contains(".workspace_aux_group_summary(selection)"));
    assert!(
        NATIVE_GUI_RS.contains(".or_else(|| self.workspace_aux_renderer_group_name(selection))")
    );
    assert!(NATIVE_GUI_RS.contains("if response.double_clicked()"));
    assert!(NATIVE_GUI_RS.contains("self.open_renderer_group_rename("));
    assert!(NATIVE_GUI_RS.contains("if let Some(group_name) = self.selected_renderer_group_name()"));
    assert!(NATIVE_GUI_RS.contains("self.workspace_aux_group_summary(selection)"));
}

#[test]
fn tauri_workspace_reorders_and_regroups_renderers_by_dragging() {
    assert!(JS.contains("function configureWorkspaceRendererDrag(row, object, renderer"));
    assert!(JS.contains("row.draggable = !disabled"));
    assert!(JS.contains("function workspaceRendererDragPayload(object, renderer, target)"));
    assert!(JS.contains("application/x-cuemol-renderer"));
    assert!(JS.contains("function workspaceRendererDropPosition(event, renderer)"));
    assert!(JS.contains("\"move_workspace_renderers\" : \"move_workspace_renderer\""));
    assert!(TAURI_APP_RS.contains("fn move_workspace_renderers("));
    assert!(CSS.contains(".tree-renderer.drop-before::after"));
    assert!(CSS.contains(".tree-renderer.drop-inside"));
}

#[test]
fn tauri_workspace_reorders_objects_across_supported_kinds() {
    assert!(JS.contains("function configureWorkspaceObjectDrag(row, sceneUid, kind, uid)"));
    assert!(JS.contains("draggable = !objectIsRenaming && workspaceSelectionKind !== \"multiple\""));
    assert!(JS.contains("application/x-cuemol-object"));
    assert!(JS.contains("mutateWithoutViewport(\"move_workspace_object\""));
    assert!(JS.contains("workspaceObjectBlocks.sort("));
    assert!(CSS.contains(".tree-object.drop-before::after"));
}

#[test]
fn tauri_workspace_context_menu_matches_selection_presets() {
    assert!(HTML.contains("id=\"workspace-add\""));
    assert!(HTML.contains("id=\"inspector-add-renderer\""));
    assert!(JS.contains("workspaceAdd.disabled ="));
    assert!(JS.contains("inspectorAddRenderer.disabled ="));
    assert!(JS.contains(
        "[\"multiple\", \"rendererGroup\", \"cameraRoot\", \"camera\", \"styleRoot\", \"style\"]"
    ));
    assert!(NATIVE_GUI_RS.contains("fn workspace_selection_can_zoom(&self) -> bool"));
    assert!(NATIVE_GUI_RS.contains("workspace_aux_group_summary(selection).is_none()"));
    assert!(NATIVE_GUI_RS.contains("let add_enabled = !multiple_selected"));
    assert!(JS.contains("function showWorkspaceContextMenu(event, items)"));
    assert!(JS.contains("function workspaceObjectContextItems(scene, object)"));
    assert!(JS.contains("function workspaceAuxObjectContextItems(scene, kind, object)"));
    assert!(JS.contains(
        "async function workspaceAuxRendererContextItems(scene, kind, object, renderer)"
    ));
    assert!(JS.contains("[3, 5, 7, 10].map((radius) => ({"));
    assert!(JS.contains("function workspaceRendererContextItems(scene, object, renderer)"));
    assert!(JS.contains("set_workspace_renderer_selection"));
    assert!(JS.contains("change_workspace_renderer_type"));
    assert!(JS.contains("create_workspace_renderer"));
    assert!(JS.contains("async function showNewWorkspaceRendererDialog("));
    assert!(JS.contains("dialogTitle.textContent = \"Setup renderer\""));
    assert!(JS.contains("get_workspace_renderer_setup_options"));
    assert!(JS.contains("setupDefaults?.rendererType"));
    assert!(JS.contains("setupDefaults?.recenter ?? true"));
    assert!(TAURI_APP_RS.contains("GetWorkspaceRendererSetupOptions"));
    assert!(TAURI_APP_RS.contains("new_renderer_from_setup("));
    assert!(TAURI_APP_RS.contains("save_renderer_setup_preferences(&renderer_type, recenter)"));
    assert!(NATIVE_GUI_RS.contains("molecule_renderer_setup_options(scene_uid)"));
    assert!(NATIVE_GUI_RS.contains("new_renderer_name_default"));
    assert!(JS.contains("densityCentering: kind === \"densityRenderer\" ? recenter.value : null"));
    assert!(TAURI_APP_RS.contains("set_density_renderer_center("));
    assert!(NATIVE_GUI_RS.contains("\"Set map center\""));
    assert!(NATIVE_GUI_RS.contains("\"Move view center\""));
    assert!(JS.contains("cachedWorkspaceRendererStyles"));
    assert!(JS.contains("apply_workspace_renderer_style"));
    assert!(JS.contains("showApplyRendererStyleDialog(object, renderer, styleOptions)"));
    assert!(JS.contains("set_workspace_renderer_coloring"));
    assert!(JS.contains("insert_workspace_renderer_paint"));
    assert!(JS.contains("insert_workspace_object_paint"));
    assert!(JS.contains("copy_workspace_renderer"));
    assert!(JS.contains("paste_workspace_renderers"));
    assert!(JS.contains("copy_workspace_object"));
    assert!(JS.contains("paste_workspace_object"));
    assert!(JS.contains("copy_workspace_camera"));
    assert!(JS.contains("paste_workspace_camera"));
    assert!(JS.contains("copy_workspace_style"));
    assert!(JS.contains("paste_workspace_style"));
    assert!(JS.contains("workspaceSceneContextItems"));
    assert!(JS.contains("function workspaceRendererNames(sceneUid)"));
    assert!(JS.contains("async function renameWorkspaceRendererGroup("));
    assert!(JS.contains("label: \"Change rend group name:\""));
    let tauri_scene_menu = JS
        .split("function workspaceSceneContextItems(scene)")
        .nth(1)
        .and_then(|source| source.split("async function copyWorkspaceCamera").next())
        .expect("Tauri scene context menu");
    let mut cursor = 0;
    for item in [
        "label: \"Background color\"",
        "label: \"Use color proofing\"",
        "label: \"Paste\"",
        "label: \"Properties...\"",
    ] {
        let relative = tauri_scene_menu[cursor..]
            .find(item)
            .unwrap_or_else(|| panic!("missing scene menu item {item}"));
        cursor += relative + item.len();
    }
    assert!(!tauri_scene_menu.contains("label: currentScene.name"));
    assert!(!tauri_scene_menu.contains("label: \"Paste Object\""));
    assert!(tauri_scene_menu.contains("disabled: !workspaceObjectClipboardAvailable"));
    assert!(NATIVE_GUI_RS.contains("egui::Button::new(\"Paste\")"));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Name for new group:\")"));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Change rend group name:\")"));
    assert!(NATIVE_GUI_RS.contains("scene_attribute(\"icc_filename\")"));
    assert!(JS.contains("workspaceCameraContextItems"));
    assert!(JS.contains("label: \"New Camera...\""));
    assert!(JS.contains("label: \"Camera file\""));
    assert!(JS.contains("function loadWorkspaceCameraFile(scene)"));
    assert!(JS.contains("function saveWorkspaceCameraFile(scene, camera, saveAs)"));
    assert!(JS.contains("name: \"Camera setting (*.cam)\""));
    assert!(JS.contains("mutate(\"load_camera_file\""));
    assert!(JS.contains("mutateWithoutViewport(\"reload_camera_file\""));
    assert!(JS.contains("mutateWithoutViewport(\"save_camera_file\""));
    assert!(JS.contains("Save from scene (with vis flags)"));
    assert!(JS.contains("Apply to scene (with vis flags)"));
    assert!(JS.contains("workspaceStyleContextItems"));
    assert!(JS.contains("label: \"Style file\""));
    assert!(JS.contains("function loadWorkspaceStyleFile(scene)"));
    assert!(JS.contains("function saveWorkspaceStyleFile(scene, style, saveAs)"));
    assert!(JS.contains("function reloadWorkspaceStyleFile(scene, style)"));
    assert!(JS.contains("name: \"Style file (*.xml)\""));
    assert!(JS.contains("mutate(\"load_style_file\""));
    assert!(JS.contains("mutateWithoutViewport(\"reload_style_file\""));
    assert!(JS.contains("mutateWithoutViewport(\"save_style_file\""));
    assert!(!JS.contains("Not implemented!!"));
    assert!(NATIVE_GUI_RS.contains("label: \"Camera setting (*.cam)\""));
    assert!(NATIVE_GUI_RS.contains("label: \"Style file (*.xml)\""));
    assert!(NATIVE_GUI_RS.contains("label: \"Internal Javascript (*.js)\""));
    assert!(JS.contains("function updateWorkspaceStyle(style, update)"));
    assert!(JS.contains("label: \"New Style...\""));
    assert!(JS.contains("async function openNewStyleFlow()"));
    assert!(JS.contains("title: \"New Style\""));
    assert!(JS.contains("label: \"Name for new style:\""));
    assert!(JS.contains("defaultValue: `style_${index}`"));
    assert!(NATIVE_GUI_RS.contains("ui.button(\"New Style...\")"));
    assert!(NATIVE_GUI_RS.contains("egui::Window::new(\"New Style\")"));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Name for new style:\")"));
    assert!(JS.contains("label: \"New Group...\""));
    assert!(JS.contains("title: \"New Renderer Group\""));
    assert!(JS.contains("label: \"Name for new group:\""));
    assert!(JS.contains("defaultValue: `group${serial}`"));
    assert!(!JS.contains("globalThis.prompt?.(\"Name for new group\""));
    assert!(JS.contains("label: \"Read-only\""));
    assert!(JS.contains("label: \"Background color\""));
    assert!(JS.contains("label: \"White\""));
    assert!(JS.contains("label: \"Black\""));
    assert!(JS.contains("option: \"color-proof\", value"));
    assert!(JS.contains("currentScene.useColorProof && currentScene.iccFilename"));
    assert!(JS.contains("showReferenceDialog(\"Scene Properties\")"));
    assert!(JS.contains("role\", \"menuitemcheckbox"));
    assert!(JS.contains("workspaceRendererClipboardAvailable"));
    assert!(JS.contains("Paste Renderer"));
    assert!(JS.contains("label: \"Save As ...\""));
    assert!(JS.contains("label: \"Delete\""));
    assert!(JS.contains("label: \"Generate surface obj\""));
    assert!(JS.contains("generate_density_surface"));
    assert!(JS.contains("label: \"Regenerate surface...\""));
    assert!(JS.contains("regenerate_surface"));
    assert!(JS.contains("function showSurfaceRegenerateDialog(object)"));
    assert!(JS.contains("dialogTitle.textContent = \"Mol surface tool\""));
    assert!(JS.contains("surfaceUid: object.uid, density"));
    assert!(TAURI_APP_RS.contains("regenerate_surface_object_with_density"));
    assert!(NATIVE_GUI_RS.contains("fn surface_regenerate_dialog("));
    assert!(JS.contains("mutate(\"create_molecular_surface\""));
    assert!(!JS.contains("chooseOutputPath(`${name}.obj`"));
    assert!(TAURI_APP_RS.contains("ModelRequest::CreateMolecularSurface"));
    assert!(TAURI_APP_RS.contains("fn create_molecular_surface("));
    assert!(NATIVE_GUI_RS.contains("fn open_surface_generation_tool(&mut self)"));
    assert!(NATIVE_GUI_RS.contains("create_molecular_surface_object("));
    assert!(CSS.contains(".workspace-context-menu.has-submenu"));
    assert!(JS.contains("WORKSPACE_PAINT_MONOCHROME"));
    assert!(JS.contains("function workspacePaintMenuChildren(action)"));
    assert!(JS.contains("label: \"B-factor\""));
    assert!(JS.contains("label: \"Style\", children: styleChildren"));
    assert!(JS.contains("Edit interaction list..."));
    let aux_renderer_menu = JS
        .split("async function workspaceAuxRendererContextItems(scene, kind, object, renderer)")
        .nth(1)
        .and_then(|source| source.split("function workspaceStyleSignature").next())
        .expect("Tauri auxiliary renderer context menu");
    for item in [
        "label: \"Change sel\"",
        "label: \"Change type\"",
        "label: \"Coloring\"",
        "label: \"Paint\"",
        "label: \"Style\"",
        "label: \"Edit style...\"",
        "label: \"Create style...\"",
        "label: \"New Renderer...\"",
        "label: \"Properties...\"",
    ] {
        assert!(aux_renderer_menu.contains(item), "missing {item}");
    }
    assert!(JS.contains("function changeWorkspaceAuxRendererType("));
    assert!(JS.contains("kind: \"renderer\""));
    assert!(JS.contains("create_workspace_renderer_group"));
    assert!(JS.contains("rename_workspace_node"));
    assert!(JS.contains("label: \"Rename...\""));
    assert!(!JS.contains("globalThis.prompt"));
    assert!(JS.contains("function beginWorkspaceInlineRename(kind, objectUid, rendererUid = null)"));
    assert!(
        JS.contains("function scheduleWorkspaceInlineRename(kind, objectUid, rendererUid = null)")
    );
    assert!(JS.contains("}, 500);"));
    assert!(JS.contains("workspaceSelectionKind === kind"));
    assert!(JS.contains("`Scene: ${scene.name || \"Untitled\"}`"));
    assert!(JS.contains("function selectedWorkspaceRenameCoordinates("));
    assert!(JS.contains("case \"densityMap\":"));
    assert!(JS.contains("case \"densityRenderer\":"));
    assert!(JS.contains("case \"surface\":"));
    assert!(JS.contains("case \"surfaceRenderer\":"));
    assert!(JS.contains("input.addEventListener(\"blur\", () => void finish(true))"));
    assert!(JS.contains("event.key === \"Enter\""));
    assert!(JS.contains("event.key === \"Escape\""));
    assert!(CSS.contains(".workspace-inline-rename"));
    let tauri_object_menu = JS
        .split("function workspaceObjectContextItems(scene, object)")
        .nth(1)
        .and_then(|source| source.split("function selectWorkspaceAuxItem").next())
        .expect("Tauri object context menu");
    let mut cursor = 0;
    for item in [
        "label: \"Selection\"",
        "label: \"Paint\"",
        "label: \"Copy Object\"",
        "label: \"Paste Renderer\"",
        "label: \"New Renderer...\"",
        "label: \"New Group...\"",
        "label: \"Save As ...\"",
        "label: \"Delete Object\"",
        "label: \"Properties...\"",
    ] {
        let relative = tauri_object_menu[cursor..]
            .find(item)
            .unwrap_or_else(|| panic!("missing object menu item {item}"));
        cursor += relative + item.len();
    }
    for item in [
        "label: object.name",
        "object.visible ? \"Hide\" : \"Show\"",
        "label: \"Rename…\"",
        "if (current && object.coloringType === \"PaintColoring\")",
    ] {
        assert!(
            !tauri_object_menu.contains(item),
            "unexpected object menu item {item}"
        );
    }
    for item in [
        "label: \"Invert\"",
        "label: \"Toggle Sidechain\"",
        "label: \"Around Byresid\"",
        "label: \"Around\"",
        "label: \"Ligand\"",
    ] {
        assert!(
            tauri_object_menu.contains(item),
            "missing object selection item {item}"
        );
    }
    assert!(!tauri_object_menu.contains("Selection ·"));
    assert!(tauri_object_menu.contains("disabled: !workspaceRendererClipboardAvailable"));
    assert!(NATIVE_GUI_RS.contains("workspace_paint_menu(ui, true"));
    let tauri_aux_object_menu = JS
        .split("function workspaceAuxObjectContextItems(scene, kind, object)")
        .nth(1)
        .and_then(|source| {
            source
                .split("async function workspaceAuxRendererContextItems")
                .next()
        })
        .expect("Tauri auxiliary object context menu");
    let mut cursor = 0;
    for item in [
        "label: \"Selection\"",
        "label: \"Paint\"",
        "label: \"Copy Object\"",
        "label: \"Paste Renderer\"",
        "label: \"New Renderer...\"",
        "label: \"New Group...\"",
        "label: \"Save As ...\"",
        "label: \"Delete Object\"",
        "label: \"Properties...\"",
    ] {
        let relative = tauri_aux_object_menu[cursor..]
            .find(item)
            .unwrap_or_else(|| panic!("missing auxiliary object menu item {item}"));
        cursor += relative + item.len();
    }
    assert!(tauri_aux_object_menu.contains("kind === \"surface\" && object.originalMolecule"));
    assert!(tauri_aux_object_menu.contains("label: \"Regenerate surface...\""));
    assert!(!tauri_aux_object_menu.contains("label: object.name"));
    assert!(!tauri_aux_object_menu.contains("label: \"Rename…\""));
    assert!(tauri_aux_object_menu.contains("disabled: !workspaceRendererClipboardAvailable"));
    let tauri_renderer_group_menu = JS
        .split("async function workspaceRendererContextItems(scene, object, renderer)")
        .nth(1)
        .and_then(|source| {
            source
                .split("if (renderer.rendererType === \"*group\") {")
                .nth(1)
        })
        .and_then(|source| source.split("return items;").next())
        .expect("Tauri renderer-group context menu");
    let mut cursor = 0;
    for item in [
        "label: \"Copy\"",
        "label: \"Paste Renderer\"",
        "label: \"Change Name\"",
        "label: \"New Renderer...\"",
        "label: \"Delete\"",
    ] {
        let relative = tauri_renderer_group_menu[cursor..]
            .find(item)
            .unwrap_or_else(|| panic!("missing renderer-group menu item {item}"));
        cursor += relative + item.len();
    }
    for omitted in [
        "{ label: renderer.name, disabled: true }",
        "renderer.visible ? \"Hide\" : \"Show\"",
        "label: \"Properties…\"",
    ] {
        assert!(
            !tauri_renderer_group_menu.contains(omitted),
            "unexpected renderer-group menu item {omitted}"
        );
    }
    let tauri_renderer_menu = JS
        .split("async function workspaceRendererContextItems(scene, object, renderer)")
        .nth(1)
        .and_then(|source| source.split("return items;").nth(1))
        .and_then(|source| {
            source
                .split("function clearWorkspaceRendererDropMarkers")
                .next()
        })
        .expect("Tauri renderer context menu");
    let mut cursor = 0;
    for item in [
        "label: \"Change sel\"",
        "label: \"Change type\"",
        "label: \"Coloring\"",
        "label: \"Paint\"",
        "label: \"Style\"",
        "label: \"Edit style...\"",
        "label: \"Create style...\"",
        "label: \"Copy\"",
        "label: \"New Renderer...\"",
        "label: \"Delete\"",
        "label: \"Properties...\"",
    ] {
        let relative = tauri_renderer_menu[cursor..]
            .find(item)
            .unwrap_or_else(|| panic!("missing renderer menu item {item}"));
        cursor += relative + item.len();
    }
    for item in [
        "{ label: renderer.name, disabled: true }",
        "renderer.visible ? \"Hide\" : \"Show\"",
        "label: \"Rename…\"",
    ] {
        assert!(
            !tauri_renderer_menu.contains(item),
            "unexpected renderer menu item {item}"
        );
    }
    assert!(!tauri_renderer_menu.contains("Change selection ·"));
    assert!(!tauri_renderer_menu.contains("Change type ·"));
    assert!(!tauri_renderer_menu.contains("Paint ·"));
    assert!(!tauri_renderer_menu.contains("Style ·"));
    assert!(!tauri_renderer_menu.contains("label: \"Paint\", disabled: true"));
    assert!(tauri_renderer_menu.contains("disabled: renderer.coloringType !== \"PaintColoring\""));
    assert!(tauri_renderer_menu.contains("disabled: selectionHidden"));
    assert!(NATIVE_GUI_RS.contains("ui.button(\"Rename...\")"));
    assert!(NATIVE_GUI_RS.contains("ui.button(\"Change Name\")"));
    assert!(NATIVE_GUI_RS.contains("ui.button(\"New Renderer...\")"));
    assert!(NATIVE_GUI_RS.contains("ui.menu_button(\"Change sel\""));
    assert!(CSS.contains(".workspace-context-menu button.checked::before"));
}

#[test]
fn tauri_workspace_exposes_reference_camera_nodes_and_actions() {
    assert!(JS.contains("function activeCameras()"));
    assert!(JS.contains("workspaceSelectionKind === \"cameraRoot\""));
    assert!(JS.contains("workspaceSelectionKind === \"camera\""));
    assert!(JS.contains("mutate(\"save_camera\""));
    assert!(JS.contains("mutate(\"load_camera\""));
    assert!(JS.contains("mutateWithoutViewport(\"save_camera_view\""));
    assert!(JS.contains("mutate(\"load_camera_view\""));
    assert!(JS.contains("mutate(\"destroy_camera\""));
    assert!(JS.contains("invoke(\"get_camera_properties\""));
    assert!(JS.contains("invoke(\"set_camera_properties\""));
    assert!(JS.contains("function showCameraVisibilityEditor(scene, camera)"));
    assert!(JS.contains("invoke(\"get_camera_visibility\""));
    assert!(JS.contains("\"set_camera_visibility\""));
    assert!(JS.contains("\"clear_camera_visibility\""));
    assert!(JS.contains("Edit visibility flags"));
    assert!(JS.contains("label: \"Edit vis flags...\""));
    assert!(JS.contains("label: \"Clear vis flags\""));
    assert!(JS.contains("label: \"Save from view\""));
    assert!(JS.contains("label: \"Apply to view\""));
    assert!(JS.contains("camera.visibilityCount === 0"));
    assert!(CSS.contains(".visibility-flag-editor"));
    assert!(CSS.contains("width: min(420px, calc(100vw - 48px))"));
    assert!(JS.contains("function showTextPromptDialog({"));
    assert!(JS.contains("function showConfirmationDialog({"));
    assert!(JS.contains("async function openNewCameraFlow()"));
    assert!(JS.contains("title: \"New Camera\""));
    assert!(JS.contains("label: \"Name for new camera:\""));
    assert!(JS.contains("defaultValue: `camera_${index}`"));
    assert!(JS.contains("confirmLabel: \"Create\""));
    assert!(JS.contains("mutateWithoutViewport(\"create_camera\", { name }, () => {})"));
    assert!(JS.contains("function workspaceCameraInlineRenameInput(scene, camera)"));
    assert!(JS.contains("async function renameWorkspaceCamera(scene, camera)"));
    assert!(JS.contains("label: `Rename camera \"${camera.name}\":`"));
    assert!(JS.contains("async function deleteWorkspaceCamera(scene, camera)"));
    assert!(JS.contains("message: `Delete camera: ${camera.name}`"));
    assert!(JS.contains("\"rename_workspace_camera\""));
    assert!(!JS.contains("globalThis.prompt?.(\"Camera name\""));
    assert!(CSS.contains(".reference-dialog.text-prompt-dialog"));
    assert!(TAURI_APP_RS.contains("ModelRequest::CreateCamera(name)"));
    assert!(TAURI_APP_RS.contains("fn create_camera(name: String"));
    assert!(TAURI_APP_RS.contains("RenameWorkspaceCamera(WorkspaceCameraRenameInput)"));
    assert!(TAURI_APP_RS.contains("fn rename_workspace_camera("));
    assert!(NATIVE_GUI_RS.contains("struct NativeNewCameraDraft"));
    assert!(NATIVE_GUI_RS.contains("fn new_camera_dialog("));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Name for new camera:\")"));
    assert!(NATIVE_GUI_RS.contains("egui::Button::new(\"Create\")"));
    assert!(NATIVE_GUI_RS.contains("fn camera_rename_dialog("));
    assert!(NATIVE_GUI_RS.contains("Rename camera \\\"{}\\\":"));
    assert!(NATIVE_GUI_RS.contains("fn camera_delete_dialog("));
    assert!(NATIVE_GUI_RS.contains("Delete camera: {}"));
    assert!(!NATIVE_GUI_RS.contains("open_new_camera_properties"));

    let tauri_editor = JS
        .split("async function showCameraVisibilityEditor(scene, camera)")
        .nth(1)
        .and_then(|source| source.split("function workspaceCameraContextItems").next())
        .expect("Tauri camera visibility editor");
    assert!(tauri_editor.contains("Edit visibility flags: ${camera.name}"));
    assert!(tauri_editor.contains(r#"["Inc", "Object / Renderer", "Vis"]"#));
    assert!(tauri_editor.contains("(no scene elements)"));
    assert!(tauri_editor.contains("entry.isObject ? \"\" : \"(renderer)\""));
    assert!(!tauri_editor.contains("button(\"Apply\""));
    assert!(!tauri_editor.contains("entry.visible = false"));

    let native_editor = NATIVE_GUI_RS
        .split("fn camera_visibility_dialog(&mut self, ctx: &egui::Context)")
        .nth(1)
        .and_then(|source| source.split("fn style_paste_confirmation_dialog").next())
        .expect("native camera visibility editor");
    assert!(native_editor.contains("Edit visibility flags: {}"));
    assert!(native_editor.contains("Object / Renderer"));
    assert!(native_editor.contains("ui.weak(\"(renderer)\")"));
    assert!(native_editor.contains("ui.weak(\"(no scene elements)\")"));
    assert!(!native_editor.contains("ui.button(\"Apply\")"));
    assert!(!native_editor.contains("entry.visible = false"));

    let tauri_camera_menu = JS
        .split("function workspaceCameraContextItems(scene, camera = null)")
        .nth(1)
        .and_then(|source| source.split("function workspaceStyleContextItems").next())
        .expect("Tauri camera context menu");
    let mut cursor = 0;
    for item in [
        "label: \"New Camera...\"",
        "label: \"Delete\"",
        "label: \"Copy\"",
        "label: \"Paste\"",
        "label: \"Camera file\"",
        "label: \"Save from view\"",
        "label: \"Apply to view\"",
        "label: \"Save from scene (with vis flags)\"",
        "label: \"Apply to scene (with vis flags)\"",
        "label: \"Edit vis flags...\"",
        "label: \"Clear vis flags\"",
        "label: \"Rename...\"",
        "label: \"Properties...\"",
    ] {
        let relative = tauri_camera_menu[cursor..]
            .find(item)
            .unwrap_or_else(|| panic!("missing camera menu item {item}"));
        cursor += relative + item.len();
    }
    for item in [
        "label: \"Load ...\"",
        "label: \"Reload\"",
        "label: \"Save\"",
        "label: \"Save as...\"",
    ] {
        assert!(
            tauri_camera_menu.contains(item),
            "missing camera file item {item}"
        );
    }
    assert!(!tauri_camera_menu.contains("camera?.name || \"Cameras\""));
    assert!(!tauri_camera_menu.contains("label: \"Paste Camera\""));
    assert!(tauri_camera_menu.contains("disabled: !camera"));
    assert!(tauri_camera_menu.contains("disabled: !workspaceCameraClipboardAvailable"));
    let clear_action = tauri_camera_menu
        .split("label: \"Clear vis flags\"")
        .nth(1)
        .and_then(|source| source.split("label: \"Rename...\"").next())
        .expect("Tauri clear camera visibility action");
    assert!(clear_action.contains("mutateWithoutViewport(\"clear_camera_visibility\""));
    assert!(!clear_action.contains("globalThis.confirm"));
    assert!(!NATIVE_GUI_RS.contains("pending_camera_visibility_clear"));
    assert!(!NATIVE_GUI_RS.contains("fn camera_visibility_clear_dialog"));

    let native_camera_root = NATIVE_GUI_RS
        .split("let root_selection = WorkspaceAuxSelection::CameraRoot")
        .nth(1)
        .and_then(|source| source.split("ui.indent(format!(\"camera-root-").next())
        .expect("native camera-root context menu");
    assert!(!native_camera_root.contains("ui.weak(\"Cameras\")"));
    assert!(!native_camera_root.contains("Paste Camera"));
    for item in [
        "ui.button(\"New Camera...\")",
        "egui::Button::new(\"Delete\")",
        "egui::Button::new(\"Copy\")",
        "egui::Button::new(\"Paste\")",
        "ui.button(\"Load ...\")",
        "egui::Button::new(\"Reload\")",
        "egui::Button::new(\"Save\")",
        "egui::Button::new(\"Save as...\")",
        "egui::Button::new(\"Rename...\")",
        "egui::Button::new(\"Properties...\")",
    ] {
        assert!(
            native_camera_root.contains(item),
            "missing native camera-root item {item}"
        );
    }
}

#[test]
fn tauri_workspace_multiple_selection_matches_reference_actions() {
    assert!(JS.contains("let workspaceMultipleSelection = [];"));
    assert!(JS.contains("let workspaceSelectionAnchor = null;"));
    assert!(JS.contains("function selectWorkspaceTreeTarget(event, target)"));
    assert!(JS.contains("const multiple = event.ctrlKey || event.metaKey;"));
    assert!(JS.contains("if (event.shiftKey)"));
    assert!(JS.contains("row.workspaceSelectionTarget"));
    assert!(JS.contains("function workspaceMultipleContextItems()"));
    assert!(JS.contains("async function deleteWorkspaceMultipleItems(targets)"));
    assert!(JS.contains("approved.push(target)"));
    assert!(JS.contains("function showWorkspaceMultipleContextMenu(event, target)"));
    assert!(JS.contains("function workspaceRendererTargetClipboardKind(target)"));
    assert!(!JS.contains("workspaceRendererClipboardKind"));
    assert!(JS.contains("workspaceSelectionTarget(\"camera\""));
    assert!(JS.contains("workspaceSelectionTarget(\"style\""));
    assert!(JS.contains("sceneUid: target.objectUid"));
    assert!(JS.contains("localIndex: target.rendererUid"));
    assert!(JS.contains("rendererGroup: \"object\""));
    for kind in ["densityRenderer", "surfaceRenderer", "lwRenderer"] {
        assert!(JS.contains(&format!("{kind}:")));
    }
    assert!(JS.contains("invoke(\"copy_workspace_renderers\""));
    assert!(JS.contains("mutate(\"set_workspace_items_visible\""));
    assert!(JS.contains("mutate(\"delete_workspace_items\""));
    for label in ["Copy", "Delete", "Show", "Hide"] {
        assert!(JS.contains(&format!("label: \"{label}\"")));
    }
    assert!(NATIVE_GUI_RS.contains("enum NativeWorkspaceTarget"));
    assert!(NATIVE_GUI_RS.contains("input.modifiers.command"));
    assert!(NATIVE_GUI_RS.contains("input.modifiers.shift"));
    assert!(NATIVE_GUI_RS.contains("fn select_workspace_target_with_modifiers"));
    assert!(NATIVE_GUI_RS.contains("fn workspace_multiple_context_menu"));
    assert!(NATIVE_GUI_RS.contains("fn copy_workspace_target_renderers"));
    assert!(NATIVE_GUI_RS.contains("Some((owner.uid(), *renderer_uid))"));
    assert!(NATIVE_GUI_RS.contains("NativeWorkspaceTarget::Camera"));
    assert!(NATIVE_GUI_RS.contains("NativeWorkspaceTarget::Style"));
    assert!(NATIVE_GUI_RS.contains("fn set_workspace_targets_visible"));
    assert!(NATIVE_GUI_RS.contains("fn request_delete_workspace_targets"));
    assert!(NATIVE_GUI_RS.contains("fn multiple_delete_dialog"));
    assert!(NATIVE_GUI_RS.contains("fn advance_multiple_delete_confirmation"));
    assert!(NATIVE_GUI_RS.contains("fn delete_workspace_targets"));
}

#[test]
fn tauri_workspace_exposes_reference_style_nodes_and_actions() {
    assert!(JS.contains("function activeStyles()"));
    assert!(JS.contains("workspaceSelectionKind === \"styleRoot\""));
    assert!(JS.contains("workspaceSelectionKind === \"style\""));
    assert!(JS.contains("mutateWithoutViewport(\"create_style\""));
    assert!(JS.contains("async function deleteWorkspaceStyle(scene, style)"));
    assert!(JS.contains("message: `Delete style: ${style.name}`"));
    assert!(JS.contains("mutateWithoutViewport(\"destroy_style\""));
    assert!(JS.contains("invoke(\"get_style_properties\""));
    assert!(JS.contains("invoke(\"set_style_properties\""));
    assert!(JS.contains("async function rendererStyleCreateEditor(object, renderer)"));
    assert!(JS.contains("mutateWithoutViewport(\"create_renderer_style\""));
    assert!(JS.contains("Create Renderer Style"));
    assert!(JS.contains("Target Style set:"));
    assert!(JS.contains("Overwrite existing style"));
    assert!(CSS.contains(".renderer-style-name"));

    let tauri_style_menu = JS
        .split("function workspaceStyleContextItems(scene, style = null)")
        .nth(1)
        .and_then(|source| source.split("function workspaceObjectContextItems").next())
        .expect("Tauri style context menu");
    let mut cursor = 0;
    for item in [
        "label: \"New Style...\"",
        "label: \"Copy\"",
        "label: \"Paste\"",
        "label: \"Delete\"",
        "label: \"Style file\"",
        "label: \"Read-only\"",
        "label: \"Rename...\"",
        "label: \"Edit...\"",
    ] {
        let relative = tauri_style_menu[cursor..]
            .find(item)
            .unwrap_or_else(|| panic!("missing style menu item {item}"));
        cursor += relative + item.len();
    }
    for item in [
        "label: \"Load ...\"",
        "label: \"Reload\"",
        "label: \"Save\"",
        "label: \"Save As...\"",
    ] {
        assert!(
            tauri_style_menu.contains(item),
            "missing style file item {item}"
        );
    }
    assert!(!tauri_style_menu.contains("style?.name || \"Styles\""));
    assert!(!tauri_style_menu.contains("label: \"Paste Style\""));
    assert!(!tauri_style_menu.contains("label: \"Properties…\""));
    assert!(tauri_style_menu.contains("disabled: !style || style.localIndex === null"));
    assert!(tauri_style_menu.contains("disabled: !workspaceStyleClipboardName"));
    assert!(!tauri_style_menu.contains("globalThis.prompt"));
    assert!(NATIVE_GUI_RS.contains("fn style_delete_dialog("));
    assert!(NATIVE_GUI_RS.contains("Delete style: {}"));

    let native_style_menu = NATIVE_GUI_RS
        .split("let root_selection = WorkspaceAuxSelection::StyleRoot")
        .nth(1)
        .and_then(|source| {
            source
                .split("ui.separator();\n        ui.horizontal")
                .next()
        })
        .expect("native style context menus");
    assert!(!native_style_menu.contains("ui.weak(\"Styles\")"));
    assert!(!native_style_menu.contains("Paste Style"));
    for item in [
        "ui.button(\"New Style...\")",
        "egui::Button::new(\"Copy\")",
        "egui::Button::new(\"Paste\")",
        "egui::Button::new(\"Delete\")",
        "ui.button(\"Load ...\")",
        "egui::Button::new(\"Reload\")",
        "egui::Button::new(\"Save\")",
        "egui::Button::new(\"Save As...\")",
        "egui::Button::new(\"Rename...\")",
        "egui::Button::new(\"Edit...\")",
    ] {
        assert!(
            native_style_menu.contains(item),
            "missing native style item {item}"
        );
    }
}

#[test]
fn tauri_workspace_exposes_reference_visibility_and_lock_columns() {
    assert!(HTML.contains("id=\"workspace-tree-columns\""));
    assert!(HTML.contains("class=\"workspace-column-splitter\""));
    assert!(JS.contains("function workspaceStateControls("));
    assert!(JS.contains("mutate(\"set_workspace_visible\""));
    assert!(JS.contains("mutateWithoutViewport(\"set_workspace_locked\""));
    assert!(JS.contains("WORKSPACE_STATE_COLUMN_WIDTH_STORAGE_KEY"));
    assert!(JS.contains("function setupWorkspaceColumnSplitter()"));
    assert!(JS.contains("setWorkspaceStateColumnWidth(width, true)"));
    assert!(JS.contains("aria-checked"));
    assert!(CSS.contains(".workspace-state-controls"));
    assert!(CSS.contains(".workspace-state-toggle"));
    assert!(CSS.contains("var(--workspace-state-column-width, 34px)"));
    assert!(CSS.contains(".workspace-column-splitter"));
    assert!(CSS.contains("position: absolute;"));
    let native_workspace = NATIVE_GUI_RS
        .split("fn workspace_section(")
        .nth(1)
        .and_then(|source| source.split("fn workspace_aux_renderer_group_name(").next())
        .expect("native Workspace panel");
    assert!(native_workspace.contains("RichText::new(\"V\").strong()"));
    assert!(native_workspace.contains("RichText::new(\"Name\").strong()"));
    assert!(native_workspace.contains("workspace_panel:state_width"));
    assert!(native_workspace.contains("splitter.drag_delta().x"));
    assert!(native_workspace.contains("egui::vec2(state_width, ui.spacing().interact_size.y)"));
}

#[test]
fn tauri_projection_toggle_uses_the_compact_viewport_response() {
    assert!(JS.contains("snapshot.perspective = result.perspective"));
    assert!(JS.contains("renderProjectionControls()"));
    assert!(!JS.contains(r#"snapshot = await invoke("get_snapshot")"#));
}

#[test]
fn tauri_trajectory_frames_use_one_compact_ipc_round_trip() {
    assert!(JS.contains("invoke(\"set_trajectory_frame_viewport\""));
    assert!(JS.contains("trajectory.currentFrame = result.frame"));
    assert!(JS.contains("applyViewport(result.viewport, sequence)"));
}

#[test]
fn tauri_coloring_updates_use_one_compact_ipc_round_trip() {
    assert!(JS.contains(r#"const result = await invoke("set_coloring", {"#));
    assert!(JS.contains("applyViewport(result.viewport, sequence)"));
    assert!(!JS.contains(r#"return mutate("set_coloring""#));
}

#[test]
fn tauri_undo_redo_toolbar_matches_the_reference_history_contract() {
    for id in ["undo-history", "redo-history"] {
        assert!(HTML.contains(&format!("id=\"{id}\"")));
    }
    for label in ["Undo history", "Redo history"] {
        assert!(HTML.contains(&format!("aria-label=\"{label}\"")));
    }
    assert!(HTML.contains("data-action=\"undo\" data-depth=\"0\""));
    assert!(HTML.contains("data-action=\"redo\" data-depth=\"0\""));
    assert!(CSS.contains(".history-split.open > .history-popup"));
    assert!(JS.contains("snapshot.undoDescriptions"));
    assert!(JS.contains("snapshot.redoDescriptions"));
    assert!(JS.contains("await mutate(kind, { depth })"));
}

#[test]
fn tauri_help_surfaces_use_live_runtime_information() {
    for command in ["get_application_info", "set_config_value"] {
        assert!(JS.contains(command), "missing Help IPC binding {command}");
    }
    for heading in [
        "Registered I/O plugins",
        "Application preferences",
        "Built-in components",
    ] {
        assert!(JS.contains(heading), "missing Help surface {heading}");
    }
    for selector in [".plugin-catalog", ".config-browser", ".addon-components"] {
        assert!(CSS.contains(selector), "missing Help styling {selector}");
    }
}

#[test]
fn tauri_workspace_properties_match_the_reference_renderer_pages() {
    assert!(HTML.contains("id=\"workspace-properties\""));
    for command in [
        "set_object_properties",
        "set_renderer_properties",
        "set_density_map_properties",
        "set_density_renderer_properties",
        "set_density_renderer_visible",
        "set_surface_properties",
        "set_surface_renderer_properties",
        "set_surface_renderer_visible",
        "set_workspace_visible",
        "set_workspace_items_visible",
        "set_workspace_locked",
        "get_renderer_style_options",
        "create_renderer_style",
        "move_workspace_object",
        "move_workspace_renderer",
        "set_workspace_renderer_selection",
        "change_workspace_renderer_type",
        "create_workspace_renderer",
        "apply_workspace_renderer_style",
        "set_workspace_renderer_coloring",
        "insert_workspace_renderer_paint",
        "insert_workspace_object_paint",
        "copy_workspace_renderer",
        "copy_workspace_renderers",
        "paste_workspace_renderers",
        "copy_workspace_object",
        "paste_workspace_object",
        "copy_workspace_camera",
        "paste_workspace_camera",
        "copy_workspace_style",
        "paste_workspace_style",
        "create_workspace_renderer_group",
        "rename_workspace_node",
        "delete_workspace_item",
        "delete_workspace_items",
        "fit_workspace_item",
    ] {
        assert!(
            JS.contains(command),
            "missing Workspace IPC binding {command}"
        );
    }
    for label in [
        "Object properties",
        "Renderer properties",
        "Basic settings",
        "Edge lines:",
        "Simple Rend Properties",
        "Ballstick renderer properties",
        "Ball & Stick",
        "Bond width:",
        "Show ring",
        "CPK Rend Properties",
        "Atom radii",
        "OpenGL rendering:",
        "Name label renderer properties",
        "Maximum labels:",
        "Font family:",
        "Font style:",
        "Font weight:",
        "Disorder renderer properties",
        "Dot separation:",
        "Loop size 2:",
        "Tube Rend Properties",
        "Nucl renderer properties",
        "Ribbon representation properties",
        "Cartoon representation properties",
        "Contour renderer properties",
        "Isosurf renderer properties",
        "MolSurf Rend Properties",
        "Drawing Mode:",
        "Line/Point size:",
        "Surface type:",
        "Show selected:",
        "Selection mol:",
        "Coloring mode:",
        "Center update:",
        "Buffer size:",
        "Max grid size:",
        "Back-face culling",
        "Limit display by",
        "Helix section",
        "Sheet section",
        "Coil section",
        "Backbone spline",
        "Base width (%):",
        "Anchor selection",
        "Draw helix as ribbon",
        "Cylindrical helix",
        "Rounded square",
        "Axial detail:",
        "Use a custom pivot atom",
        "Connect base pair",
        "Thickness (%):",
        "Reset all to default",
    ] {
        assert!(
            JS.contains(label),
            "missing Workspace property field {label}"
        );
    }
    assert!(JS.contains("selectedRendererUid"));
    assert!(HTML
        .contains("id=\"workspace-delete\" type=\"button\" data-action=\"delete-workspace-item\""));
    assert!(
        HTML.contains("id=\"workspace-zoom\" type=\"button\" data-action=\"fit-workspace-item\"")
    );
    assert!(JS.contains("showReferenceDialog(\"Object Properties\")"));
    assert!(JS.contains("showReferenceDialog(\"Renderer Properties\")"));
    assert!(JS.contains("rendererPropertiesEditor(object, renderer, styleOptions)"));
    assert!(JS.contains("style: style.value"));
    assert!(CSS.contains(".workspace-property-editor"));
    assert!(CSS.contains(".renderer-style-editor"));
    assert!(CSS.contains(".property-common-group"));
    assert!(CSS.contains(".renderer-row.selected"));
    assert!(CSS.contains(".tree-object.density-map"));
    assert!(CSS.contains(".tree-renderer.density-renderer"));
    assert!(CSS.contains(".tree-object.surface-object"));
    assert!(CSS.contains(".tree-renderer.surface-renderer"));
}

#[test]
fn tauri_renderer_picker_exposes_the_reference_molecular_catalog() {
    assert!(JS.contains("const MOLECULE_RENDERER_TYPES = ["));
    assert!(
        JS.contains("MOLECULE_RENDERER_TYPES.map((rendererType) => [rendererType, rendererType])")
    );
    for renderer_type in cuemol_desktop::MOLECULE_RENDERER_TYPES {
        assert!(
            HTML.contains(&format!(
                "<option value=\"{renderer_type}\">{renderer_type}</option>"
            )),
            "missing initial renderer choice {renderer_type}"
        );
        assert!(
            JS.contains(&format!("\"{renderer_type}\",")),
            "missing dynamic renderer choice {renderer_type}"
        );
    }
}

#[test]
fn tauri_view_properties_match_the_reference_generic_editor() {
    for command in ["get_view_properties", "set_view_properties"] {
        assert!(
            JS.contains(command),
            "missing View property IPC binding {command}"
        );
    }
    for column in [
        r#"["Name", "R", "Type", "Value"]"#,
        "centerMark",
        "stereoMode",
    ] {
        assert!(JS.contains(column), "missing View property field {column}");
    }
    assert!(JS.contains("generic-property-editor"));
    assert!(CSS.contains(".property-table"));
    assert!(CSS.contains(".property-detail"));
    assert!(JS.contains("cameraCommand(`center-mark-${element.dataset.centerMark}`)"));
    assert!(JS.contains("result.centerMark === \"crosshair\""));
    assert!(!JS.contains("dialogParagraph(`Projection:"));
}

#[test]
fn tauri_scene_properties_match_the_reference_scene_qif_editor() {
    assert!(JS.contains("set_scene_properties"));
    for property in [
        "bgcolor",
        "aoEnabled",
        "aoRadius",
        "aoIntensity",
        "aoSlices",
        "aoSteps",
        "aoHalfRes",
        "aa_method",
        "aaJitterLevel",
        "onload",
        "use_colproof",
        "icc_filename",
        "icc_intent",
    ] {
        assert!(
            JS.contains(property),
            "missing Scene.qif property {property}"
        );
    }
    assert!(JS.contains("scenePropertyEditor(scene)"));
    assert!(JS.contains("genericPropertyEditor(info, \"scene\")"));
    assert!(CSS.contains(".property-color-editor"));
    assert!(!JS.contains("[\"Objects\", scene?.objectCount"));
}

#[test]
fn tauri_options_match_the_reference_misc_key_and_mouse_panes() {
    assert!(HTML.contains("data-dialog=\"Options\">Options…</button>"));
    for command in ["get_preferences", "set_preferences"] {
        assert!(
            JS.contains(command),
            "missing Options IPC binding {command}"
        );
    }
    for pane in ["Misc", "Key", "Mouse"] {
        assert!(JS.contains(&format!("createPane(\"{pane}\")")));
    }
    for preset in [
        "DefaultViewInConf",
        "CootViewInConf",
        "PymolViewInConf",
        "IphoneViewInConf",
        "TrackpadViewInConf",
    ] {
        assert!(JS.contains(preset), "missing mouse preset {preset}");
    }
    assert!(JS.contains("preferences = initial.preferences"));
    assert!(JS.contains("preferences?.trackballRadius"));
    assert!(JS.contains("preferences?.useHidpi"));
    assert!(JS.contains("preferences?.shortcuts?.find"));
    assert!(CSS.contains(".options-tabs"));
    assert!(CSS.contains(".shortcut-table"));
}

#[test]
fn tauri_global_shortcuts_match_the_reference_keyset() {
    let shortcuts = JS
        .split("window.addEventListener(\"keydown\", (event) => {")
        .nth(1)
        .and_then(|source| {
            source
                .split("document.addEventListener(\"pointerdown\"")
                .next()
        })
        .expect("global keydown handler");

    for key in ["n", "t", "o", "r", "s", "z", "y", "k"] {
        assert!(
            shortcuts.contains(&format!("key === \"{key}\"")),
            "missing reference shortcut {key}"
        );
    }
    assert!(shortcuts.contains("handleLocalAction(\"new-window\")"));
    assert!(shortcuts.contains("handleAction(\"new-scene\")"));
    assert!(shortcuts.contains("event.shiftKey && key === \"o\""));
    assert!(shortcuts.contains("openScene()"));
    assert!(shortcuts.contains("handleLocalAction(\"reload-scene\")"));
    assert!(shortcuts.contains("handleAction(\"undo\")"));
    assert!(shortcuts.contains("handleAction(\"redo\")"));
    assert!(shortcuts.contains("showReferenceDialog(\"Options\")"));
    assert!(shortcuts.contains("event.key === \"F2\""));
    assert!(shortcuts.contains("handleLocalAction(\"performance-measure\")"));
    assert!(shortcuts.contains("!editing && primary"));
    assert!(NATIVE_GUI_RS.contains("fn native_window_command("));
    assert!(NATIVE_GUI_RS.contains("std::env::current_exe()"));
    assert!(NATIVE_GUI_RS.contains("Opened new window (PID {})"));
    assert!(!NATIVE_GUI_RS.contains("New window mapped to a new scene"));
}
