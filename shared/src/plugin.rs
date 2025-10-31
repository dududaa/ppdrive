use libloading::Library;
use reqwest::StatusCode;

use crate::{AppResult, errors::Error, tools::root_dir};
use std::path::PathBuf;

pub trait Plugin {
    /// package name of the plugin, as declared in manifest (Cargo.toml)
    fn package_name(&self) -> &'static str;

    fn symbol_name(&self) -> Vec<u8> {
        let name = self.package_name().replace("-", "_");
        name.as_bytes().to_vec()
    }

    /// local installation of the plugin. this involves building the plugin from source code
    /// and moving the binary to route.
    #[cfg(debug_assertions)]
    fn install_local(&self) -> AppResult<()> {
        use std::process::Command;

        let args = vec!["build", "-p", self.package_name()];
        let mut child = Command::new("cargo").args(args).spawn()?;
        child.wait()?;

        let release_path = self.release_path()?;
        let output_path = self.output_name()?;

        std::fs::rename(release_path, output_path)?;

        Ok(())
    }

    /// remote installation of the plugin. this involves downloading the binary from a remote
    /// server
    fn install_remote(&self) -> AppResult<()> {
        let version = env!("CARGO_PKG_VERSION");
        let name = format!("{}{}", self.package_name(), self.ext());
        
        let url = format!("https://github.com/dududaa/ppdrive/releases/download/v{version}/{name}");
        println!("downloading {name} from {url}...");
        
        let resp = reqwest::blocking::get(url)?;
        match resp.status() {
            StatusCode::OK => {
                let body = resp.bytes()?;
                let output = self.output_name()?;
                std::fs::write(output, body)?;
        
                Ok(())
            }
            _ => {
                let msg = resp.text()?;
                Err(Error::ServerError(format!("unable download {name}: {msg}")))
            }
        }
    }

    /// install the plugin, depending on the environment.
    fn install(&self) -> AppResult<()> {

        #[cfg(debug_assertions)]
        self.install_local()?;

        #[cfg(not(debug_assertions))]
        self.install_remote()?;

        Ok(())
    }

    /// remove the plugin
    fn remove(&self) -> AppResult<()> {
        let filename = self.output_name()?;

        if let Err(err) = std::fs::remove_file(&filename) {
            eprintln!("cannot remove previous plugin: {err}")
        }

        Ok(())
    }

    /// prepare plugin for loading. attempts to install plugin (and its dependencies) if it's not installed.
    /// If `prompt` is true, users will
    fn preload(&self, auto_install: bool, reload: bool) -> AppResult<()> {
        if reload {
            self.remove()?;
        }

        let filename = self.output_name()?;
        let mut install = if auto_install || reload { "y" } else { "n" };
        let mut promp_resp = String::new();

        if !filename.is_file() {
            if !auto_install {
                println!(
                    "You currently don't have \"{}\" plugin installed. Do you want to install it? (y/n)",
                    self.package_name()
                );

                std::io::stdin().read_line(&mut promp_resp)?;
                promp_resp = promp_resp.trim().to_lowercase();
                install = promp_resp.as_str();
            }

            if install != "y" {
                return Err(Error::ServerError(format!(
                    "required module \"{}\" is missing. install the module and try again.",
                    self.package_name()
                )));
            } else {
                println!("installing \"{}\" plugin...", self.package_name());
                self.install()?;

            }
        } else {
            println!("package \"{}\" is available. skipping installation...", self.package_name());
        }


        Ok(())
    }

    /// load the plugin
    fn load(&self, filename: PathBuf) -> AppResult<Library> {
        let lib = unsafe { Library::new(filename)? };
        Ok(lib)
    }

    /// output file extenstion for the plugin
    fn ext(&self) -> &'static str {
        #[cfg(target_os = "windows")]
        {
            ".dll"
        }

        #[cfg(target_os = "macos")]
        {
            ".dylib"
        }

        #[cfg(target_os = "linux")]
        ".so"
    }

    /// the output filename path of the release build.
    #[cfg(debug_assertions)]
    fn release_path(&self) -> AppResult<PathBuf> {
        let package_name = self.package_name().replace("-", "_");

        #[cfg(target_os = "windows")]
        let n = format!("target/debug/{}{}", package_name, self.ext());
        
        
        #[cfg(not(target_os = "windows"))]
        let n = format!("target/debug/lib{}{}", package_name, self.ext());

        let p = root_dir()?.join(n);
        Ok(p)
    }

    /// output filename (path) of the plugin, relative to installation directory
    fn output_name(&self) -> AppResult<PathBuf> {
        let name = format!("{}{}", self.package_name(), self.ext());
        let path = root_dir()?.join(name);

        Ok(path)
    }
}

pub trait Module: Plugin {
    fn has_dependencies(&self) -> bool {
        !self.dependecies().is_empty()
    }

    fn preload_deps(&self, auto_install: bool, reload: bool) -> AppResult<()> {
        if self.has_dependencies() {
            for dep in self.dependecies() {
                dep.preload(auto_install, reload)?;
            }
        }

        Ok(())
    }

    fn dependecies(&self) -> Vec<Box<dyn Plugin>>;
}