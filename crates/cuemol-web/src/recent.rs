use super::*;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(super) struct RecentFile {
    pub id: String,
    pub name: String,
    pub persistent: bool,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=cuemolFiles, js_name=register)]
    pub(super) fn register(file: &web_sys::File);
    #[wasm_bindgen(js_namespace=cuemolFiles, js_name=choose)]
    pub(super) fn choose();
    #[wasm_bindgen(js_namespace=cuemolFiles, js_name=reopen)]
    pub(super) fn reopen(id: &str);
    #[wasm_bindgen(js_namespace=cuemolFiles, js_name=clear)]
    pub(super) fn clear();
    #[wasm_bindgen(js_namespace=cuemolFiles, js_name=remember)]
    fn remember(file: web_sys::File);
}

#[wasm_bindgen]
pub fn recent_files_changed(files: String) -> Result<(), JsValue> {
    let files =
        serde_json::from_str(&files).map_err(|error| JsValue::from_str(&error.to_string()))?;
    enqueue(Event::RecentFiles(files));
    Ok(())
}

#[wasm_bindgen]
pub fn file_picker_error(error: String) {
    enqueue(Event::FilePickerError(error));
}

impl App {
    pub(super) fn remember_source(&mut self) {
        if let Some(file) = self.source_recent.take() {
            remember(file);
        }
    }
}
