use std::{path::PathBuf, sync::Arc};

use libloading::Library;
use tokio::sync::{mpsc::{self, Receiver, Sender}, Mutex};
use tokio_util::sync::CancellationToken;

use crate::{AppResult, errors::Error, tools::root_dir};

pub trait Plugin {
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
        let output_path = self.output()?;

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
        let filename = self.output()?;

        if let Err(err) = std::fs::remove_file(&filename) {
            eprintln!("cannot remove previous plugin: {err}")
        }

        Ok(())
    }

    /// prepare plugin for loading. attempts to install plugin (and its dependencies) if it's not installed.
    fn preload(&self) -> AppResult<()> {
        #[cfg(debug_assertions)]
        self.remove()?;

        let filename = self.output()?;

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
        let n = format!("target/debug/lib{}{}", self.package_name(), self.ext());

        let p = root_dir()?.join(n);
        Ok(p)
    }

    /// output filename (path) of the plugin, relative to installation directory
    fn output(&self) -> AppResult<PathBuf> {
        let n = format!("{}{}", self.package_name(), self.ext());
        let p = root_dir()?.join(n);

        Ok(p)
    }
}

pub trait HasDependecies: Plugin {
    fn has_dependencies(&self) -> bool {
        !self.dependecies().is_empty()
    }

    fn preload_deps(&self) -> AppResult<()> {
        if self.has_dependencies() {
            for dep in self.dependecies() {
                dep.preload()?;
            }
        }

        Ok(())
    }

    fn dependecies(&self) -> Vec<Box<dyn Plugin>>;
}

pub type TTRaw<T> = *const TTChannel<T>;
type TransportInner<T> = Arc<TTChannel<T>>;

pub struct PluginTransport<T>(TransportInner<T>);

impl<T> PluginTransport<T> {
    pub fn new() -> Self {
        let c = mpsc::channel::<T>(1);
        let inner = Arc::new(Mutex::new(c.into()));
        Self(inner)
    }

    pub async fn send(&self, value: T) -> AppResult<()> {
        let mut state = self.0.lock().await;
        let tx = &state.tx;

        tx.send(value)
            .await
            .map_err(|_| Error::ServerError("unable to send token".to_string()))?;

        state.sent = true;
        Ok(())
    }

    pub async fn recv(self) -> Option<T> {
        let mut state = self
            .0
            .lock()
            .await;


        state.rx.recv().await
    }

    pub fn into_raw(self) -> TTRaw<T> {
        Arc::into_raw(self.0)
    }

    pub fn from_raw(ptr: TTRaw<T>) -> Self {
        let inner = unsafe { Arc::from_raw(ptr) };
        Self(inner)
    }
}

impl<T> Clone for PluginTransport<T> {
    fn clone(&self) -> Self {
        let ptr = self.0.clone();

        Self(ptr)
    }
}

type TTChannel<T> = Mutex<TTChannelState<T>>;
pub struct TTChannelState<T> {
    tx: Sender<T>,
    rx: Receiver<T>,
    sent: bool
}

impl<T> From<(Sender<T>, Receiver<T>)> for TTChannelState<T> {
    fn from(value: (Sender<T>, Receiver<T>)) -> Self {
        Self { tx: value.0, rx: value.1, sent: false }
    }
}