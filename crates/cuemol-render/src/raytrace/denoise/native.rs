use cuemol_core::{CueMolError, Result};
use libloading::Library;
use std::ffi::{c_char, c_void, CStr};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
type Handle = *mut c_void;
type SetImage = unsafe extern "C" fn(
    Handle,
    *const c_char,
    *mut c_void,
    i32,
    usize,
    usize,
    usize,
    usize,
    usize,
);
struct Context {
    _library: Library,
    device: Handle,
    filter: Handle,
    image: SetImage,
    commit: unsafe extern "C" fn(Handle),
    execute: unsafe extern "C" fn(Handle),
    error: unsafe extern "C" fn(Handle, *mut *const c_char) -> i32,
    release_filter: unsafe extern "C" fn(Handle),
    release_device: unsafe extern "C" fn(Handle),
}
// OIDN permits calls from different threads; the global mutex serializes each device.
unsafe impl Send for Context {}
impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            (self.release_filter)(self.filter);
            (self.release_device)(self.device);
        }
    }
}
fn error_text(error: *const c_char) -> String {
    if error.is_null() {
        "unknown device error".into()
    } else {
        unsafe { CStr::from_ptr(error).to_string_lossy().into_owned() }
    }
}
impl Context {
    unsafe fn new(library: Library) -> std::result::Result<Self, String> {
        unsafe {
            macro_rules! function {
                ($name:literal,$ty:ty) => {
                    *library
                        .get::<$ty>(concat!($name, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                };
            }
            let new_device = function!("oidnNewDevice", unsafe extern "C" fn(i32) -> Handle);
            let commit_device = function!("oidnCommitDevice", unsafe extern "C" fn(Handle));
            let new_filter = function!(
                "oidnNewFilter",
                unsafe extern "C" fn(Handle, *const c_char) -> Handle
            );
            let set_bool = function!(
                "oidnSetFilterBool",
                unsafe extern "C" fn(Handle, *const c_char, bool)
            );
            let set_int = function!(
                "oidnSetFilterInt",
                unsafe extern "C" fn(Handle, *const c_char, i32)
            );
            let image = function!("oidnSetSharedFilterImage", SetImage);
            let commit = function!("oidnCommitFilter", unsafe extern "C" fn(Handle));
            let execute = function!("oidnExecuteFilter", unsafe extern "C" fn(Handle));
            let error = function!(
                "oidnGetDeviceError",
                unsafe extern "C" fn(Handle, *mut *const c_char) -> i32
            );
            let release_filter = function!("oidnReleaseFilter", unsafe extern "C" fn(Handle));
            let release_device = function!("oidnReleaseDevice", unsafe extern "C" fn(Handle));
            let device = new_device(1);
            let mut message = std::ptr::null();
            if device.is_null() {
                error(device, &mut message);
                return Err(error_text(message));
            }
            commit_device(device);
            if error(device, &mut message) != 0 {
                let message = error_text(message);
                release_device(device);
                return Err(message);
            }
            let filter = new_filter(device, c"RT".as_ptr());
            if filter.is_null() {
                error(device, &mut message);
                let message = error_text(message);
                release_device(device);
                return Err(message);
            }
            let context = Self {
                _library: library,
                device,
                filter,
                image,
                commit,
                execute,
                error,
                release_filter,
                release_device,
            };
            set_bool(filter, c"hdr".as_ptr(), true);
            set_bool(filter, c"cleanAux".as_ptr(), true);
            set_int(filter, c"maxMemoryMB".as_ptr(), 512);
            context.check()?;
            Ok(context)
        }
    }
    fn check(&self) -> std::result::Result<(), String> {
        let mut message = std::ptr::null();
        if unsafe { (self.error)(self.device, &mut message) } != 0 {
            Err(error_text(message))
        } else {
            Ok(())
        }
    }
}
fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("CUEMOL_OIDN_LIBRARY") {
        paths.push(path.into());
    }
    let mut roots = Vec::new();
    if let Some(path) = std::env::var_os("CUEMOL_TOOLS_DIR") {
        roots.push(PathBuf::from(path));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("cuemol-tools"));
            roots.push(dir.join("../Resources/cuemol-tools"));
        }
    }
    if let Some(cache) = std::env::var_os("CUEMOL_CACHE_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|v| PathBuf::from(v).join("cuemol")))
        .or_else(|| std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".cache/cuemol")))
    {
        roots.push(cache.join("tools"));
    }
    if let Ok(dir) = std::env::current_dir() {
        for root in dir.ancestors().take(4) {
            roots.push(root.join(".cache/bundled-tools"));
        }
    }
    let names: &[&str] = if cfg!(windows) {
        &["OpenImageDenoise.dll"]
    } else if cfg!(target_os = "macos") {
        &["libOpenImageDenoise.2.dylib", "libOpenImageDenoise.dylib"]
    } else {
        &["libOpenImageDenoise.so.2", "libOpenImageDenoise.so"]
    };
    for root in roots {
        for directory in [root.join("oidn/lib"), root.join("oidn/bin"), root.clone()] {
            for name in names {
                paths.push(directory.join(name));
            }
        }
    }
    paths.extend(names.iter().map(PathBuf::from));
    paths
}
fn load() -> std::result::Result<Context, String> {
    let mut last = String::new();
    for path in candidates() {
        let library = unsafe {
            #[cfg(windows)]
            {
                libloading::os::windows::Library::load_with_flags(&path, 0x00001100)
                    .map(Library::from)
            }
            #[cfg(not(windows))]
            {
                Library::new(&path)
            }
        };
        match library {
            Ok(library) => return unsafe { Context::new(library) },
            Err(e) => {
                if path.is_file() {
                    last = e.to_string();
                }
            }
        }
    }
    Err(format!("OIDN runtime was not found. Install the CueMol tool bundle or set CUEMOL_OIDN_LIBRARY. {last}"))
}
static CONTEXT: OnceLock<Mutex<Option<Context>>> = OnceLock::new();
pub(super) fn filter(
    width: usize,
    height: usize,
    color: &[f32],
    albedo: &[f32],
    normal: &[f32],
) -> Result<Vec<f32>> {
    let expected = width.checked_mul(height).and_then(|v| v.checked_mul(4));
    if expected != Some(color.len()) || albedo.len() != color.len() || normal.len() != color.len() {
        return Err(CueMolError::invalid_argument(
            "OIDN input dimensions do not match",
        ));
    }
    let mut lock = CONTEXT
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| CueMolError::Io("OIDN lock poisoned".into()))?;
    if lock.is_none() {
        *lock = Some(load().map_err(CueMolError::Io)?);
    }
    let ctx = lock.as_mut().unwrap();
    let mut output = color.to_vec();
    let result = (|| {
        unsafe {
            for (name, data) in [
                (c"color", color.as_ptr()),
                (c"albedo", albedo.as_ptr()),
                (c"normal", normal.as_ptr()),
                (c"output", output.as_mut_ptr()),
            ] {
                (ctx.image)(
                    ctx.filter,
                    name.as_ptr(),
                    data as *mut c_void,
                    3,
                    width,
                    height,
                    0,
                    16,
                    width * 16,
                );
            }
            (ctx.commit)(ctx.filter);
            ctx.check()?;
            (ctx.execute)(ctx.filter);
            ctx.check()?;
        }
        Ok::<(), String>(())
    })();
    if let Err(e) = result {
        *lock = None;
        return Err(CueMolError::Io(format!("OIDN: {e}")));
    }
    Ok(output)
}
