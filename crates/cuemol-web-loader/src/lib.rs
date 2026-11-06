#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn prepare_file(name: &str, bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    cuemol_desktop_core::PreparedFile::parse(name, bytes)
        .and_then(|file| file.encode())
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn prepare_pdb_file(name: &str, bytes: &[u8], kind: &str) -> Result<Vec<u8>, JsValue> {
    use cuemol_desktop_core::GuiPdbFileKind;
    let kind = match kind {
        "coordinates" => GuiPdbFileKind::Coordinates,
        "2fofc" => GuiPdbFileKind::TwoFoFc,
        "fofc" => GuiPdbFileKind::FoFc,
        "emdb" => GuiPdbFileKind::Emdb,
        _ => return Err(JsValue::from_str("Invalid PDB file kind")),
    };
    cuemol_desktop_core::PreparedFile::parse_pdb_download(name, bytes, kind)
        .and_then(|file| file.encode())
        .map_err(|error| JsValue::from_str(&error.to_string()))
}
