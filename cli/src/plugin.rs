use anyhow::Context;
use ppdrive::plugin::{PluginEntry, PluginRegistry, PluginType, plugin_lib_name};
use std::path::PathBuf;

pub async fn execute_add(
    id_or_path: &str,
    plugin_type: PluginType,
    version: &str,
    local: bool,
    source: Option<&str>,
) -> Result<(), anyhow::Error> {
    let mut registry = PluginRegistry::load().await?;
    let libs_dir = PluginRegistry::libs_dir()?;
    tokio::fs::create_dir_all(&libs_dir).await?;

    if local {
        add_local(id_or_path, &plugin_type, source, &libs_dir, &mut registry).await?;
    } else {
        add_remote(id_or_path, &plugin_type, version, &libs_dir, &mut registry).await?;
    }

    registry.save().await?;
    Ok(())
}

async fn add_remote(
    id: &str,
    plugin_type: &PluginType,
    version: &str,
    libs_dir: &std::path::Path,
    registry: &mut PluginRegistry,
) -> Result<(), anyhow::Error> {
    if registry.is_installed(id) {
        println!("Plugin '{id}' is already installed.");
        return Ok(());
    }

    let api_url = if version == "latest" {
        format!("https://api.github.com/repos/{id}/releases/latest")
    } else {
        format!("https://api.github.com/repos/{id}/releases/tags/v{version}")
    };

    println!("Fetching release info for {id}...");

    let body: String = ureq::get(&api_url)
        .header("User-Agent", "ppdrive-plugin-manager")
        .call()
        .with_context(|| format!("failed to fetch release info for {id}"))?
        .body_mut()
        .read_to_string()
        .context("failed to read GitHub API response")?;

    let json: serde_json::Value =
        serde_json::from_str(&body).context("failed to parse GitHub API response")?;

    let tag_name = json["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing tag_name in release response"))?;
    let release_version = tag_name.trim_start_matches('v');

    let plugin_name = id.split('/').last().unwrap_or(id);
    let expected_asset = plugin_lib_name(plugin_name);

    let assets = json["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("no assets found in release"))?;

    let asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(&expected_asset))
        .ok_or_else(|| {
            let available: Vec<&str> = assets
                .iter()
                .filter_map(|a| a["name"].as_str())
                .collect();
            anyhow::anyhow!(
                "no matching asset '{expected_asset}' in release. Available: {available:?}"
            )
        })?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing download URL for asset"))?;

    println!("Downloading {expected_asset}...");
    download_file(download_url, &libs_dir.join(&expected_asset)).await?;

    let entry = PluginEntry {
        id: id.to_string(),
        filename: expected_asset,
        version: release_version.to_string(),
        plugin_type: plugin_type.clone(),
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    registry.add(entry);

    println!("Plugin '{id}' v{release_version} installed.");
    Ok(())
}

async fn add_local(
    path: &str,
    plugin_type: &PluginType,
    source: Option<&str>,
    libs_dir: &std::path::Path,
    registry: &mut PluginRegistry,
) -> Result<(), anyhow::Error> {
    let plugin_name = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path);

    let id = plugin_name.to_string();

    if registry.is_installed(&id) {
        println!("Plugin '{id}' is already installed.");
        return Ok(());
    }

    if let Some(source_dir) = source {
        build_from_source(path, source_dir).await?;
    }

    let src_path = if source.is_some() {
        // Built from source, look in target/release/
        let target_dir = std::path::Path::new(path).join("target/release");
        find_built_lib(&target_dir, plugin_name)?
    } else {
        PathBuf::from(path)
    };

    if !src_path.exists() {
        return Err(anyhow::anyhow!("file not found: {}", src_path.display()));
    }

    let lib_name = plugin_lib_name(plugin_name);
    let dest = libs_dir.join(&lib_name);
    tokio::fs::copy(&src_path, &dest)
        .await
        .with_context(|| format!("failed to copy {} to {}", src_path.display(), dest.display()))?;

    let entry = PluginEntry {
        id,
        filename: lib_name,
        version: "local".to_string(),
        plugin_type: plugin_type.clone(),
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    registry.add(entry);

    println!("Plugin installed from local file.");
    Ok(())
}

async fn build_from_source(_path: &str, source_dir: &str) -> Result<(), anyhow::Error> {
    println!("Building plugin from source...");

    let source_dir = source_dir.to_string();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&source_dir)
            .output()
    })
    .await
    .context("failed to spawn cargo build task")?
    .context("failed to run cargo build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("cargo build failed:\n{stderr}"));
    }

    println!("cargo build --release finished.");
    Ok(())
}

fn find_built_lib(target_dir: &std::path::Path, name: &str) -> Result<PathBuf, anyhow::Error> {
    let os = std::env::consts::OS;
    let ext = match os {
        "linux" => "so",
        "macos" => "dylib",
        "windows" => "dll",
        _ => "so",
    };

    // Check common output names
    let candidates = [
        target_dir.join(format!("lib{name}.{ext}")),
        target_dir.join(format!("{name}.{ext}")),
        target_dir.join(format!("lib{name}.so")),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    // Search for any file matching the extension
    if let Ok(entries) = std::fs::read_dir(target_dir) {
        for entry in entries.flatten() {
            if let Some(entry_name) = entry.file_name().to_str() {
                if entry_name.ends_with(&format!(".{ext}")) && entry_name.contains(name) {
                    return Ok(entry.path());
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "no built library found in {} matching '{name}'",
        target_dir.display()
    ))
}

async fn download_file(url: &str, dest: &std::path::Path) -> Result<(), anyhow::Error> {
    let mut resp = ureq::get(url)
        .header("User-Agent", "ppdrive-plugin-manager")
        .call()
        .with_context(|| format!("failed to download {url}"))?;

    let body = resp
        .body_mut()
        .read_to_vec()
        .context("failed to read download body")?;

    tokio::fs::write(dest, body)
        .await
        .with_context(|| format!("failed to write {}", dest.display()))?;

    Ok(())
}

pub async fn execute_list() -> Result<(), anyhow::Error> {
    let registry = PluginRegistry::load().await?;
    let plugins = registry.list();

    if plugins.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    println!("{:<40} {:<10} {:<10} {}", "ID", "VERSION", "TYPE", "INSTALLED");
    println!("{}", "-".repeat(80));
    for p in plugins {
        println!(
            "{:<40} {:<10} {:<10} {}",
            p.id, p.version, p.plugin_type, p.installed_at
        );
    }

    Ok(())
}

pub async fn execute_remove(id: &str) -> Result<(), anyhow::Error> {
    let mut registry = PluginRegistry::load().await?;

    let entry = registry
        .remove(id)
        .ok_or_else(|| anyhow::anyhow!("plugin '{id}' is not installed"))?;

    let libs_dir = PluginRegistry::libs_dir()?;
    let lib_path = libs_dir.join(&entry.filename);
    if lib_path.exists() {
        tokio::fs::remove_file(&lib_path)
            .await
            .with_context(|| format!("failed to remove {}", lib_path.display()))?;
    }

    registry.save().await?;
    println!("Plugin '{id}' removed.");
    Ok(())
}
