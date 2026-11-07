use super::*;
use crate::application_menu::MenuState;

#[tauri::command]
pub(super) async fn application_menu_request(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    operation: String,
    state: Option<MenuState>,
    blocked: Option<bool>,
) -> Result<bool, String> {
    if operation == "supported" {
        return Ok(cfg!(target_os = "macos"));
    }
    if operation == "devtools" {
        if window.is_devtools_open() {
            window.close_devtools();
        } else {
            window.open_devtools();
        }
        return Ok(true);
    }
    #[cfg(target_os = "macos")]
    {
        let label = window.label().to_owned();
        return tauri::async_runtime::spawn_blocking(move || {
            let (reply, result) = mpsc::channel();
            let handle = app.clone();
            app.run_on_main_thread(move || {
                let outcome = platform::update(&handle, &label, &operation, state, blocked);
                let _ = reply.send(outcome);
            })
            .map_err(|error| error.to_string())?;
            result.recv().map_err(|error| error.to_string())?
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, state, blocked);
        Ok(false)
    }
}

pub(super) fn install(app: &mut tauri::App) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    platform::install(app)?;
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    Ok(())
}

pub(super) fn menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    #[cfg(target_os = "macos")]
    platform::dispatch(app, &event.id.0);
    #[cfg(not(target_os = "macos"))]
    let _ = (app, event);
}

pub(super) fn clear_window(app: &tauri::AppHandle, label: &str) {
    #[cfg(target_os = "macos")]
    {
        let handle = app.clone();
        let label = label.to_owned();
        let _ = app.run_on_main_thread(move || {
            if let Err(error) = platform::update(&handle, &label, "clear", None, None) {
                eprintln!("Cannot restore application menu: {error}");
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, label);
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use crate::application_menu::{self, Node, Role};
    use std::collections::BTreeMap;
    use tauri::menu::{CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};

    struct MenuModel {
        menu: Menu<tauri::Wry>,
        entries: BTreeMap<String, MenuItemKind<tauri::Wry>>,
        state: MenuState,
        blocked: BTreeMap<String, bool>,
        native_dialogs: BTreeMap<String, bool>,
    }
    struct MenuStore(Mutex<MenuModel>);

    impl MenuModel {
        fn effective(&self) -> MenuState {
            let mut state = self.state.clone();
            state.blocked |= self.blocked.values().any(|value| *value)
                || self.native_dialogs.values().any(|value| *value);
            state
        }
        fn rebuild(&mut self, app: &tauri::AppHandle) -> tauri::Result<()> {
            let menu = Menu::new(app)?;
            let mut entries = BTreeMap::new();
            for node in application_menu::webview_items(&self.effective()) {
                menu.append(&build(app, &node, &mut entries)?)?;
            }
            app.set_menu(menu.clone())?;
            self.menu = menu;
            self.entries = entries;
            self.refresh()
        }
        fn refresh(&self) -> tauri::Result<()> {
            fn apply(
                nodes: &[Node],
                entries: &BTreeMap<String, MenuItemKind<tauri::Wry>>,
                blocked: bool,
            ) -> tauri::Result<()> {
                for node in nodes {
                    match node {
                        Node::Item {
                            id,
                            enabled,
                            checked,
                            ..
                        } => {
                            if let Some(item) = entries.get(id) {
                                match item {
                                    MenuItemKind::MenuItem(item) => item.set_enabled(*enabled)?,
                                    MenuItemKind::Check(item) => {
                                        item.set_enabled(*enabled)?;
                                        if let Some(checked) = checked {
                                            item.set_checked(*checked)?;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Node::Group { id, items, .. } => {
                            if let Some(MenuItemKind::Submenu(item)) = entries.get(*id) {
                                item.set_enabled(!blocked || *id == "edit")?;
                            }
                            apply(items, entries, blocked)?;
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            let state = self.effective();
            apply(
                &application_menu::webview_items(&state),
                &self.entries,
                state.blocked,
            )
        }
    }

    pub fn install(app: &mut tauri::App) -> tauri::Result<()> {
        let mut model = MenuModel {
            menu: Menu::new(app)?,
            entries: BTreeMap::new(),
            state: MenuState {
                blocked: true,
                ..Default::default()
            },
            blocked: BTreeMap::new(),
            native_dialogs: BTreeMap::new(),
        };
        model.rebuild(app.handle())?;
        app.manage(MenuStore(Mutex::new(model)));
        Ok(())
    }

    fn build(
        app: &tauri::AppHandle,
        node: &Node,
        entries: &mut BTreeMap<String, MenuItemKind<tauri::Wry>>,
    ) -> tauri::Result<MenuItemKind<tauri::Wry>> {
        let value = match node {
            Node::Separator => MenuItemKind::Predefined(PredefinedMenuItem::separator(app)?),
            Node::Role(role) => MenuItemKind::Predefined(match role {
                Role::Services => PredefinedMenuItem::services(app, None)?,
                Role::Hide => PredefinedMenuItem::hide(
                    app,
                    Some(&format!("Hide {}", cuemol_command::about::PRODUCT)),
                )?,
                Role::HideOthers => PredefinedMenuItem::hide_others(app, None)?,
                Role::ShowAll => PredefinedMenuItem::show_all(app, None)?,
            }),
            Node::Item {
                id,
                label,
                enabled,
                checked,
                accelerator,
            } => match checked {
                Some(checked) => MenuItemKind::Check(CheckMenuItem::with_id(
                    app,
                    id,
                    label,
                    *enabled,
                    *checked,
                    *accelerator,
                )?),
                None => MenuItemKind::MenuItem(MenuItem::with_id(
                    app,
                    id,
                    label,
                    *enabled,
                    *accelerator,
                )?),
            },
            Node::Group { id, label, items } => {
                let submenu = Submenu::with_id(app, *id, label, true)?;
                for node in items {
                    submenu.append(&build(app, node, entries)?)?;
                }
                MenuItemKind::Submenu(submenu)
            }
        };
        entries.insert(value.id().0.clone(), value.clone());
        Ok(value)
    }

    pub fn update(
        app: &tauri::AppHandle,
        label: &str,
        operation: &str,
        state: Option<MenuState>,
        blocked: Option<bool>,
    ) -> Result<bool, String> {
        let store = app.state::<MenuStore>();
        let mut model = store.0.lock().map_err(|error| error.to_string())?;
        let mut rebuild = false;
        match operation {
            "state" if label == "main" => {
                let state = state.ok_or("Menu state is missing")?;
                rebuild = state.recent != model.state.recent;
                model.state = state;
            }
            "blocked" => {
                model.blocked.insert(label.into(), blocked.unwrap_or(false));
            }
            "native-dialog" => {
                model
                    .native_dialogs
                    .insert(label.into(), blocked.unwrap_or(false));
            }
            "clear" => {
                model.blocked.remove(label);
                model.native_dialogs.remove(label);
            }
            _ => return Err("Unknown application menu operation".into()),
        }
        if rebuild {
            model.rebuild(app)
        } else {
            model.refresh()
        }
        .map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub fn dispatch(app: &tauri::AppHandle, id: &str) {
        let Some(store) = app.try_state::<MenuStore>() else {
            return;
        };
        let Ok(model) = store.0.lock() else {
            return;
        };
        if !model.entries.contains_key(id) {
            return;
        }
        // Reconcile AppKit's automatic check toggle even if the model stays unchanged.
        if let Err(error) = model.refresh() {
            eprintln!("Cannot restore application menu checks: {error}");
        }
        if model.native_dialogs.values().any(|value| *value) {
            if application_menu::text_action(id) {
                application_menu::native_text_edit(id);
            }
            return;
        }
        if model.effective().blocked && !application_menu::text_action(id) {
            return;
        }
        drop(model);
        let focused = app
            .webview_windows()
            .into_values()
            .find(|window| window.is_focused().unwrap_or(false));
        if id == "devtools" {
            if let Some(window) = focused {
                if window.is_devtools_open() {
                    window.close_devtools();
                } else {
                    window.open_devtools();
                }
            }
            return;
        }
        let target = if application_menu::text_action(id) || id == "close-tab" {
            focused.or_else(|| app.get_webview_window("main"))
        } else {
            app.get_webview_window("main")
        };
        if let Some(window) = target {
            if !application_menu::text_action(id) {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            let _ = window.emit("cuemol-menu-action", id);
        }
    }
}
