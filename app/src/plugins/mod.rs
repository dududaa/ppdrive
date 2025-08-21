use std::path::PathBuf;

use ppd_shared::tools::root_dir;

use crate::errors::AppResult;

pub mod service;

pub trait Plugin {
    /// filename (path) of the plugin, relative to installation directory
    fn filename(&self) -> AppResult<PathBuf>;

    /// package name of the plugin, as declared in manifest (Cargo.toml)
    fn package_name(&self) -> &'static str;

    /// local installation of the plugin. this involves building the plugin from source code
    /// and moving the binary to route.
    #[cfg(debug_assertions)]
    fn install_local(&self) -> AppResult<()>;

    /// remote installation of the plugin. this involves downloading the binary from a remote
    /// server
    fn install_remote(&self) {
        unimplemented!()
    }

    /// install the plugin, depending on the environment.
    fn install(&self) -> AppResult<()> {
        if cfg!(debug_assertions) {
            self.install_local()?;
        } else {
            self.install_remote();
        }

        Ok(())
    }

    /// output file extenstion for the plugin
    fn ext() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "dll"
        }

        #[cfg(target_os = "macos")]
        {
            "dylib"
        }

        #[cfg(target_os = "linux")]
        "dll"
    }

    /// the output filename path of the release build.
    #[cfg(debug_assertions)]
    fn release_path(&self) -> AppResult<PathBuf> {
        let n = format!(
            "target/debug/lib{}.{}",
            self.package_name(),
            Self::ext()
        );

        let p = root_dir()?.join(n);
        Ok(p)
    }
}

