use crate::plugin::PluginType;
use crate::state::AppState;
use anyhow::Context;
use std::path::Path;
use std::ptr::null_mut;
#[cfg(feature = "server")]
use axum::Router;

#[repr(C)]
pub struct PluginBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

unsafe impl Send for PluginBuffer {}
unsafe impl Sync for PluginBuffer {}

pub enum PluginRequest {
    #[cfg(feature = "server")]
    Router {
        base_path: String,
        state: AppState
    },
}

pub enum PluginResponse {
    Info {
        name: String,
        version: String,
        plugin_type: PluginType,
    },
    #[cfg(feature = "server")]
    Router(Router<AppState>),
    Error(String),
}

pub struct LoadedPlugin {
    _lib: libloading::Library,
    dispatch_fn: unsafe extern "C" fn(*mut &PluginRequest) -> *mut PluginResponse,
    resp_ptr: *mut PluginResponse,
    // free_fn: unsafe extern "C" fn(PluginBuffer),
}

unsafe impl Send for LoadedPlugin {}
unsafe impl Sync for LoadedPlugin {}

impl LoadedPlugin {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let lib = unsafe { libloading::Library::new(path) }
            .with_context(|| format!("failed to load plugin from {}", path.display()))?;

        let dispatch_fn = unsafe {
            *lib.get::<unsafe extern "C" fn(*mut &PluginRequest) -> *mut PluginResponse>(
                b"plugin_dispatch",
            )
            .context("plugin missing 'plugin_dispatch' function")?
        };

        Ok(Self {
            _lib: lib,
            dispatch_fn,
            resp_ptr: null_mut(),
        })
    }

    pub fn dispatch(&mut self, request: &PluginRequest) -> &PluginResponse {
        let input = Box::into_raw(Box::new(request));
        unsafe {
            let ptr = (self.dispatch_fn)(input);
            self.resp_ptr = ptr;
            
            &*self.resp_ptr
        }
    }
}
