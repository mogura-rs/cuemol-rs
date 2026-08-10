const HTML: &str = include_str!("../tauri-ui/index.html");
const CSS: &str = include_str!("../tauri-ui/styles.css");
const JS: &str = include_str!("../tauri-ui/app.js");
const CAPABILITIES: &str = include_str!("../capabilities/default.json");

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
        "select_screen_rect",
        "add_animation_object",
        "animation_control",
        "tick_animation",
        "tick_animation_frame",
        "superpose_objects",
        "edit_bond",
        "calculate_contacts",
        "assign_secondary_structure",
        "write_surface",
        "write_cut_surface",
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
    assert!(JS.contains("mutateWithViewport(\"open_paths\", { paths: objectPaths })"));
    assert!(JS.contains("snapshot?.openedPaths"));
}

#[test]
fn tauri_reload_scene_uses_the_active_scene_source() {
    assert!(HTML.contains("data-local-action=\"reload-scene\""));
    assert!(JS.contains("scene?.modified"));
    assert!(JS.contains("window.confirm(`Do you want to reload the scene:"));
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
    assert!(JS.contains("scheduleMoleculeDetailsRefresh(delay = 0)"));
    assert!(JS.contains("scheduleMoleculeDetailsRefresh()"));
    assert!(JS.contains("includeSequence"));
    assert!(JS.contains("initial.startupPathsPending"));
    assert!(JS.contains("function scheduleSecondaryPanelRender()"));
    assert!(JS.contains("globalThis.requestIdleCallback(render, { timeout: 100 })"));
    let render_shell = JS
        .split("function renderShell()")
        .nth(1)
        .and_then(|source| source.split("function renderUndoControls").next())
        .expect("renderShell function");
    assert!(render_shell.contains("scheduleSecondaryPanelRender()"));
    assert!(!render_shell.contains("renderColoring()"));
    assert!(!render_shell.contains("renderTrajectory()"));
    assert!(JS.contains("void mutateWithViewport(\"open_initial_paths\")"));
    assert!(JS.contains("mutateWithViewport(\"open_paths\""));
    assert!(JS.contains("mutateWithViewport(\"open_pdb\""));
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
    assert!(JS.contains("label: \"New Camera...\""));
    assert!(JS.contains("Save from scene (with vis flags)"));
    assert!(JS.contains("Apply to scene (with vis flags)"));
    assert!(JS.contains("workspaceStyleContextItems"));
    assert!(JS.contains("function updateWorkspaceStyle(style, update)"));
    assert!(JS.contains("label: \"New Style...\""));
    assert!(JS.contains("label: \"Read-only\""));
    assert!(JS.contains("label: \"Rename...\""));
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
    assert!(JS.contains("mutate(\"destroy_camera\""));
    assert!(JS.contains("invoke(\"get_camera_properties\""));
    assert!(JS.contains("invoke(\"set_camera_properties\""));
    assert!(JS.contains("camera.visibilityCount > 0"));
}

#[test]
fn tauri_workspace_exposes_reference_style_nodes_and_actions() {
    assert!(JS.contains("function activeStyles()"));
    assert!(JS.contains("workspaceSelectionKind === \"styleRoot\""));
    assert!(JS.contains("workspaceSelectionKind === \"style\""));
    assert!(JS.contains("mutate(\"create_style\""));
    assert!(JS.contains("mutate(\"destroy_style\""));
    assert!(JS.contains("invoke(\"get_style_properties\""));
    assert!(JS.contains("invoke(\"set_style_properties\""));
    assert!(JS.contains("async function rendererStyleCreateEditor(object, renderer)"));
    assert!(JS.contains("mutateWithoutViewport(\"create_renderer_style\""));
    assert!(JS.contains("Create Renderer Style"));
    assert!(JS.contains("Target Style set:"));
    assert!(JS.contains("Overwrite existing style"));
    assert!(CSS.contains(".renderer-style-name"));
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
