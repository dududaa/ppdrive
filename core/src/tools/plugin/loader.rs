use anyhow::{Context, anyhow};
use libloading::Library;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::null_mut;

#[repr(C)]
#[derive(Clone)]
/// A raw pointer to the dispatched response.
/// The [PluginDispatcher] **must** stay alive for as long as we want the (dispatched response)[DispatchResponse]
/// to stay.
pub struct PluginDispatcher<T> {
    ptr: *mut T,
}

impl<T> Default for PluginDispatcher<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> PluginDispatcher<T> {
    pub fn new() -> Self {
        Self { ptr: null_mut() }
    }

    pub fn dispatch<A>(&mut self, lib: &LoadedPlugin, args: A) -> anyhow::Result<&T> {
        let input = Box::into_raw(Box::new(args));

        unsafe {
            let resp = (lib.dispatch_fn)(input as *mut c_void);
            if resp.is_null() {
                return Err(anyhow!("plugin call returned null"));
            }

            match *Box::from_raw(resp) {
                DispatchResponse::Ok(ptr) => {
                    self.ptr = ptr as *mut T;
                    Ok(&*self.ptr)
                }
                DispatchResponse::Error(msg) => Err(anyhow!(msg)),
            }
        }
    }
}

impl<T> Drop for PluginDispatcher<T> {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                let _ = Box::from_raw(self.ptr);
            }
        }
    }
}

#[repr(C)]
pub enum DispatchResponse {
    Ok(*mut c_void),
    Error(String),
}

pub struct LoadedPlugin {
    id: String,
    _lib: Library,
    dispatch_fn: unsafe extern "C" fn(*mut c_void) -> *mut DispatchResponse,
}

unsafe impl Send for LoadedPlugin {}
unsafe impl Sync for LoadedPlugin {}

impl LoadedPlugin {
    pub fn load(path: &Path, id: String) -> anyhow::Result<Self> {
        #[cfg(windows)]
        let lib = {
            use libloading::os::windows::{LOAD_WITH_ALTERED_SEARCH_PATH, Library as OsLibrary};
            // LOAD_WITH_ALTERED_SEARCH_PATH resolves the plugin's dependent
            // DLLs (the shared ppdrive-media-runtime) relative to the
            // plugin's directory, not the process CWD/PATH. It requires an
            // absolute path, so canonicalize first (the \\?\ form is
            // accepted by LoadLibraryExW).
            let abs = std::fs::canonicalize(path)
                .with_context(|| format!("failed to resolve plugin path {}", path.display()))?;
            unsafe { OsLibrary::load_with_flags(&abs, LOAD_WITH_ALTERED_SEARCH_PATH) }
                .map(Library::from)
        };
        #[cfg(not(windows))]
        let lib = unsafe { Library::new(path) };

        let lib = lib.map_err(|err| {
            anyhow!(
                "failed to load plugin from {}. Error: {err}",
                path.display()
            )
        })?;

        let dispatch_fn = unsafe {
            *lib.get::<unsafe extern "C" fn(*mut c_void) -> *mut DispatchResponse>(
                b"plugin_dispatch",
            )
            .context("plugin missing 'plugin_dispatch' function")?
        };

        Ok(Self {
            _lib: lib,
            dispatch_fn,
            id,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}
