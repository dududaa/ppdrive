use std::path::PathBuf;

use libloading::Library;

use crate::{AppResult, tools::root_dir};

pub mod service;
pub mod router;

pub trait Plugin {
    /// output filename (path) of the plugin, relative to installation directory
    fn filename(&self) -> AppResult<PathBuf>;

    /// package name of the plugin, as declared in manifest (Cargo.toml)
    fn package_name(&self) -> &'static str;

    /// local installation of the plugin. this involves building the plugin from source code
    /// and moving the binary to route.
    #[cfg(debug_assertions)]
    fn install_local(&self) -> AppResult<()> {
        use std::process::Command;

        let args = vec!["build", "-p", self.package_name()];

        let mut child = Command::new("cargo").args(args).spawn()?;
        child.wait()?;

        let release_path = self.release_path()?;
        let output_path = self.filename()?;

        std::fs::rename(release_path, output_path)?;

        Ok(())
    }

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

    /// remove the plugin
    fn remove(&self) -> AppResult<()> {
        let filename = self.filename()?;

        if let Err(err) = std::fs::remove_file(&filename) {
            eprintln!("cannot remove previous plugin: {err}")
        }

        Ok(())
    }

    /// load the plugin. attempts to install plugin if it's not installed.
    fn load(&self) -> AppResult<Library> {
        let filename = self.filename()?;

        if !filename.is_file() {
            // confirm before installation
            println!(
                "You currently don't have \"{}\" plugin installed. Do you want to install it? (y/n)",
                self.package_name()
            );
            let mut ans = String::new();

            std::io::stdin().read_line(&mut ans)?;
            let ans = ans.trim().to_lowercase();

            if &ans == "y" {
                println!("installing \"{}\" plugin ...", self.package_name());
                self.install()?;
            } 
        }

        let lib = unsafe { Library::new(&filename)? };

        Ok(lib)
    }

    /// output file extenstion for the plugin
    fn ext() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            ".dll"
        }

        #[cfg(target_os = "macos")]
        {
            ".dylib"
        }

        #[cfg(target_os = "linux")]
        ".dll"
    }

    /// the output filename path of the release build.
    #[cfg(debug_assertions)]
    fn release_path(&self) -> AppResult<PathBuf> {
        let n = format!(
            "target/debug/lib{}{}",
            self.package_name(),
            Self::ext()
        );

        let p = root_dir()?.join(n);
        Ok(p)
    }
}

