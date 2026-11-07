use cuemol_command::about::PRODUCT;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "tauri-gui", derive(serde::Deserialize))]
#[cfg_attr(feature = "tauri-gui", serde(rename_all = "camelCase", default))]
pub(crate) struct MenuState {
    pub has_scene: bool,
    pub background: String,
    pub color_proofing: bool,
    pub perspective: bool,
    pub center_mark: String,
    pub palette_visible: bool,
    pub recent: Vec<String>,
    pub blocked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Services,
    Hide,
    HideOthers,
    ShowAll,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Node {
    Item {
        id: String,
        label: String,
        accelerator: Option<&'static str>,
        checked: Option<bool>,
        enabled: bool,
    },
    Group {
        id: &'static str,
        label: String,
        items: Vec<Node>,
    },
    Role(Role),
    Separator,
}

fn item(id: &str, label: impl Into<String>, accelerator: Option<&'static str>) -> Node {
    Node::Item {
        id: id.into(),
        label: label.into(),
        accelerator,
        checked: None,
        enabled: true,
    }
}

fn check(id: &str, label: &str, checked: bool, accelerator: Option<&'static str>) -> Node {
    Node::Item {
        id: id.into(),
        label: label.into(),
        accelerator,
        checked: Some(checked),
        enabled: true,
    }
}

fn group(id: &'static str, label: impl Into<String>, items: Vec<Node>) -> Node {
    Node::Group {
        id,
        label: label.into(),
        items,
    }
}

pub(crate) fn text_action(id: &str) -> bool {
    matches!(
        id,
        "cut" | "copy" | "paste" | "select-all" | "undo" | "redo"
    )
}

pub(crate) fn allowed(id: &str, blocked: bool, editing: bool) -> bool {
    !blocked || (text_action(id) && editing)
}

#[cfg(target_os = "macos")]
pub(crate) fn native_text_edit(id: &str) {
    use objc2::sel;
    let action = match id {
        "cut" => sel!(cut:),
        "copy" => sel!(copy:),
        "paste" => sel!(paste:),
        "select-all" => sel!(selectAll:),
        "undo" => sel!(undo:),
        "redo" => sel!(redo:),
        _ => return,
    };
    if let Some(main) = objc2_foundation::MainThreadMarker::new() {
        let app = objc2_app_kit::NSApplication::sharedApplication(main);
        unsafe {
            app.sendAction_to_from(action, None, None);
        }
    }
}

pub(crate) fn items(state: &MenuState) -> Vec<Node> {
    use Node::Separator as Sep;
    let mut recent = state
        .recent
        .iter()
        .map(|path| {
            let label = std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path);
            item(&format!("recent:{path}"), label, None)
        })
        .collect::<Vec<_>>();
    if !recent.is_empty() {
        recent.push(Sep);
    }
    recent.push(item("clear-recent", "Clear Menu", None));
    let mut menus = vec![
        group(
            "application",
            PRODUCT,
            vec![
                item("about", format!("About {PRODUCT}"), None),
                item("check-updates", "Check for Updates...", None),
                Sep,
                item("options", "Preferences...", Some("Cmd+,")),
                Sep,
                Node::Role(Role::Services),
                Sep,
                Node::Role(Role::Hide),
                Node::Role(Role::HideOthers),
                Node::Role(Role::ShowAll),
                Sep,
                item("quit", format!("Quit {PRODUCT}"), Some("Cmd+Q")),
            ],
        ),
        group(
            "file",
            "File",
            vec![
                item("new-tab", "New Tab", Some("Cmd+T")),
                Sep,
                item("open-file", "Open File...", Some("Cmd+O")),
                group("open-recent", "Open Recent", recent),
                Sep,
                item("save-file-as", "Save File As...", None),
                item("save-current-view", "Save current view...", None),
                Sep,
                item("close-tab", "Close Tab", Some("Cmd+W")),
                Sep,
                item("open-scene", "Open Scene...", Some("Cmd+Shift+O")),
                item("reload-scene", "Reload Scene", Some("Cmd+R")),
                item("save-scene", "Save Scene", Some("Cmd+S")),
                item("save-scene-as", "Save Scene As...", Some("Cmd+Shift+S")),
            ],
        ),
        group(
            "edit",
            "Edit",
            vec![
                item("undo", "Undo", Some("Cmd+Z")),
                item("redo", "Redo", Some("Cmd+Shift+Z")),
                Sep,
                item("cut", "Cut", Some("Cmd+X")),
                item("copy", "Copy", Some("Cmd+C")),
                item("paste", "Paste", Some("Cmd+V")),
                item("select-all", "Select All", Some("Cmd+A")),
                Sep,
                item("clear-undo", "Clear undo data", None),
                Sep,
                item("merge-mol", "Merge molecule...", None),
                item("delete-mol-atoms", "Delete mol atoms...", None),
                item("change-chain-id", "Change chain ID...", None),
                item("change-resid-num", "Change residue number...", None),
            ],
        ),
        group(
            "rendering",
            "Rendering",
            vec![
                item("image-render", "Image rendering...", None),
                item("movie-render", "Movie rendering...", None),
                group(
                    "export-scene",
                    "Export scene",
                    vec![
                        item("export-png", "PNG image...", None),
                        item("export-umbreon", "Umbreon ray-traced image...", None),
                        item("export-pov", "POV-Ray SDL...", None),
                        item("export-stl", "STL...", None),
                        item("export-mqo", "Metasequoia (MQO)...", None),
                    ],
                ),
            ],
        ),
        group(
            "scene",
            "Scene",
            vec![
                group(
                    "background",
                    "Background",
                    vec![
                        check("bg-white", "White", state.background == "white", None),
                        check("bg-black", "Black", state.background == "black", None),
                    ],
                ),
                Sep,
                check(
                    "color-proof",
                    "Use color proofing",
                    state.color_proofing,
                    None,
                ),
                item("scene-props", "Properties...", None),
            ],
        ),
        group(
            "view",
            "View",
            vec![
                check("view-perspective", "Perspective", state.perspective, None),
                check(
                    "view-orthographic",
                    "Orthographic",
                    !state.perspective,
                    None,
                ),
                Sep,
                group(
                    "center-mark",
                    "Center mark",
                    vec![
                        check(
                            "center-mark-cross",
                            "Cross",
                            state.center_mark == "cross",
                            None,
                        ),
                        check(
                            "center-mark-axis",
                            "Axis",
                            state.center_mark == "axis",
                            None,
                        ),
                        check(
                            "center-mark-none",
                            "None",
                            state.center_mark == "none",
                            None,
                        ),
                    ],
                ),
                Sep,
                check(
                    "view-tool-palette",
                    "Tool palette",
                    state.palette_visible,
                    Some("Cmd+Shift+P"),
                ),
                Sep,
                item("view-props", "View property...", None),
            ],
        ),
        group(
            "tools",
            "Tools",
            vec![
                item("mol-superpose", "Molecular superposition...", None),
                item("interaction", "Interaction...", None),
                item("reassign-2ndry", "Reassign secondary str...", None),
                item("morph-anim", "Mol morphing animation...", None),
                item("mol-surf", "Mol surface generation...", None),
                item("surf-cutter", "Mol surface cutter...", None),
                item("apbs", "APBS elepot calculation...", None),
            ],
        ),
        group(
            "window",
            "Window",
            vec![
                item("window-main", "Main Window", None),
                item("window-render", "Rendering Window", None),
            ],
        ),
    ];
    fn update(nodes: &mut [Node], state: &MenuState) {
        for node in nodes {
            match node {
                Node::Item {
                    id,
                    enabled,
                    checked,
                    ..
                } => {
                    *enabled = (!state.blocked || text_action(id))
                        && (checked.is_none() || id == "view-tool-palette" || state.has_scene)
                        && (id != "clear-recent" || !state.recent.is_empty());
                }
                Node::Group { items, .. } => update(items, state),
                _ => {}
            }
        }
    }
    update(&mut menus, state);
    menus
}

pub(crate) fn webview_items(state: &MenuState) -> Vec<Node> {
    let mut menus = items(state);
    if let Some(Node::Group { items, .. }) = menus
        .iter_mut()
        .find(|node| matches!(node, Node::Group { id: "view", .. }))
    {
        items.push(Node::Separator);
        let mut devtools = item("devtools", "Toggle Developer Tools", Some("Cmd+Alt+I"));
        if let Node::Item { enabled, .. } = &mut devtools {
            *enabled = !state.blocked;
        }
        items.push(devtools);
    }
    menus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(nodes: &'a [Node], wanted: &str) -> &'a Node {
        for node in nodes {
            match node {
                Node::Item { id, .. } if id == wanted => return node,
                Node::Group { items, .. } => {
                    if let Some(node) = find_optional(items, wanted) {
                        return node;
                    }
                }
                _ => {}
            }
        }
        panic!("missing menu item {wanted}")
    }

    fn find_optional<'a>(nodes: &'a [Node], wanted: &str) -> Option<&'a Node> {
        nodes.iter().find_map(|node| match node {
            Node::Item { id, .. } if id == wanted => Some(node),
            Node::Group { items, .. } => find_optional(items, wanted),
            _ => None,
        })
    }

    #[test]
    fn modal_block_preserves_text_edits_and_restores_view_checks() {
        let mut state = MenuState {
            has_scene: true,
            perspective: true,
            center_mark: "axis".into(),
            palette_visible: true,
            ..Default::default()
        };
        let before = items(&state);
        state.blocked = true;
        let blocked = items(&state);
        assert!(matches!(
            find(&blocked, "quit"),
            Node::Item { enabled: false, .. }
        ));
        assert!(matches!(
            find(&blocked, "paste"),
            Node::Item { enabled: true, .. }
        ));
        assert!(matches!(
            find(&blocked, "view-perspective"),
            Node::Item {
                enabled: false,
                checked: Some(true),
                ..
            }
        ));
        assert!(!allowed("quit", true, true));
        assert!(!allowed("cut", true, false));
        assert!(allowed("paste", true, true));
        state.blocked = false;
        assert_eq!(items(&state), before);
    }

    #[test]
    fn recent_actions_preserve_the_chosen_path_when_order_changes() {
        let mut state = MenuState {
            recent: vec!["a/scene.qsc".into(), "b/scene.qsc".into()],
            ..Default::default()
        };
        let before = find(&items(&state), "recent:a/scene.qsc").clone();
        state.recent.reverse();
        assert_eq!(&before, find(&items(&state), "recent:a/scene.qsc"));
        state.recent.clear();
        assert!(matches!(
            find(&items(&state), "clear-recent"),
            Node::Item { enabled: false, .. }
        ));
    }
}
