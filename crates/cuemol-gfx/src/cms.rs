use crate::rgba_code;
use cuemol_core::{CueMolError, Result};
use libloading::Library;
use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

type Profile = *mut c_void;
type Transform = *mut c_void;

type CreateSrgbProfile = unsafe extern "C" fn() -> Profile;
type OpenProfileFromFile = unsafe extern "C" fn(*const c_char, *const c_char) -> Profile;
type CloseProfile = unsafe extern "C" fn(Profile) -> i32;
type CreateProofingTransform =
    unsafe extern "C" fn(Profile, u32, Profile, u32, Profile, u32, u32, u32) -> Transform;
type DeleteTransform = unsafe extern "C" fn(Transform);
type DoTransform = unsafe extern "C" fn(Transform, *const c_void, *mut c_void, u32);
type SetAlarmCodes = unsafe extern "C" fn(*const u16);
type GetProfileInfoAscii =
    unsafe extern "C" fn(Profile, i32, *const c_char, *const c_char, *mut c_char, u32) -> u32;

const TYPE_RGB_8: u32 = (4 << 16) | (3 << 3) | 1;
const INTENT_ABSOLUTE_COLORIMETRIC: u32 = 3;
const FLAGS_NOCACHE: u32 = 0x0040;
const FLAGS_GAMUTCHECK: u32 = 0x1000;
const FLAGS_SOFTPROOFING: u32 = 0x4000;
const MAX_CHANNELS: usize = 16;

struct LcmsApi {
    _library: Library,
    create_srgb_profile: CreateSrgbProfile,
    open_profile_from_file: OpenProfileFromFile,
    close_profile: CloseProfile,
    create_proofing_transform: CreateProofingTransform,
    delete_transform: DeleteTransform,
    do_transform: DoTransform,
    set_alarm_codes: SetAlarmCodes,
    get_profile_info_ascii: GetProfileInfoAscii,
}

impl LcmsApi {
    fn load() -> Result<Self> {
        let candidates = [
            "liblcms2.so.2",
            "liblcms2.so",
            "liblcms2.2.dylib",
            "liblcms2.dylib",
            "/opt/homebrew/opt/little-cms2/lib/liblcms2.2.dylib",
            "/usr/local/opt/little-cms2/lib/liblcms2.2.dylib",
            "lcms2.dll",
        ];
        let mut last_error = String::new();
        for candidate in candidates {
            let library = match unsafe { Library::new(candidate) } {
                Ok(library) => library,
                Err(error) => {
                    last_error = error.to_string();
                    continue;
                }
            };
            return unsafe { Self::from_library(library) };
        }
        Err(CueMolError::not_found(format!(
            "LittleCMS 2 shared library: {last_error}"
        )))
    }

    unsafe fn from_library(library: Library) -> Result<Self> {
        unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T> {
            unsafe { library.get::<T>(name) }
                .map(|symbol| *symbol)
                .map_err(|error| CueMolError::not_found(error.to_string()))
        }

        Ok(Self {
            create_srgb_profile: symbol(&library, b"cmsCreate_sRGBProfile\0")?,
            open_profile_from_file: symbol(&library, b"cmsOpenProfileFromFile\0")?,
            close_profile: symbol(&library, b"cmsCloseProfile\0")?,
            create_proofing_transform: symbol(&library, b"cmsCreateProofingTransform\0")?,
            delete_transform: symbol(&library, b"cmsDeleteTransform\0")?,
            do_transform: symbol(&library, b"cmsDoTransform\0")?,
            set_alarm_codes: symbol(&library, b"cmsSetAlarmCodes\0")?,
            get_profile_info_ascii: symbol(&library, b"cmsGetProfileInfoASCII\0")?,
            _library: library,
        })
    }
}

struct CmsHandles {
    api: LcmsApi,
    transform: Transform,
    gamut_transform: Transform,
}

impl Drop for CmsHandles {
    fn drop(&mut self) {
        unsafe {
            if !self.transform.is_null() {
                (self.api.delete_transform)(self.transform);
            }
            if !self.gamut_transform.is_null() {
                (self.api.delete_transform)(self.gamut_transform);
            }
        }
    }
}

unsafe impl Send for CmsHandles {}
unsafe impl Sync for CmsHandles {}

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
        let api = LcmsApi::load()?;
        let path = CString::new(path.as_ref().to_string_lossy().as_bytes())
            .map_err(|_| CueMolError::invalid_argument("ICC path contains a NUL byte"))?;
        let read_mode = c"r";
        let input = unsafe { (api.create_srgb_profile)() };
        if input.is_null() {
            return Err(CueMolError::Io(
                "LittleCMS could not create the sRGB profile".to_string(),
            ));
        }
        let proof = unsafe { (api.open_profile_from_file)(path.as_ptr(), read_mode.as_ptr()) };
        if proof.is_null() {
            unsafe {
                (api.close_profile)(input);
            }
            return Err(CueMolError::Io(format!(
                "cannot open ICC profile: {}",
                path.to_string_lossy()
            )));
        }

        let info = profile_info(&api, proof);
        let alarm = [0xffff, 0, 0xffff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        unsafe {
            (api.set_alarm_codes)(alarm.as_ptr());
        }
        let intent = self.intent.clamp(0, 3) as u32;
        let transform = unsafe {
            (api.create_proofing_transform)(
                input,
                TYPE_RGB_8,
                input,
                TYPE_RGB_8,
                proof,
                intent,
                INTENT_ABSOLUTE_COLORIMETRIC,
                FLAGS_SOFTPROOFING,
            )
        };
        let gamut_transform = unsafe {
            (api.create_proofing_transform)(
                input,
                TYPE_RGB_8,
                input,
                TYPE_RGB_8,
                proof,
                intent,
                INTENT_ABSOLUTE_COLORIMETRIC,
                FLAGS_SOFTPROOFING | FLAGS_GAMUTCHECK | FLAGS_NOCACHE,
            )
        };
        unsafe {
            (api.close_profile)(input);
            (api.close_profile)(proof);
        }
        if transform.is_null() || gamut_transform.is_null() {
            unsafe {
                if !transform.is_null() {
                    (api.delete_transform)(transform);
                }
                if !gamut_transform.is_null() {
                    (api.delete_transform)(gamut_transform);
                }
            }
            return Err(CueMolError::Io(
                "LittleCMS could not create the proofing transform".to_string(),
            ));
        }

        self.handles = Some(Arc::new(CmsHandles {
            api,
            transform,
            gamut_transform,
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
        let input = [
            ((code >> 16) & 0xff) as u8,
            ((code >> 8) & 0xff) as u8,
            (code & 0xff) as u8,
        ];
        let mut output = [0_u8; 3];
        unsafe {
            (handles.api.do_transform)(
                handles.transform,
                input.as_ptr().cast(),
                output.as_mut_ptr().cast(),
                1,
            );
        }
        rgba_code(output[0], output[1], output[2], ((code >> 24) & 0xff) as u8)
    }

    pub fn is_in_gamut(&self, code: u32) -> bool {
        let Some(handles) = self.handles.as_ref().filter(|_| self.enabled) else {
            return true;
        };
        let input = [
            ((code >> 16) & 0xff) as u8,
            ((code >> 8) & 0xff) as u8,
            (code & 0xff) as u8,
        ];
        let alarm_rgb = if input == [0xff, 0, 0xff] {
            [0_u16, 0xffff, 0xffff]
        } else {
            [0xffff_u16, 0, 0xffff]
        };
        let mut alarm = [0_u16; MAX_CHANNELS];
        alarm[..3].copy_from_slice(&alarm_rgb);
        let mut output = [0_u8; 3];
        unsafe {
            (handles.api.set_alarm_codes)(alarm.as_ptr());
            (handles.api.do_transform)(
                handles.gamut_transform,
                input.as_ptr().cast(),
                output.as_mut_ptr().cast(),
                1,
            );
        }
        output
            != [
                (alarm_rgb[0] >> 8) as u8,
                (alarm_rgb[1] >> 8) as u8,
                (alarm_rgb[2] >> 8) as u8,
            ]
    }
}

fn profile_info(api: &LcmsApi, profile: Profile) -> String {
    [0, 1, 2, 3]
        .into_iter()
        .filter_map(|info_type| {
            let length = unsafe {
                (api.get_profile_info_ascii)(
                    profile,
                    info_type,
                    c"en".as_ptr(),
                    c"US".as_ptr(),
                    std::ptr::null_mut(),
                    0,
                )
            };
            if length == 0 {
                return None;
            }
            let mut buffer = vec![0_i8; length as usize];
            unsafe {
                (api.get_profile_info_ascii)(
                    profile,
                    info_type,
                    c"en".as_ptr(),
                    c"US".as_ptr(),
                    buffer.as_mut_ptr(),
                    length,
                );
            }
            let value = unsafe { CStr::from_ptr(buffer.as_ptr()) }
                .to_string_lossy()
                .trim()
                .to_string();
            (!value.is_empty()).then_some(value)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn missing_profile_is_reported() {
        let mut transform = CmsXform::new();
        assert!(transform
            .load_icc_file("/path/that/does/not/exist.icc")
            .is_err());
        assert!(!transform.is_prof_ok());
    }

    #[test]
    fn generic_cmyk_fixture_uses_littlecms_when_available() {
        let path = Path::new("../cuemol2/src/xul_gui/data/icc_profiles/GenericCMYK.icm");
        if !path.is_file() {
            return;
        }
        let mut transform = CmsXform::new();
        transform.load_icc_file(path).unwrap();
        assert!(transform.is_prof_ok());
        assert!(!transform.info().is_empty());
        assert_ne!(transform.do_xform(0xff00_00ff), 0xff00_00ff);
    }
}
