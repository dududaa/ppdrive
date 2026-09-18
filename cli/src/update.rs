use anyhow::{Context, anyhow};
use flate2::read::GzDecoder;
use ppdrive::root_dir;
use std::path::{Path, PathBuf};
use tar::Archive;

const REPO: &str = "dududaa/ppdrive";

pub async fn execute() -> Result<(), anyhow::Error> {
    println!("Checking for updates...");

    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: v{current_version}");

    let latest_version = fetch_latest_version()
        .await
        .context("failed to fetch latest version from GitHub")?;
    println!("Latest version:  v{latest_version}");

    if current_version == latest_version.trim_start_matches('v') {
        println!("Already up to date!");
        return Ok(());
    }

    let asset_name = detect_asset()?;
    let install_dir = root_dir()?;
    let temp_dir = install_dir.join("tmp_update");

    println!("Downloading {asset_name}...");

    let tarball_path = download_release(&latest_version, &asset_name, &temp_dir)
        .await
        .context("failed to download release")?;

    println!("Extracting binaries...");

    let extracted = extract_tarball(&tarball_path, &temp_dir).context("failed to extract tarball")?;

    // Create backups before replacing
    let mut backups: Vec<(PathBuf, PathBuf)> = Vec::new();
    for name in &["ppdrive", "server"] {
        let dest = install_dir.join(name);
        if dest.exists() {
            let backup = install_dir.join(format!("{name}.bak"));
            tokio::fs::copy(&dest, &backup).await?;
            backups.push((dest, backup));
        }
    }

    // Replace binaries
    let mut updated = Vec::new();
    for name in &["ppdrive", "server"] {
        let src = extracted.join(name);
        if !src.exists() {
            println!("Warning: {name} not found in release archive, skipping");
            continue;
        }
        let dest = install_dir.join(name);
        tokio::fs::copy(&src, &dest).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&dest, perms)?;
        }

        updated.push(name.to_string());
        println!("Updated {name}");
    }

    // Clean up backups on success
    for (_, backup) in &backups {
        let _ = tokio::fs::remove_file(backup).await;
    }
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    // Update symlinks on Unix
    #[cfg(unix)]
    update_symlinks(&install_dir)?;

    if updated.is_empty() {
        println!("No binaries were updated. The release may not include this platform.");
    } else {
        println!(
            "\nUpdated successfully from v{current_version} to v{latest_version}!"
        );
        println!("Restart your server if it's running.");
    }

    Ok(())
}

async fn fetch_latest_version() -> Result<String, anyhow::Error> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body: String = ureq::get(&url)
        .header("User-Agent", "ppdrive-updater")
        .call()
        .context("failed to connect to GitHub API")?
        .body_mut()
        .read_to_string()
        .context("failed to read GitHub API response")?;

    let json: serde_json::Value =
        serde_json::from_str(&body).context("failed to parse GitHub API response")?;

    let tag = json["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow!("missing tag_name in GitHub response"))?;

    Ok(tag.trim_start_matches('v').to_string())
}

fn detect_asset() -> Result<String, anyhow::Error> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match (os, arch) {
        ("linux", "x86_64") => Ok("ppdrive-linux.tar.gz".to_string()),
        ("linux", "aarch64") => Ok("ppdrive-linux-arm64.tar.gz".to_string()),
        ("macos", "x86_64") => Ok("ppdrive-macos.tar.gz".to_string()),
        ("macos", "aarch64") => Ok("ppdrive-macos-arm64.tar.gz".to_string()),
        _ => Err(anyhow!("unsupported platform: {os}/{arch}")),
    }
}

async fn download_release(
    version: &str,
    asset_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, anyhow::Error> {
    tokio::fs::create_dir_all(temp_dir).await?;

    let url = format!(
        "https://github.com/{REPO}/releases/download/v{version}/{asset_name}"
    );

    let mut resp = ureq::get(&url)
        .header("User-Agent", "ppdrive-updater")
        .call()
        .context("failed to start download")?;

    let tarball_path = temp_dir.join(asset_name);
    let body = resp
        .body_mut()
        .read_to_vec()
        .context("failed to read download body")?;
    std::fs::write(&tarball_path, body)?;

    Ok(tarball_path)
}

fn extract_tarball(tarball_path: &Path, dest_dir: &Path) -> Result<PathBuf, anyhow::Error> {
    let file = std::fs::File::open(tarball_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    archive.unpack(dest_dir)?;

    Ok(dest_dir.to_path_buf())
}

#[cfg(unix)]
fn update_symlinks(install_dir: &Path) -> Result<(), anyhow::Error> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let bin_dir = PathBuf::from(home).join(".local/bin");

    if !bin_dir.exists() {
        return Ok(());
    }

    for name in &["ppdrive", "server"] {
        let symlink = bin_dir.join(name);
        let target = install_dir.join(name);
        if !target.exists() {
            continue;
        }
        // Remove existing symlink or file
        if symlink.exists() || symlink.symlink_metadata().is_ok() {
            let _ = std::fs::remove_file(&symlink);
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &symlink)?;
        println!("Linked {name} -> {}", target.display());
    }

    Ok(())
}
