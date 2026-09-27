use crate::root_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod loader;
pub const PLUGINS_FILENAME: &str = "plugins.json";
pub const LIBS_DIR: &str = "libs";
/// Prefix prepended to the short id to form the full plugin id.
pub const PLUGIN_ID_PREFIX: &str = "ppdrive-";
/// Previous prefix convention; still accepted on input and in stored registries.
pub const LEGACY_PLUGIN_ID_PREFIX: &str = "ppdrive_";

/// The short id as accepted on input: any `ppdrive-`/`ppdrive_` prefix is
/// stripped. Use [`normalize_plugin_id`] when a canonical id is needed.
pub fn plugin_short_id(id: &str) -> &str {
    id.strip_prefix(PLUGIN_ID_PREFIX)
        .or_else(|| id.strip_prefix(LEGACY_PLUGIN_ID_PREFIX))
        .unwrap_or(id)
}

/// Canonicalize a plugin id: strip either prefix convention and normalize
/// `_` separators to `-` (e.g. `ppdrive_image_compression` →
/// `image-compression`). All ids are stored and compared in this form.
pub fn normalize_plugin_id(id: &str) -> String {
    plugin_short_id(id).replace('_', "-")
}

/// The full plugin id: short id with the `ppdrive-` prefix.
///
/// This is the cargo package name, the release artifact base name, and the
/// id the server looks up in the registry. Stored entries keep the short id;
/// the full id is rebuilt wherever it is needed.
pub fn plugin_full_id(id: &str) -> String {
    format!("{PLUGIN_ID_PREFIX}{}", normalize_plugin_id(id))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEntry {
    /// Short id without the `ppdrive-` prefix (e.g. `dashboard`).
    /// Use [`PluginEntry::full_id`] to get `ppdrive-dashboard`.
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PluginsFile {
    pub plugins: Vec<PluginEntry>,
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
        // Migrate to the current convention: short id, `-` separators
        // (entries used to store the full `ppdrive_` id).
        for p in &mut plugins.plugins {
            p.id = normalize_plugin_id(&p.id);
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
        entry.id = normalize_plugin_id(&entry.id);
        self.plugins.plugins.push(entry);
    }

    pub fn remove(&mut self, id: &str) -> Option<PluginEntry> {
        let idx = self
            .plugins
            .plugins
            .iter()
            .position(|p| same_id(&p.id, id))?;
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

/// Match a stored id against a query, regardless of prefix convention
/// (`ppdrive-`/`ppdrive_`) or separator style (`-`/`_`).
fn same_id(stored: &str, query: &str) -> bool {
    normalize_plugin_id(stored) == normalize_plugin_id(query)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_strips_both_prefix_conventions() {
        assert_eq!(plugin_short_id("ppdrive-dashboard"), "dashboard");
        assert_eq!(plugin_short_id("ppdrive_dashboard"), "dashboard");
        assert_eq!(plugin_short_id("dashboard"), "dashboard");
    }

    #[test]
    fn normalize_enforces_hyphen_convention() {
        assert_eq!(
            normalize_plugin_id("ppdrive_image_compression"),
            "image-compression"
        );
        assert_eq!(
            normalize_plugin_id("image_compression"),
            "image-compression"
        );
        assert_eq!(
            normalize_plugin_id("image-compression"),
            "image-compression"
        );
        assert_eq!(
            normalize_plugin_id("ppdrive-image-compression"),
            "image-compression"
        );
    }

    #[test]
    fn full_id_is_hyphenated_from_any_input_form() {
        assert_eq!(plugin_full_id("dashboard"), "ppdrive-dashboard");
        assert_eq!(plugin_full_id("ppdrive_dashboard"), "ppdrive-dashboard");
        assert_eq!(
            plugin_full_id("image_compression"),
            "ppdrive-image-compression"
        );
        assert_eq!(
            plugin_full_id(&plugin_full_id("image_compression")),
            "ppdrive-image-compression",
            "full id must be idempotent"
        );
    }

    #[test]
    fn same_id_matches_across_conventions() {
        assert!(same_id("image_compression", "image-compression"));
        assert!(same_id("ppdrive_image_compression", "image-compression"));
        assert!(same_id("ppdrive-dashboard", "dashboard"));
        assert!(!same_id("image-compression", "image-transformation"));
    }

    #[test]
    fn lib_name_follows_hyphen_convention() {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let ext = if os == "windows" {
            "dll"
        } else if os == "macos" {
            "dylib"
        } else {
            "so"
        };
        assert_eq!(
            plugin_lib_name(&plugin_full_id("image_compression")),
            format!("ppdrive-image-compression-{os}-{arch}.{ext}")
        );
    }

    #[test]
    fn registry_migrates_legacy_ids_on_add_and_load_match() {
        let mut registry = PluginRegistry {
            file_path: PathBuf::from("unused-test-path"),
            plugins: PluginsFile::default(),
        };

        registry.add(PluginEntry {
            id: "ppdrive_image_compression".to_string(),
            filename: "ppdrive_image_compression-linux-x86_64.so".to_string(),
            version: "local".to_string(),
            installed_at: "now".to_string(),
            source: None,
            build: false,
        });

        assert_eq!(registry.list()[0].id, "image-compression");
        assert!(registry.is_installed("image_compression"));
        assert!(registry.is_installed("ppdrive-image-compression"));
        assert_eq!(registry.list()[0].full_id(), "ppdrive-image-compression");
        // Stored filename is preserved so already-installed libraries keep loading.
        assert_eq!(
            registry.list()[0].filename,
            "ppdrive_image_compression-linux-x86_64.so"
        );
    }
}
