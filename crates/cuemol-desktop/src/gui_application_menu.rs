use super::*;
use crate::application_menu::{self, MenuState, Node, Role};
use muda::{CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};
use std::cell::Cell;
use std::collections::BTreeMap;

thread_local! {
    static CURRENT_MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
    static NATIVE_DIALOG_DEPTH: Cell<usize> = const { Cell::new(0) };
}

pub(super) struct ApplicationMenu {
    menu: Menu,
    entries: BTreeMap<String, MenuItemKind>,
    state: MenuState,
    receiver: mpsc::Receiver<String>,
}

impl ApplicationMenu {
    pub fn new(proxy: EventLoopProxy<()>, state: MenuState) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
            let id = event.id.0;
            if NATIVE_DIALOG_DEPTH.with(|depth| depth.get() > 0) {
                if application_menu::text_action(&id) {
                    application_menu::native_text_edit(&id);
                }
                return;
            }
            if sender.send(id).is_ok() {
                let _ = proxy.send_event(());
            }
        }));
        let mut value = Self {
            menu: Menu::new(),
            entries: BTreeMap::new(),
            state,
            receiver,
        };
        value.rebuild()?;
        Ok(value)
    }

    fn rebuild(&mut self) -> Result<()> {
        let menu = Menu::new();
        let mut entries = BTreeMap::new();
        for node in application_menu::items(&self.state) {
            let item = build(&node, &mut entries)?;
            menu.append(item_ref(&item)).map_err(gui_error)?;
        }
        menu.init_for_nsapp();
        CURRENT_MENU.with(|current| *current.borrow_mut() = Some(menu.clone()));
        self.menu = menu;
        self.entries = entries;
        self.update_items();
        Ok(())
    }

    fn update_items(&self) {
        fn update(nodes: &[Node], entries: &BTreeMap<String, MenuItemKind>, blocked: bool) {
            for node in nodes {
                match node {
                    Node::Item {
                        id,
                        checked,
                        enabled,
                        ..
                    } => {
                        if let Some(item) = entries.get(id) {
                            set_enabled(item, *enabled);
                            if let (MenuItemKind::Check(item), Some(checked)) = (item, checked) {
                                item.set_checked(*checked);
                            }
                        }
                    }
                    Node::Group { id, items, .. } => {
                        if let Some(item) = entries.get(*id) {
                            set_enabled(item, !blocked || *id == "edit");
                        }
                        update(items, entries, blocked);
                    }
                    _ => {}
                }
            }
        }
        update(
            &application_menu::items(&self.state),
            &self.entries,
            self.state.blocked,
        );
    }

    pub fn update(&mut self, state: MenuState) -> Result<()> {
        if self.state == state {
            return Ok(());
        }
        let rebuild = self.state.recent != state.recent;
        self.state = state;
        if rebuild {
            self.rebuild()
        } else {
            self.update_items();
            Ok(())
        }
    }

    pub fn take_actions(&self) -> Vec<String> {
        let actions: Vec<_> = self.receiver.try_iter().collect();
        if !actions.is_empty() {
            // AppKit toggles check items even when selecting the current radio choice.
            self.update_items();
        }
        actions
    }
}

fn build(node: &Node, entries: &mut BTreeMap<String, MenuItemKind>) -> Result<MenuItemKind> {
    let value = match node {
        Node::Separator => MenuItemKind::Predefined(PredefinedMenuItem::separator()),
        Node::Role(role) => MenuItemKind::Predefined(match role {
            Role::Services => PredefinedMenuItem::services(None),
            Role::Hide => {
                PredefinedMenuItem::hide(Some(&format!("Hide {}", cuemol_command::about::PRODUCT)))
            }
            Role::HideOthers => PredefinedMenuItem::hide_others(None),
            Role::ShowAll => PredefinedMenuItem::show_all(None),
        }),
        Node::Item {
            id,
            label,
            accelerator,
            checked,
            enabled,
        } => {
            let accelerator = accelerator.map(str::parse).transpose().map_err(gui_error)?;
            match checked {
                Some(checked) => MenuItemKind::Check(CheckMenuItem::with_id(
                    id,
                    label,
                    *enabled,
                    *checked,
                    accelerator,
                )),
                None => MenuItemKind::MenuItem(MenuItem::with_id(id, label, *enabled, accelerator)),
            }
        }
        Node::Group { id, label, items } => {
            let submenu = Submenu::with_id(*id, label, true);
            for node in items {
                let item = build(node, entries)?;
                submenu.append(item_ref(&item)).map_err(gui_error)?;
            }
            MenuItemKind::Submenu(submenu)
        }
    };
    entries.insert(value.id().0.clone(), value.clone());
    Ok(value)
}

fn set_enabled(item: &MenuItemKind, enabled: bool) {
    match item {
        MenuItemKind::MenuItem(item) => item.set_enabled(enabled),
        MenuItemKind::Check(item) => item.set_enabled(enabled),
        MenuItemKind::Submenu(item) => item.set_enabled(enabled),
        _ => {}
    }
}

fn item_ref(item: &MenuItemKind) -> &dyn muda::IsMenuItem {
    match item {
        MenuItemKind::MenuItem(item) => item,
        MenuItemKind::Check(item) => item,
        MenuItemKind::Submenu(item) => item,
        MenuItemKind::Predefined(item) => item,
        MenuItemKind::Icon(item) => item,
    }
}

pub(super) struct NativeDialogGuard(Vec<(MenuItemKind, bool)>);

impl NativeDialogGuard {
    pub fn new() -> Self {
        NATIVE_DIALOG_DEPTH.with(|depth| depth.set(depth.get() + 1));
        fn block(items: Vec<MenuItemKind>, saved: &mut Vec<(MenuItemKind, bool)>) {
            for item in items {
                if application_menu::text_action(&item.id().0) {
                    continue;
                }
                let enabled = match &item {
                    MenuItemKind::MenuItem(item) => item.is_enabled(),
                    MenuItemKind::Check(item) => item.is_enabled(),
                    MenuItemKind::Submenu(submenu) => {
                        block(submenu.items(), saved);
                        if submenu.id().0 == "edit" {
                            continue;
                        }
                        submenu.is_enabled()
                    }
                    _ => continue,
                };
                set_enabled(&item, false);
                saved.push((item, enabled));
            }
        }
        let mut saved = Vec::new();
        CURRENT_MENU.with(|current| {
            if let Some(menu) = current.borrow().as_ref() {
                block(menu.items(), &mut saved);
            }
        });
        Self(saved)
    }
}

impl Drop for NativeDialogGuard {
    fn drop(&mut self) {
        for (item, enabled) in &self.0 {
            set_enabled(item, *enabled);
        }
        NATIVE_DIALOG_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

impl NativeRunner {
    pub(super) fn sync_application_menu(&mut self) {
        let (Some(app), Some(gpu)) = (&mut self.app, &self.gpu) else {
            return;
        };
        let state = app.application_menu_state(&self.egui_ctx, &gpu.camera);
        let result = if let Some(menu) = &mut self.application_menu {
            menu.update(state)
        } else {
            ApplicationMenu::new(self.event_proxy.clone(), state).map(|menu| {
                self.application_menu = Some(menu);
                app.native_application_menu = true;
                self.egui_ctx.request_repaint();
            })
        };
        if let Err(error) = result {
            app.set_error(error);
        }
    }

    pub(super) fn poll_application_menu(&mut self) {
        let actions = self
            .application_menu
            .as_ref()
            .map(ApplicationMenu::take_actions)
            .unwrap_or_default();
        for id in actions {
            let (Some(app), Some(gpu)) = (&mut self.app, &mut self.gpu) else {
                continue;
            };
            let blocked = app.application_menu_blocked(&self.egui_ctx);
            let rendering = self
                .render_window
                .as_mut()
                .filter(|window| window.visible && app.ray_panel.focused);
            if let Some(rendering) = rendering {
                if !application_menu::allowed(&id, blocked, rendering.wants_keyboard_input()) {
                    continue;
                }
                if application_menu::text_action(&id) {
                    rendering.menu_edit(app, &id);
                    continue;
                }
                if id == "close-tab" {
                    app.close_native_render(&gpu.camera);
                    continue;
                }
            }
            if !application_menu::allowed(&id, blocked, app.clipboard_focus.text.is_some()) {
                continue;
            }
            app.application_menu_action(&self.egui_ctx, &mut gpu.camera, &id);
            if !application_menu::text_action(&id)
                && !matches!(
                    id.as_str(),
                    "image-render" | "movie-render" | "window-render"
                )
            {
                app.main_focus_requested = true;
            }
            self.egui_ctx.request_repaint();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}
