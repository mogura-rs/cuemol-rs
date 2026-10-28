use cuemol_core::{CueMolError, Result};

#[cfg(not(target_arch = "wasm32"))]
mod implementation {
    use super::*;
    use std::ffi::{c_char, c_void, CStr};
    unsafe extern "C" {
        fn cuemol_cms_create(bytes: *const u8, length: u32, intent: u32) -> *mut c_void;
        fn cuemol_cms_destroy(handle: *mut c_void);
        fn cuemol_cms_color(handle: *mut c_void, rgb: u32, gamut: i32) -> u32;
        fn cuemol_cms_info(handle: *mut c_void, kind: u32, output: *mut c_char, length: u32)
            -> u32;
    }
    pub fn create(bytes: &[u8], intent: u32) -> Result<(usize, String)> {
        let handle = unsafe { cuemol_cms_create(bytes.as_ptr(), bytes.len() as u32, intent) };
        if handle.is_null() {
            return Err(CueMolError::invalid_argument(
                "LittleCMS rejected the ICC profile",
            ));
        }
        let info = (0..4)
            .filter_map(|kind| {
                let length = unsafe { cuemol_cms_info(handle, kind, std::ptr::null_mut(), 0) };
                if length == 0 {
                    return None;
                }
                let mut text = vec![0; length as usize];
                unsafe {
                    cuemol_cms_info(handle, kind, text.as_mut_ptr(), length);
                }
                let text = unsafe { CStr::from_ptr(text.as_ptr()) }
                    .to_string_lossy()
                    .trim()
                    .to_string();
                (!text.is_empty()).then_some(text)
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok((handle as usize, info))
    }
    pub fn destroy(handle: usize) {
        unsafe {
            cuemol_cms_destroy(handle as *mut c_void);
        }
    }
    pub fn color(handle: usize, rgb: u32, gamut: bool) -> u32 {
        unsafe { cuemol_cms_color(handle as *mut c_void, rgb, i32::from(gamut)) }
    }
}
#[cfg(target_arch = "wasm32")]
mod implementation {
    use super::*;
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = cuemolKernels, js_name = cmsCreate, catch)]
        fn create_transform(bytes: &[u8], intent: u32) -> std::result::Result<u32, JsValue>;
        #[wasm_bindgen(js_namespace = cuemolKernels, js_name = cmsDestroy)]
        fn destroy_transform(handle: u32);
        #[wasm_bindgen(js_namespace = cuemolKernels, js_name = cmsColor)]
        fn transform_color(handle: u32, rgb: u32, gamut: bool) -> u32;
        #[wasm_bindgen(js_namespace = cuemolKernels, js_name = cmsInfo)]
        fn transform_info(handle: u32) -> String;
    }
    pub fn create(bytes: &[u8], intent: u32) -> Result<(usize, String)> {
        let handle = create_transform(bytes, intent)
            .map_err(|e| CueMolError::invalid_argument(format!("ICC profile: {e:?}")))?;
        Ok((handle as usize, transform_info(handle)))
    }
    pub fn destroy(handle: usize) {
        destroy_transform(handle as u32);
    }
    pub fn color(handle: usize, rgb: u32, gamut: bool) -> u32 {
        transform_color(handle as u32, rgb, gamut)
    }
}
pub use implementation::*;
