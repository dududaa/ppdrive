use anyhow::{Context, anyhow};
use std::ffi::c_void;
use std::path::Path;
use std::ptr::null_mut;

#[repr(C)]
pub enum DispatchResponse {
    Ok(*mut c_void),
    Error(String),
}

pub struct LoadedPlugin<A, R> {
    _lib: libloading::Library,
    dispatch_fn: unsafe extern "C" fn(*mut c_void) -> *mut DispatchResponse,
    args_ptr: *mut A,
    resp_ptr: *mut R,
}

unsafe impl<A, R> Send for LoadedPlugin<A, R> {}
unsafe impl<A, R> Sync for LoadedPlugin<A, R> {}

impl<A, R> LoadedPlugin<A, R> {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let lib = unsafe { libloading::Library::new(path) }
            .with_context(|| format!("failed to load plugin from {}", path.display()))?;

        let dispatch_fn = unsafe {
            *lib.get::<unsafe extern "C" fn(*mut c_void) -> *mut DispatchResponse>(
                b"plugin_dispatch",
            )
            .context("plugin missing 'plugin_dispatch' function")?
        };

        Ok(Self {
            _lib: lib,
            dispatch_fn,
            args_ptr: null_mut(),
            resp_ptr: null_mut(),
        })
    }

    pub fn dispatch(&mut self, args: A) -> anyhow::Result<&R> {
        let input = Box::into_raw(Box::new(args));
        self.args_ptr = input;

        unsafe {
            let resp = (self.dispatch_fn)(input as *mut c_void);
            match *Box::from_raw(resp) {
                DispatchResponse::Ok(ptr) => {
                    self.resp_ptr = ptr as *mut R;
                    Ok(&*self.resp_ptr)
                }
                DispatchResponse::Error(msg) => Err(anyhow!(msg)),
            }
        }
    }
}

impl<A, R> Drop for LoadedPlugin<A, R> {
    fn drop(&mut self) {
        unsafe {
            if !self.args_ptr.is_null() {
                let _ = Box::from_raw(self.args_ptr);
            }

            if !self.resp_ptr.is_null() {
                let _ = Box::from_raw(self.resp_ptr);
            }
        }
    }
}
