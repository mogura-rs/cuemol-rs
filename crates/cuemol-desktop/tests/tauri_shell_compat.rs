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
        "set_density_level",
        "set_background",
        "set_selection",
        "molecule_selection_action",
        "get_molecule_details",
        "open_initial_paths",
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
    assert!(JS.contains("selectedMolstructExpression()"));
    assert!(JS.contains("event.shiftKey"));
    assert!(JS.contains("composeSelectionEditorExpression()"));
    assert!(JS.contains("selectionActiveTab = element.dataset.selectionTab"));
    assert!(JS.contains("byres ((${selection}) around ${item.args})"));
    assert!(JS.contains("await applySelectionCommand()"));
    assert!(JS.contains("item.supportsColoring"));
    assert!(JS.contains("PaintColoring"));
    assert!(JS.contains("CPKColoring"));
    assert!(JS.contains("RainbowColoring"));
    assert!(JS.contains("BfacColoring"));
    assert!(JS.contains(r#"extensions: ["dcd", "xtc", "trr", "nc"]"#));
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
        "write_surface",
        "cut_surface",
        "write_apbs_files",
        "append_morph_frame",
        "remove_morph_frame",
        "apply_morph_frame",
        "clear_log",
        "execute_script",
        "export_pov",
        "render_animation",
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
    assert!(JS.contains("sidePanel.hidden = !cards.some"));
    assert!(JS.contains("for (const name of openPanels) renderSidePanel(name)"));
    assert!(JS.contains("getAllWebviewWindows"));
    assert!(JS.contains("await appWindow.setFocus()"));
    assert!(CAPABILITIES.contains(r#""cuemol-*""#));
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
    assert!(CSS.contains(".reference-dialog.confirm-close-dialog"));
    assert!(NATIVE_GUI_RS.contains("fn close_tab_dialog"));
    assert!(NATIVE_GUI_RS.contains("close_after_scene_save"));
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
    assert_eq!(JS.matches("await showPngExportOptions(").count(), 2);
    assert!(CSS.contains(".reference-dialog.png-options-dialog"));
    assert!(CSS.contains("width: min(360px, calc(100vw - 48px))"));
    assert!(TAURI_APP_RS.contains("export_active_scene_png_with_options"));
    assert!(TAURI_APP_RS.contains("GuiPngExportOptions"));
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
        "Change symmetry…",
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
    assert!(JS.contains("groupName:"));
    assert!(CSS.contains(".tree-renderer.renderer-group"));
    assert!(CSS.contains(".tree-renderer.group-child"));
}

#[test]
fn tauri_workspace_reorders_and_regroups_renderers_by_dragging() {
    assert!(JS.contains("rendererRow.draggable = true"));
    assert!(JS.contains("application/x-cuemol-renderer"));
    assert!(JS.contains("function workspaceRendererDropPosition(event, renderer)"));
    assert!(JS.contains("mutateWithoutViewport(\"move_workspace_renderer\""));
    assert!(CSS.contains(".tree-renderer.drop-before::after"));
    assert!(CSS.contains(".tree-renderer.drop-inside"));
}

#[test]
fn tauri_workspace_reorders_objects_across_supported_kinds() {
    assert!(JS.contains("function configureWorkspaceObjectDrag(row, sceneUid, kind, uid)"));
    assert!(JS.contains("application/x-cuemol-object"));
    assert!(JS.contains("mutateWithoutViewport(\"move_workspace_object\""));
    assert!(JS.contains("workspaceObjectBlocks.sort("));
    assert!(CSS.contains(".tree-object.drop-before::after"));
}

#[test]
fn tauri_workspace_context_menu_matches_selection_presets() {
    assert!(JS.contains("function showWorkspaceContextMenu(event, items)"));
    assert!(JS.contains("function workspaceObjectContextItems(scene, object)"));
    assert!(JS.contains("function workspaceAuxObjectContextItems(scene, kind, object)"));
    assert!(JS.contains("function workspaceAuxRendererContextItems(scene, kind, object, renderer)"));
    assert!(JS.contains("Around Byresid"));
    assert!(JS.contains("function workspaceRendererContextItems(scene, object, renderer)"));
    assert!(JS.contains("set_workspace_renderer_selection"));
    assert!(JS.contains("change_workspace_renderer_type"));
    assert!(JS.contains("cachedWorkspaceRendererStyles"));
    assert!(JS.contains("apply_workspace_renderer_style"));
    assert!(JS.contains("set_workspace_renderer_coloring"));
    assert!(JS.contains("insert_workspace_renderer_paint"));
    assert!(JS.contains("copy_workspace_renderer"));
    assert!(JS.contains("paste_workspace_renderers"));
    assert!(JS.contains("copy_workspace_object"));
    assert!(JS.contains("paste_workspace_object"));
    assert!(JS.contains("copy_workspace_camera"));
    assert!(JS.contains("paste_workspace_camera"));
    assert!(JS.contains("copy_workspace_style"));
    assert!(JS.contains("paste_workspace_style"));
    assert!(JS.contains("workspaceSceneContextItems"));
    assert!(JS.contains("workspaceCameraContextItems"));
    assert!(JS.contains("label: \"New Camera…\""));
    assert!(JS.contains("label: \"Camera file\""));
    assert!(JS.contains("function loadWorkspaceCameraFile(scene)"));
    assert!(JS.contains("function saveWorkspaceCameraFile(scene, camera, saveAs)"));
    assert!(JS.contains("mutate(\"load_camera_file\""));
    assert!(JS.contains("mutateWithoutViewport(\"reload_camera_file\""));
    assert!(JS.contains("mutateWithoutViewport(\"save_camera_file\""));
    assert!(JS.contains("Save from scene (with vis flags)"));
    assert!(JS.contains("Apply to scene (with vis flags)"));
    assert!(JS.contains("workspaceStyleContextItems"));
    assert!(JS.contains("label: \"Style file\""));
    assert!(JS.contains("function loadWorkspaceStyleFile(scene)"));
    assert!(JS.contains("function saveWorkspaceStyleFile(scene, style, saveAs)"));
    assert!(JS.contains("mutate(\"load_style_file\""));
    assert!(JS.contains("mutateWithoutViewport(\"save_style_file\""));
    assert!(JS.contains("globalThis.alert?.(\"Not implemented!!\")"));
    assert!(JS.contains("function updateWorkspaceStyle(style, update)"));
    assert!(JS.contains("label: \"New Style…\""));
    assert!(JS.contains("async function openNewStyleFlow()"));
    assert!(JS.contains("title: \"New Style\""));
    assert!(JS.contains("label: \"Name for new style:\""));
    assert!(JS.contains("defaultValue: `style_${index}`"));
    assert!(NATIVE_GUI_RS.contains("ui.button(\"New Style…\")"));
    assert!(NATIVE_GUI_RS.contains("egui::Window::new(\"New Style\")"));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Name for new style:\")"));
    assert!(JS.contains("label: \"New Group…\""));
    assert!(JS.contains("title: \"New Renderer Group\""));
    assert!(JS.contains("label: \"Name for new group:\""));
    assert!(JS.contains("defaultValue: `group${serial}`"));
    assert!(!JS.contains("globalThis.prompt?.(\"Name for new group\""));
    assert!(JS.contains("label: \"Read-only\""));
    assert!(JS.contains("Background color · White"));
    assert!(JS.contains("Background color · Black"));
    assert!(JS.contains("option: \"color-proof\", value"));
    assert!(JS.contains("showReferenceDialog(\"Scene Properties\")"));
    assert!(JS.contains("role\", \"menuitemcheckbox"));
    assert!(JS.contains("workspaceRendererClipboardAvailable"));
    assert!(JS.contains("Paste Renderer"));
    assert!(JS.contains("Paste Object"));
    assert!(JS.contains("label: \"Save As ...\""));
    assert!(JS.contains("label: \"Delete Object\""));
    assert!(JS.contains("label: \"Generate surface obj\""));
    assert!(JS.contains("generate_density_surface"));
    assert!(JS.contains("label: \"Regenerate surface...\""));
    assert!(JS.contains("regenerate_surface"));
    assert!(CSS.contains(".workspace-context-menu.has-submenu"));
    assert!(JS.contains("WORKSPACE_PAINT_MONOCHROME"));
    assert!(JS.contains("Paint · ${group} · ${label}"));
    assert!(JS.contains("[\"B-factor\", \"bfactor\"]"));
    assert!(JS.contains("Style · ${style.description || style.name}"));
    assert!(JS.contains("Edit interaction list..."));
    assert!(JS.contains("kind: \"renderer\""));
    assert!(JS.contains("create_workspace_renderer_group"));
    assert!(JS.contains("rename_workspace_renderer_group"));
    assert!(JS.contains("Change Name..."));
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
    assert!(JS.contains("label: \"Edit vis flags…\""));
    assert!(JS.contains("label: \"Clear vis flags\""));
    assert!(JS.contains("label: \"Save from view\""));
    assert!(JS.contains("label: \"Apply to view\""));
    assert!(JS.contains("camera.visibilityCount > 0"));
    assert!(CSS.contains(".visibility-flag-editor"));
    assert!(CSS.contains("width: min(420px, calc(100vw - 48px))"));
    assert!(JS.contains("function showTextPromptDialog({"));
    assert!(JS.contains("async function openNewCameraFlow()"));
    assert!(JS.contains("title: \"New Camera\""));
    assert!(JS.contains("label: \"Name for new camera:\""));
    assert!(JS.contains("defaultValue: `camera_${index}`"));
    assert!(JS.contains("confirmLabel: \"Create\""));
    assert!(JS.contains("mutateWithoutViewport(\"create_camera\", { name }, () => {})"));
    assert!(!JS.contains("globalThis.prompt?.(\"Camera name\""));
    assert!(CSS.contains(".reference-dialog.text-prompt-dialog"));
    assert!(TAURI_APP_RS.contains("ModelRequest::CreateCamera(name)"));
    assert!(TAURI_APP_RS.contains("fn create_camera(name: String"));
    assert!(NATIVE_GUI_RS.contains("struct NativeNewCameraDraft"));
    assert!(NATIVE_GUI_RS.contains("fn new_camera_dialog("));
    assert!(NATIVE_GUI_RS.contains("ui.label(\"Name for new camera:\")"));
    assert!(NATIVE_GUI_RS.contains("egui::Button::new(\"Create\")"));
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
    assert!(tauri_camera_menu.contains("camera?.name || \"Cameras\""));
    assert!(tauri_camera_menu.contains("label: \"Paste Camera\""));
    assert!(tauri_camera_menu.contains("label: \"Load…\""));
    assert!(tauri_camera_menu.contains("label: \"Save As…\""));
    let tauri_camera_root = tauri_camera_menu
        .split("if (!camera) {")
        .nth(1)
        .and_then(|source| source.split("return items;").next())
        .expect("Tauri camera-root context menu");
    assert!(tauri_camera_root.contains("children: [loadCamera]"));
    assert!(!tauri_camera_root.contains("label: \"Reload\""));
    let clear_action = tauri_camera_menu
        .split("label: \"Clear vis flags\"")
        .nth(1)
        .and_then(|source| source.split("label: \"Rename…\"").next())
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
    assert!(native_camera_root.contains("ui.weak(\"Cameras\")"));
    assert!(native_camera_root.contains("ui.button(\"Paste Camera\")"));
    assert!(native_camera_root.contains("ui.button(\"Load…\")"));
    assert!(!native_camera_root.contains("Button::new(\"Reload\")"));
}

#[test]
fn tauri_workspace_exposes_reference_style_nodes_and_actions() {
    assert!(JS.contains("function activeStyles()"));
    assert!(JS.contains("workspaceSelectionKind === \"styleRoot\""));
    assert!(JS.contains("workspaceSelectionKind === \"style\""));
    assert!(JS.contains("mutateWithoutViewport(\"create_style\""));
    assert!(JS.contains("mutate(\"destroy_style\""));
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
    assert!(tauri_style_menu.contains("style?.name || \"Styles\""));
    assert!(tauri_style_menu.contains("label: \"Paste Style\""));
    assert!(tauri_style_menu.contains("label: \"Load…\""));
    assert!(tauri_style_menu.contains("label: \"Save As…\""));
    assert!(tauri_style_menu.contains("label: \"Edit…\""));
    assert!(tauri_style_menu.contains("label: \"Properties…\""));
    assert!(!tauri_style_menu.contains("label: \"Rename...\""));
    assert!(!tauri_style_menu.contains("globalThis.prompt"));
    let tauri_style_root = tauri_style_menu
        .split("if (!style) {")
        .nth(1)
        .and_then(|source| source.split("return items;").next())
        .expect("Tauri style-root context menu");
    assert!(tauri_style_root.contains("children: [loadStyle]"));
    assert!(!tauri_style_root.contains("label: \"Reload\""));

    let native_style_menu = NATIVE_GUI_RS
        .split("let root_selection = WorkspaceAuxSelection::StyleRoot")
        .nth(1)
        .and_then(|source| {
            source
                .split("ui.separator();\n        ui.horizontal")
                .next()
        })
        .expect("native style context menus");
    assert!(native_style_menu.contains("ui.weak(\"Styles\")"));
    assert!(native_style_menu.contains("ui.button(\"Paste Style\")"));
    assert!(native_style_menu.contains("ui.button(\"Load…\")"));
    assert!(native_style_menu.contains("ui.button(\"Edit…\")"));
    assert!(native_style_menu.contains("ui.button(\"Properties…\")"));
    assert!(!native_style_menu.contains("egui::Button::new(\"Rename...\")"));
}

#[test]
fn tauri_workspace_exposes_reference_visibility_and_lock_columns() {
    assert!(JS.contains("function workspaceStateControls("));
    assert!(JS.contains("mutate(\"set_workspace_visible\""));
    assert!(JS.contains("mutateWithoutViewport(\"set_workspace_locked\""));
    assert!(JS.contains("aria-checked"));
    assert!(CSS.contains(".workspace-state-controls"));
    assert!(CSS.contains(".workspace-state-toggle"));
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
        "add_density_renderer",
        "set_surface_properties",
        "set_surface_renderer_properties",
        "set_surface_renderer_visible",
        "add_surface_renderer",
        "set_workspace_visible",
        "set_workspace_locked",
        "get_renderer_style_options",
        "create_renderer_style",
        "move_workspace_object",
        "move_workspace_renderer",
        "set_workspace_renderer_selection",
        "change_workspace_renderer_type",
        "apply_workspace_renderer_style",
        "set_workspace_renderer_coloring",
        "insert_workspace_renderer_paint",
        "copy_workspace_renderer",
        "paste_workspace_renderers",
        "copy_workspace_object",
        "paste_workspace_object",
        "copy_workspace_camera",
        "paste_workspace_camera",
        "copy_workspace_style",
        "paste_workspace_style",
        "create_workspace_renderer_group",
        "rename_workspace_renderer_group",
        "delete_workspace_item",
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
