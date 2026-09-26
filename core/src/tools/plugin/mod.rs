use crate::root_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod loader;
pub const PLUGINS_FILENAME: &str = "plugins.json";
pub const LIBS_DIR: &str = "libs";
/// Prefix prepended to the short id to form the full plugin id.
pub const PLUGIN_ID_PREFIX: &str = "ppdrive_";

/// The short id as accepted on input: any `ppdrive_` prefix is stripped.
pub fn plugin_short_id(id: &str) -> &str {
    id.strip_prefix(PLUGIN_ID_PREFIX).unwrap_or(id)
}

/// The full plugin id: short id with the `ppdrive_` prefix.
///
/// This is the cargo package name, the release artifact base name, and the
/// id the server looks up in the registry. Stored entries keep the short id;
/// the full id is rebuilt wherever it is needed.
pub fn plugin_full_id(id: &str) -> String {
    format!("{PLUGIN_ID_PREFIX}{}", plugin_short_id(id))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEntry {
    /// Short id without the `ppdrive_` prefix (e.g. `dashboard`).
    /// Use [`PluginEntry::full_id`] to get `ppdrive_dashboard`.
    pub id: String,
    pub filename: String,
    pub version: String,
    pub installed_at: String,
    /// Where this plugin was installed from.
    /// For remote: `"github:owner/repo"`.
    /// For local: the file path or source directory path.
    #[serde(default)]
    pub source: Option<String>,
    /// Whether the plugin was built from source during install.
    #[serde(default)]
    pub build: bool,
}

impl PluginEntry {
    /// The full plugin id, rebuilt from the stored short id.
    pub fn full_id(&self) -> String {
        plugin_full_id(&self.id)
    }
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
        let mut plugins: PluginsFile = if file_path.exists() {
            let content = tokio::fs::read_to_string(&file_path).await?;
            toml::from_str(&content)
                .or_else(|_| serde_json::from_str(&content))
                .unwrap_or_default()
        } else {
            PluginsFile::default()
        };
        // Entries used to store the full id; keep everything short on disk.
        for p in &mut plugins.plugins {
            let short = plugin_short_id(&p.id).to_string();
            p.id = short;
        }
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

    pub fn add(&mut self, mut entry: PluginEntry) {
        entry.id = plugin_short_id(&entry.id).to_string();
        self.plugins.plugins.push(entry);
    }

    pub fn remove(&mut self, id: &str) -> Option<PluginEntry> {
        let idx = self.plugins.plugins.iter().position(|p| same_id(&p.id, id))?;
        Some(self.plugins.plugins.remove(idx))
    }

    pub fn get(&self, id: &str) -> Option<&PluginEntry> {
        self.plugins.plugins.iter().find(|p| same_id(&p.id, id))
    }

    pub fn list(&self) -> &[PluginEntry] {
        &self.plugins.plugins
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.plugins.plugins.iter().any(|p| same_id(&p.id, id))
    }

    pub fn find(&self, id: &str) -> Option<&PluginEntry> {
        self.plugins.plugins.iter().find(|p| same_id(&p.id, id))
    }

    pub fn libs_dir() -> anyhow::Result<PathBuf> {
        Ok(root_dir()?.join(LIBS_DIR))
    }
}

/// Match a stored id against a query, regardless of whether either side
/// carries the `ppdrive_` prefix.
fn same_id(stored: &str, query: &str) -> bool {
    plugin_short_id(stored) == plugin_short_id(query)
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
