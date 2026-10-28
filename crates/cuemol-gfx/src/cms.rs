use crate::rgba_code;
use cuemol_core::{CueMolError, Result, Vector4};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
#[path = "cms_platform.rs"]
mod platform;

struct CmsHandles {
    raw: usize,
    colors: Mutex<BTreeMap<u32, u32>>,
}
impl Drop for CmsHandles {
    fn drop(&mut self) {
        platform::destroy(self.raw);
    }
}

#[derive(Clone)]
pub struct CmsXform {
    handles: Option<Arc<CmsHandles>>,
    enabled: bool,
    intent: i32,
    info: String,
}
impl fmt::Debug for CmsXform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CmsXform")
            .field("profile_loaded", &self.is_prof_ok())
            .field("enabled", &self.enabled)
            .field("intent", &self.intent)
            .field("info", &self.info)
            .finish()
    }
}
impl Default for CmsXform {
    fn default() -> Self {
        Self::new()
    }
}
impl CmsXform {
    pub fn new() -> Self {
        Self {
            handles: None,
            enabled: true,
            intent: 0,
            info: String::new(),
        }
    }
    pub fn load_icc_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let name = path.as_ref().to_string_lossy();
        if let Some(bytes) = profiles().lock().unwrap().get(name.as_ref()).cloned() {
            return self.load_icc_bytes(&bytes);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.load_icc_bytes(&std::fs::read(path).map_err(|e| CueMolError::Io(e.to_string()))?)
        }
        #[cfg(target_arch = "wasm32")]
        {
            Err(CueMolError::not_found(format!(
                "Import the ICC profile first: {name}"
            )))
        }
    }
    pub fn load_icc_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() < 128 || bytes.len() > u32::MAX as usize {
            return Err(CueMolError::invalid_argument("invalid ICC profile size"));
        }
        let (raw, info) = platform::create(bytes, self.intent as u32)?;
        self.handles = Some(Arc::new(CmsHandles {
            raw,
            colors: Mutex::new(BTreeMap::new()),
        }));
        self.info = info;
        Ok(())
    }
    pub fn reset(&mut self) {
        self.handles = None;
        self.info.clear();
    }
    pub fn is_prof_ok(&self) -> bool {
        self.handles.is_some()
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn icc_intent(&self) -> i32 {
        self.intent
    }
    pub fn get_icc_intent(&self) -> i32 {
        self.icc_intent()
    }
    pub fn set_icc_intent(&mut self, intent: i32) {
        self.intent = intent.clamp(0, 3);
    }
    pub fn info(&self) -> &str {
        &self.info
    }
    pub fn get_info(&self) -> &str {
        self.info()
    }
    pub fn do_xform(&self, code: u32) -> u32 {
        let Some(handles) = self.handles.as_ref().filter(|_| self.enabled) else {
            return code;
        };
        let rgb = code & 0x00ff_ffff;
        let mut colors = handles.colors.lock().unwrap();
        let transformed = if let Some(value) = colors.get(&rgb) {
            *value
        } else {
            let value = platform::color(handles.raw, rgb, false);
            if colors.len() >= 4096 {
                colors.clear();
            }
            colors.insert(rgb, value);
            value
        };
        (code & 0xff00_0000) | transformed
    }
    pub fn is_in_gamut(&self, code: u32) -> bool {
        let Some(handles) = self.handles.as_ref().filter(|_| self.enabled) else {
            return true;
        };
        let _guard = handles.colors.lock().unwrap();
        platform::color(handles.raw, code & 0x00ff_ffff, true) != 0
    }
}

type ProfileStore = BTreeMap<String, Arc<[u8]>>;
fn profiles() -> &'static Mutex<ProfileStore> {
    static PROFILES: OnceLock<Mutex<ProfileStore>> = OnceLock::new();
    PROFILES.get_or_init(|| Mutex::new(BTreeMap::new()))
}
static PROFILE_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn register_icc_profile(name: &str, bytes: &[u8]) -> Result<()> {
    let mut transform = CmsXform::new();
    transform.load_icc_bytes(bytes)?;
    profiles()
        .lock()
        .unwrap()
        .insert(name.to_string(), Arc::from(bytes));
    PROFILE_REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}
pub fn icc_profile_revision() -> u64 {
    PROFILE_REVISION.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone, Default)]
pub struct ColProfMgr {
    transforms: BTreeMap<u64, CmsXform>,
}

impl ColProfMgr {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_cms(&mut self, scene_id: u64) {
        self.transforms.entry(scene_id).or_default();
    }

    pub fn unregister_cms(&mut self, scene_id: u64) {
        self.transforms.remove(&scene_id);
    }

    pub fn cms_by_id(&self, scene_id: u64) -> Option<&CmsXform> {
        self.transforms.get(&scene_id)
    }

    pub fn cms_by_id_mut(&mut self, scene_id: u64) -> Option<&mut CmsXform> {
        self.transforms.get_mut(&scene_id)
    }

    pub fn len(&self) -> usize {
        self.transforms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transforms.is_empty()
    }

    pub fn do_xform(&self, scene_id: u64, color: u32) -> u32 {
        self.cms_by_id(scene_id)
            .filter(|transform| transform.is_prof_ok())
            .map_or(color, |transform| transform.do_xform(color))
    }

    pub fn is_in_gamut(&self, scene_id: u64, color: u32) -> bool {
        self.cms_by_id(scene_id)
            .filter(|transform| transform.is_prof_ok())
            .is_none_or(|transform| transform.is_in_gamut(color))
    }

    pub fn do_xform_vector(&self, scene_id: u64, color: Vector4) -> Vector4 {
        let code = rgba_code(
            float_color_component(color.x),
            float_color_component(color.y),
            float_color_component(color.z),
            float_color_component(color.w),
        );
        let transformed = self.do_xform(scene_id, code);
        Vector4::new4(
            f64::from((transformed >> 16) as u8) / 255.0,
            f64::from((transformed >> 8) as u8) / 255.0,
            f64::from(transformed as u8) / 255.0,
            f64::from((transformed >> 24) as u8) / 255.0,
        )
    }
}

pub fn global_color_profile_manager() -> &'static Mutex<ColProfMgr> {
    static MANAGER: OnceLock<Mutex<ColProfMgr>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(ColProfMgr::new()))
}

pub fn register_color_profile_scene(scene_id: u64) {
    if let Ok(mut manager) = global_color_profile_manager().lock() {
        manager.register_cms(scene_id);
    }
}

pub fn unregister_color_profile_scene(scene_id: u64) {
    if let Ok(mut manager) = global_color_profile_manager().lock() {
        manager.unregister_cms(scene_id);
    }
}

fn float_color_component(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    fn srgb_profile() -> Vec<u8> {
        use std::ffi::c_void;
        unsafe extern "C" {
            fn cmsCreate_sRGBProfile() -> *mut c_void;
            fn cmsSaveProfileToMem(
                profile: *mut c_void,
                memory: *mut c_void,
                size: *mut u32,
            ) -> i32;
            fn cmsCloseProfile(profile: *mut c_void) -> i32;
        }
        unsafe {
            let profile = cmsCreate_sRGBProfile();
            assert!(!profile.is_null());
            let mut size = 0;
            assert_eq!(
                cmsSaveProfileToMem(profile, std::ptr::null_mut(), &mut size),
                1
            );
            let mut bytes = vec![0; size as usize];
            assert_eq!(
                cmsSaveProfileToMem(profile, bytes.as_mut_ptr().cast(), &mut size),
                1
            );
            cmsCloseProfile(profile);
            bytes
        }
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn embedded_icc_profile_roundtrips_colors_and_rejects_bad_data() {
        let bytes = srgb_profile();
        let mut transform = CmsXform::new();
        transform.load_icc_bytes(&bytes).unwrap();
        assert!(transform.is_prof_ok());
        assert!(transform.info().contains("sRGB"));
        for rgb in [0x804080c0, 0xffffffff, 0xff000000, 0x20ff00ff] {
            let result = transform.do_xform(rgb);
            assert_eq!(result >> 24, rgb >> 24);
            for shift in [0, 8, 16] {
                assert!(
                    (((result >> shift) & 255) as i32 - ((rgb >> shift) & 255) as i32).abs() <= 1
                );
            }
            assert!(transform.is_in_gamut(rgb));
        }
        assert!(transform.load_icc_bytes(&vec![0; 128]).is_err());
        assert!(transform.is_prof_ok());
        register_icc_profile("generated-srgb.icc", &bytes).unwrap();
        let mut registered = CmsXform::new();
        registered.load_icc_file("generated-srgb.icc").unwrap();
        assert_eq!(
            registered.do_xform(0xff4080c0),
            transform.do_xform(0xff4080c0)
        );
    }

    #[test]
    fn unloaded_transform_is_identity() {
        let mut transform = CmsXform::new();
        assert_eq!(transform.do_xform(0x8040_80c0), 0x8040_80c0);
        assert!(transform.is_in_gamut(0x8040_80c0));
        assert!(!transform.is_prof_ok());
        transform.set_icc_intent(99);
        assert_eq!(transform.get_icc_intent(), 3);
    }

    #[test]
    fn profile_manager_registration_and_unloaded_fallback_match_reference() {
        let mut manager = ColProfMgr::new();
        manager.register_cms(10);
        manager.register_cms(10);
        assert_eq!(manager.len(), 1);
        assert!(manager.cms_by_id(10).is_some());
        assert_eq!(manager.do_xform(10, 0x8040_80ff), 0x8040_80ff);
        assert_eq!(
            manager.do_xform_vector(10, Vector4::new4(0.25, 0.5, 1.0, 0.5)),
            Vector4::new4(64.0 / 255.0, 128.0 / 255.0, 1.0, 128.0 / 255.0)
        );
        manager.unregister_cms(10);
        assert!(manager.is_empty());
    }

    #[test]
    fn missing_profile_is_reported() {
        let mut transform = CmsXform::new();
        assert!(transform
            .load_icc_file("missing-color-profile.icc")
            .is_err());
        assert!(!transform.is_prof_ok());
    }

    #[test]
    fn generic_cmyk_fixture_uses_littlecms_when_available() {
        let reference = std::env::var_os("CUEMOL2_REF_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
        let path = reference.join("src/xul_gui/data/icc_profiles/GenericCMYK.icm");
        if !path.is_file() {
            return;
        }
        let mut transform = CmsXform::new();
        transform.load_icc_file(path).unwrap();
        assert!(transform.is_prof_ok());
        assert!(!transform.info().is_empty());
        for (input, expected) in [
            (0xffffffff, 0xfff2f3fa),
            (0xff777777, 0xff767578),
            (0xff0000ff, 0xff345ba0),
            (0xffff00ff, 0xffb469a4),
        ] {
            assert_eq!(transform.do_xform(input), expected);
        }
    }
}
