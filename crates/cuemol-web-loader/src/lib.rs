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

#[wasm_bindgen]
pub fn decode_file_prefix(bytes: &[u8], limit: usize) -> Result<Vec<u8>, JsValue> {
    cuemol_core::compressed::decode_prefix(bytes, limit)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn mtz_header_offset(prefix: &[u8], file_size: usize) -> Result<usize, JsValue> {
    cuemol_xtal::mtz::mtz_header_offset(prefix, file_size)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn inspect_file(
    name: &str,
    prefix: &[u8],
    mtz_header: &[u8],
    file_size: usize,
) -> Result<String, JsValue> {
    let metadata = if prefix.starts_with(b"MTZ ") {
        Some(
            cuemol_xtal::mtz::read_mtz_metadata_parts(prefix, mtz_header, file_size)
                .map_err(|error| JsValue::from_str(&error.to_string()))?,
        )
    } else {
        None
    };
    let result = cuemol_desktop_core::GuiFileOpenMetadata::inspect(name, prefix, metadata.as_ref())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::to_string(&result).map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn prepare_file_with_options(
    name: &str,
    bytes: &[u8],
    options: &str,
    companion: &[u8],
    has_companion: bool,
) -> Result<Vec<u8>, JsValue> {
    let options =
        serde_json::from_str(options).map_err(|error| JsValue::from_str(&error.to_string()))?;
    cuemol_desktop_core::PreparedFile::parse_with_file_options(
        name,
        bytes,
        &options,
        has_companion.then_some(companion),
    )
    .and_then(|file| file.encode())
    .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn inspect_compressed_cif(name: &str, bytes: &[u8]) -> Result<String, JsValue> {
    let result = cuemol_desktop_core::GuiFileOpenMetadata::inspect_compressed_cif(name, bytes)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::to_string(&result).map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn inspect_compressed_mtz(name: &str, bytes: &[u8]) -> Result<String, JsValue> {
    let mtz = cuemol_xtal::mtz::read_mtz_metadata_compressed_bytes(bytes)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let metadata = cuemol_desktop_core::GuiFileOpenMetadata::inspect(name, b"MTZ ", Some(&mtz))
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::to_string(&metadata).map_err(|error| JsValue::from_str(&error.to_string()))
}
