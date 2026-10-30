use super::*;
use cuemol_command::console::actions::{self, Record};
use serde_json::{json, Value};

impl AppModel {
    fn actions_directory(&self) -> PathBuf {
        self.config_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(Path::to_path_buf)
            .unwrap_or_else(cuemol_command::config::directory)
            .join("actions")
    }

    fn selected_action_records(&self, args: &Value) -> Result<Vec<Record>, String> {
        let ids = args["ids"]
            .as_array()
            .ok_or("Select history steps.")?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .map(|id| id as usize)
                    .ok_or("Invalid step ID.")
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let records: Vec<_> = self
            .undo_stack
            .iter()
            .chain(self.redo_stack.iter().rev())
            .filter(|entry| ids.contains(&entry.record.id()))
            .map(|entry| entry.record.clone())
            .collect();
        if records.len() != ids.len() || records.is_empty() {
            return Err("The selected history steps are no longer available.".into());
        }
        Ok(records)
    }

    pub(super) fn actions_request(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let directory = self.actions_directory();
        if operation == "list" {
            let steps: Vec<_> = self.undo_stack.iter().map(|entry| (entry, false)).chain(self.redo_stack.iter().rev().map(|entry| (entry, true))).map(|(entry, undone)| {
                json!({"id":entry.record.id(),"label":entry.record.label(),"entryCount":entry.record.entry_count(),"undone":undone})
            }).collect();
            let files: Vec<_> = actions::read_library(&directory).into_iter().map(|file| json!({"key":file.key,"name":file.name,"bundled":file.bundled,"macros":file.macros,"actionCount":file.action_count,"error":file.error})).collect();
            return Ok(
                json!({"steps":steps,"files":files,"done":self.undo_stack.len(),"running":self.console_run.is_some()}),
            );
        }
        if operation == "inspect" {
            let id = args["id"].as_u64().ok_or("Select a history step.")? as usize;
            let record = self
                .undo_stack
                .iter()
                .chain(self.redo_stack.iter())
                .find(|entry| entry.record.id() == id)
                .ok_or("History step not found.")?;
            return serde_json::to_value(record.record.view()).map_err(|error| error.to_string());
        }
        if operation == "goto" {
            if self.console_run.is_some() {
                return Err("Wait for the current action or stop it first.".into());
            }
            let id = args["id"].as_u64().unwrap_or(0) as usize;
            let index = if id == 0 {
                0
            } else {
                self.undo_stack
                    .iter()
                    .chain(self.redo_stack.iter().rev())
                    .position(|entry| entry.record.id() == id)
                    .ok_or("History step not found.")?
                    + 1
            };
            if index < self.undo_stack.len() {
                self.undo(self.undo_stack.len() - index - 1)?;
            } else if index > self.undo_stack.len() {
                self.redo(index - self.undo_stack.len() - 1)?;
            }
            return Ok(json!({"snapshot":self.snapshot(),"running":false}));
        }
        if operation == "commands" || operation == "export_history" || operation == "save_history" {
            let records = self.selected_action_records(args)?;
            if operation == "commands" {
                return Ok(json!({"text":actions::commands(&records)}));
            }
            let source = if operation == "save_history" && args["asMacro"].as_bool().unwrap_or(true)
            {
                let name = args["name"].as_str().ok_or("Enter an action name.")?;
                actions::make_macro(&records, name, name, &self.desktop)?
            } else {
                actions::export(&records)
            };
            if operation == "save_history" {
                actions::save_library(
                    &directory,
                    args["name"].as_str().ok_or("Enter an action name.")?,
                    &source,
                )?;
            } else {
                fs::write(
                    args["path"].as_str().ok_or("Choose an output file.")?,
                    source,
                )
                .map_err(|error| error.to_string())?;
            }
            return Ok(json!({"ok":true}));
        }
        if operation == "import" {
            let path = Path::new(args["path"].as_str().ok_or("Choose a QAX file.")?);
            let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            actions::save_library(&directory, &name, &source)?;
            return Ok(json!({"ok":true}));
        }
        let file = actions::resolve_library_file(
            &directory,
            args["file"].as_str().ok_or("Choose an action file.")?,
            args["bundled"].as_bool().unwrap_or(false),
        )?;
        match operation {
            "run" => {
                let name = args["macro"].as_str();
                let targets: Vec<_> = args["targetIds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_u64)
                    .collect();
                let targets = actions::targets(&self.desktop, &targets)
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>();
                let label = name
                    .and_then(|name| file.macros.iter().find(|mac| mac.name == name))
                    .map(|mac| mac.label.as_str())
                    .unwrap_or(&file.name);
                serde_json::to_value(self.start_library_action(
                    &file.source,
                    name,
                    label,
                    &targets,
                )?)
                .map_err(|error| error.to_string())
            }
            "export_file" => {
                fs::write(
                    args["path"].as_str().ok_or("Choose an output file.")?,
                    &file.source,
                )
                .map_err(|error| error.to_string())?;
                Ok(json!({"ok":true}))
            }
            "copy" => {
                let source = args["macro"]
                    .as_str()
                    .map(|name| file.copy_macro(name))
                    .unwrap_or_else(|| Ok(file.source.clone()))?;
                actions::save_library(
                    &directory,
                    args["name"].as_str().ok_or("Enter an action name.")?,
                    &source,
                )?;
                Ok(json!({"ok":true}))
            }
            "rename" | "delete" => {
                if file.bundled {
                    return Err("Presets are read-only.".into());
                }
                if operation == "rename" {
                    actions::rename_library_file(
                        &directory,
                        &file.key,
                        args["name"].as_str().ok_or("Enter an action name.")?,
                    )?;
                } else {
                    actions::delete_library_file(&directory, &file.key)?;
                }
                Ok(json!({"ok":true}))
            }
            _ => Err(format!("Unknown action operation: {operation}")),
        }
    }
}
