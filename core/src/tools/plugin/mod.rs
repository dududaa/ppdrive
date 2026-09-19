use crate::root_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod loader;
pub const PLUGINS_FILENAME: &str = "plugins.json";
pub const LIBS_DIR: &str = "libs";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub filename: String,
    pub version: String,
    pub installed_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginsFile {
    pub plugins: Vec<PluginEntry>,
}

impl Default for PluginsFile {
    fn default() -> Self {
        Self { plugins: vec![] }
    }
}

pub struct PluginRegistry {
    file_path: PathBuf,
    plugins: PluginsFile,
}

impl PluginRegistry {
    pub async fn load() -> anyhow::Result<Self> {
        let file_path = root_dir()?.join(PLUGINS_FILENAME);
        let plugins = if file_path.exists() {
            let content = tokio::fs::read_to_string(&file_path).await?;
            toml::from_str(&content)
                .or_else(|_| serde_json::from_str(&content))
                .unwrap_or_default()
        } else {
            PluginsFile::default()
        };
        Ok(Self { file_path, plugins })
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(&self.plugins)?;
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&self.file_path, content).await?;
        Ok(())
    }

    pub fn add(&mut self, entry: PluginEntry) {
        self.plugins.plugins.push(entry);
    }

    pub fn remove(&mut self, id: &str) -> Option<PluginEntry> {
        let idx = self.plugins.plugins.iter().position(|p| p.id == id)?;
        Some(self.plugins.plugins.remove(idx))
    }

    pub fn get(&self, id: &str) -> Option<&PluginEntry> {
        self.plugins.plugins.iter().find(|p| p.id == id)
    }

    pub fn list(&self) -> &[PluginEntry] {
        &self.plugins.plugins
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.plugins.plugins.iter().any(|p| p.id == id)
    }

    pub fn find(&self, id: &str) -> Option<&PluginEntry> {
        self.plugins.plugins.iter().find(|p| p.id == id)
    }
    
    pub fn libs_dir() -> anyhow::Result<PathBuf> {
        Ok(root_dir()?.join(LIBS_DIR))
    }
}

pub fn plugin_lib_name(name: &str) -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let ext = match os {
        "linux" => "so",
        "macos" => "dylib",
        "windows" => "dll",
        _ => "so",
    };
    format!("{name}-{os}-{arch}.{ext}")
}

pub fn plugin_lib_path(name: &str) -> anyhow::Result<PathBuf> {
    Ok(PluginRegistry::libs_dir()?.join(plugin_lib_name(name)))
}
