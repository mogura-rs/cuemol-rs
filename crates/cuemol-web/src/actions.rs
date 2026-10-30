use super::*;
use cuemol_command::console::actions::{self, LibraryFile};
use cuemol_ui::actions::Intent;

fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .ok_or("Browser storage is unavailable.".into())
}

fn user_files() -> Result<BTreeMap<String, String>, String> {
    storage()?
        .get_item("cuemol.actions")
        .map_err(js_error)?
        .map(|source| serde_json::from_str(&source).map_err(|error| error.to_string()))
        .unwrap_or(Ok(BTreeMap::new()))
}

pub(super) fn read_library() -> Vec<LibraryFile> {
    let mut files = vec![LibraryFile::presets()];
    for (name, source) in user_files().unwrap_or_default() {
        files.push(LibraryFile::parse(&name, source, false));
    }
    files
}

impl App {
    pub(super) fn apply_action_intent(&mut self, intent: Intent) {
        let result = (|| -> Result<(), String> {
            match intent {
                Intent::MoveTo(index) => {
                    if self.busy || self.console_run.is_some() {
                        return Err("Wait for the current action or stop it first.".into());
                    }
                    let index = index.min(self.history.len() + self.future.len());
                    while self.history.len() > index {
                        self.call("undo", json!({}));
                    }
                    while self.history.len() < index {
                        self.call("redo", json!({}));
                    }
                }
                Intent::Run {
                    source,
                    macro_name,
                    label,
                } => {
                    self.run_library_action(&source, macro_name.as_deref(), &label)?;
                }
                Intent::Export { name, source } => {
                    download(&name, source.as_bytes(), "application/xml").map_err(js_error)?
                }
                Intent::Import => choose_file(),
                Intent::Refresh => {}
                Intent::Save { name, source } => {
                    let file_name = actions::validate_file_name(&name)?;
                    let name = file_name.trim_end_matches(".qax").to_string();
                    let file = LibraryFile::parse(&name, source.clone(), false);
                    if let Some(error) = file.error {
                        return Err(error);
                    }
                    let mut files = user_files()?;
                    if files.contains_key(&name) {
                        return Err("That action already exists. Choose another name.".into());
                    }
                    files.insert(name, source);
                    storage()?
                        .set_item("cuemol.actions", &serde_json::to_string(&files).unwrap())
                        .map_err(js_error)?;
                    self.actions_panel.saved();
                }
                Intent::Rename { from, to } => {
                    let to = actions::validate_file_name(&to)?
                        .trim_end_matches(".qax")
                        .to_string();
                    let mut files = user_files()?;
                    if files.contains_key(&to) {
                        return Err("That action already exists. Choose another name.".into());
                    }
                    let source = files.remove(&from).ok_or("Action not found.")?;
                    files.insert(to, source);
                    storage()?
                        .set_item("cuemol.actions", &serde_json::to_string(&files).unwrap())
                        .map_err(js_error)?;
                    self.actions_panel.saved();
                }
                Intent::Delete(name) => {
                    let mut files = user_files()?;
                    files.remove(&name);
                    storage()?
                        .set_item("cuemol.actions", &serde_json::to_string(&files).unwrap())
                        .map_err(js_error)?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.actions_panel.error = error.clone();
            self.message = error;
        }
        self.actions_panel.files = read_library();
    }
}
