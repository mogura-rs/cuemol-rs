#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn prepare_file(name: &str, bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    cuemol_desktop_core::PreparedFile::parse(name, bytes)
        .and_then(|file| file.encode())
        .map_err(|error| JsValue::from_str(&error.to_string()))
}
