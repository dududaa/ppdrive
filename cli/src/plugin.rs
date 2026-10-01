use anyhow::Context;
use ppdrive::plugin::{
    PluginEntry, PluginRegistry, plugin_full_id, plugin_lib_name, plugin_short_id,
};
use std::path::{Path, PathBuf};

/// A parsed remote plugin source.
enum RemoteSource {
    /// GitHub repository in `owner/repo` form.
    GitHub(String),
    /// Any other cloneable git URL.
    GitUrl(String),
}

impl RemoteSource {
    /// Normalized representation stored in the plugin registry.
    fn store(&self) -> String {
        match self {
            RemoteSource::GitHub(repo) => format!("github:{repo}"),
            RemoteSource::GitUrl(url) => url.clone(),
        }
    }
}

/// How a stored `source` value should be interpreted.
enum SourceKind {
    Local,
    Remote(RemoteSource),
}

fn parse_remote_source(source: &str) -> anyhow::Result<RemoteSource> {
    let s = source.trim().trim_end_matches('/');

    if let Some(rest) = s.strip_prefix("github:") {
        let rest = rest.trim_start_matches('/');
        if rest.split('/').count() == 2 {
            return Ok(RemoteSource::GitHub(rest.to_string()));
        }
        anyhow::bail!("invalid GitHub source '{source}': expected 'github:owner/repo'");
    }

    if let Some(rest) = s
        .strip_prefix("https://github.com/")
        .or_else(|| s.strip_prefix("http://github.com/"))
    {
        let rest = rest.trim_end_matches(".git").trim_end_matches('/');
        return Ok(RemoteSource::GitHub(rest.to_string()));
    }

    if s.contains("://") || s.starts_with("git@") {
        return Ok(RemoteSource::GitUrl(s.to_string()));
    }

    if looks_like_github_repo(s) {
        return Ok(RemoteSource::GitHub(s.to_string()));
    }

    anyhow::bail!(
        "could not parse remote source '{source}': expected 'owner/repo', \
         'github:owner/repo', 'https://github.com/owner/repo', or a git URL"
    )
}

/// `owner/repo` shape: exactly two non-empty parts, owner restricted to
/// characters valid in GitHub account names (so paths like `../router`
/// or `/abs/path` are not mistaken for repositories).
fn looks_like_github_repo(s: &str) -> bool {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 2 {
        return false;
    }
    let (owner, repo) = (parts[0], parts[1]);
    !owner.is_empty()
        && !repo.is_empty()
        && !owner.starts_with('.')
        && owner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Classify a stored source string as local or remote.
fn source_kind(source: &str) -> SourceKind {
    if source.starts_with("github:") {
        return match parse_remote_source(source) {
            Ok(remote) => SourceKind::Remote(remote),
            Err(_) => SourceKind::Local,
        };
    }
    if Path::new(source).exists() {
        return SourceKind::Local;
    }
    if source.contains("://") || source.starts_with("git@") {
        return match parse_remote_source(source) {
            Ok(remote) => SourceKind::Remote(remote),
            Err(_) => SourceKind::Local,
        };
    }
    if looks_like_github_repo(source.trim_end_matches('/')) {
        return match parse_remote_source(source) {
            Ok(remote) => SourceKind::Remote(remote),
            Err(_) => SourceKind::Local,
        };
    }
    SourceKind::Local
}

pub async fn execute_add(
    id: &str,
    version: &str,
    local: bool,
    source: Option<&str>,
    build: bool,
) -> Result<(), anyhow::Error> {
    // The full id is `ppdrive-{id}`; the registry canonicalizes and stores
    // the short id (`-` separators) on add.
    let id = plugin_short_id(id);

    let source = source.ok_or_else(|| {
        anyhow::anyhow!(
            "--source is required ({})",
            if local {
                if build {
                    "path to the local Rust source directory"
                } else {
                    "path to the local plugin library file"
                }
            } else {
                "remote repository, e.g. github:owner/repo or https://github.com/owner/repo"
            }
        )
    })?;

    let mut registry = PluginRegistry::load().await?;
    let libs_dir = PluginRegistry::libs_dir()?;
    tokio::fs::create_dir_all(&libs_dir).await?;

    if registry.is_installed(id) {
        println!("Plugin '{id}' is already installed. Use 'ppdrive plugin update {id}' to update.");
        return Ok(());
    }

    let entry = if local {
        install_local(id, source, build, &libs_dir).await?
    } else {
        install_remote(id, source, version, build, &libs_dir).await?
    };

    registry.add(entry);
    registry.save().await?;
    Ok(())
}

/// Download or build a plugin from a remote repository. Returns the new `PluginEntry`.
///
/// `id` may be short (`dashboard`) or full (`ppdrive_dashboard`); the full id
/// drives package/artifact naming while the entry stores the short id.
pub async fn install_remote(
    id: &str,
    source: &str,
    version: &str,
    build: bool,
    libs_dir: &Path,
) -> Result<PluginEntry, anyhow::Error> {
    let remote = parse_remote_source(source)?;
    let short_id = plugin_short_id(id);
    let full_id = plugin_full_id(id);

    if build {
        println!("Downloading {source}...");
        let downloaded = download_repository(&remote, version).await?;

        build_from_source(downloaded.src_dir.to_str().unwrap_or("."), &full_id).await?;
        let lib_path = find_built_lib_upwards(&downloaded.src_dir, &full_id)?;

        let lib_name = plugin_lib_name(&full_id);
        let dest = libs_dir.join(&lib_name);
        tokio::fs::copy(&lib_path, &dest).await.with_context(|| {
            format!(
                "failed to copy {} to {}",
                lib_path.display(),
                dest.display()
            )
        })?;

        let _ = tokio::fs::remove_dir_all(&downloaded.temp_dir).await;

        Ok(PluginEntry {
            id: short_id.to_string(),
            filename: lib_name,
            version: downloaded.version,
            installed_at: chrono::Utc::now().to_rfc3339(),
            source: Some(remote.store()),
            build: true,
            active: true,
        })
    } else {
        let repo = match &remote {
            RemoteSource::GitHub(repo) => repo.clone(),
            RemoteSource::GitUrl(_) => anyhow::bail!(
                "release artifacts can only be downloaded from a GitHub repository; \
                 pass --build to compile '{source}' from source"
            ),
        };

        let (release_version, json) = fetch_github_release(&repo, version).await?;
        let asset = find_release_asset(&json, &full_id)?;

        let download_url = asset["browser_download_url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing download URL for asset"))?;

        let lib_name = plugin_lib_name(&full_id);
        println!("Downloading {lib_name}...");
        download_file(download_url, &libs_dir.join(&lib_name)).await?;

        Ok(PluginEntry {
            id: short_id.to_string(),
            filename: lib_name,
            version: release_version,
            installed_at: chrono::Utc::now().to_rfc3339(),
            source: Some(remote.store()),
            build: false,
            active: true,
        })
    }
}

/// Install a plugin from a local file or a local source directory.
async fn install_local(
    id: &str,
    source: &str,
    build: bool,
    libs_dir: &Path,
) -> Result<PluginEntry, anyhow::Error> {
    let short_id = plugin_short_id(id);
    let full_id = plugin_full_id(id);

    let src_path = if build {
        let source_dir = Path::new(source);
        if !source_dir.is_dir() {
            anyhow::bail!("source directory not found: {source}");
        }

        build_from_source(source, &full_id).await?;
        find_built_lib_upwards(source_dir, &full_id)?
    } else {
        let p = PathBuf::from(source);
        if !p.exists() {
            anyhow::bail!("file not found: {source}");
        }
        p
    };

    let lib_name = plugin_lib_name(&full_id);
    let dest = libs_dir.join(&lib_name);
    tokio::fs::copy(&src_path, &dest).await.with_context(|| {
        format!(
            "failed to copy {} to {}",
            src_path.display(),
            dest.display()
        )
    })?;

    Ok(PluginEntry {
        id: short_id.to_string(),
        filename: lib_name,
        version: "local".to_string(),
        installed_at: chrono::Utc::now().to_rfc3339(),
        source: Some(source.to_string()),
        build,
        active: true,
    })
}

struct DownloadedSource {
    /// Temp directory that owns the extracted source; removed by the caller.
    temp_dir: PathBuf,
    /// Extracted repository root.
    src_dir: PathBuf,
    /// Release version (GitHub) or `"source"` (git URL).
    version: String,
}

/// Download a repository: GitHub releases are fetched as source tarballs,
/// other sources are cloned with `git`.
async fn download_repository(
    remote: &RemoteSource,
    version: &str,
) -> Result<DownloadedSource, anyhow::Error> {
    let temp_dir = ppdrive::root_dir()?.join("tmp_plugin_build");
    if temp_dir.exists() {
        tokio::fs::remove_dir_all(&temp_dir)
            .await
            .context("failed to clean previous build directory")?;
    }
    tokio::fs::create_dir_all(&temp_dir).await?;

    match remote {
        RemoteSource::GitHub(repo) => {
            let (release_version, _) = fetch_github_release(repo, version).await?;
            let url =
                format!("https://github.com/{repo}/archive/refs/tags/v{release_version}.tar.gz");

            let tarball = temp_dir.join("source.tar.gz");
            download_file(&url, &tarball).await?;
            extract_tarball(&tarball, &temp_dir)?;
            let _ = tokio::fs::remove_file(&tarball).await;

            let src_dir = find_extracted_dir(&temp_dir).ok_or_else(|| {
                anyhow::anyhow!(
                    "could not locate extracted source in {}",
                    temp_dir.display()
                )
            })?;

            Ok(DownloadedSource {
                temp_dir,
                src_dir,
                version: release_version,
            })
        }
        RemoteSource::GitUrl(url) => {
            let src_dir = temp_dir.join("repo");
            git_clone(url, &src_dir, version).await?;

            Ok(DownloadedSource {
                temp_dir,
                src_dir,
                version: "source".to_string(),
            })
        }
    }
}

/// Clone a repository, honouring an explicit `version` ref when given.
async fn git_clone(url: &str, dest: &Path, version: &str) -> Result<(), anyhow::Error> {
    let mut branches: Vec<String> = Vec::new();
    if version != "latest" {
        if version.starts_with('v') {
            branches.push(version.to_string());
        } else {
            branches.push(format!("v{version}"));
            branches.push(version.to_string());
        }
    }
    branches.push(String::new()); // default branch

    let url = url.to_string();
    let dest = dest.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let mut last_err = String::new();
        for branch in &branches {
            if dest.exists() {
                let _ = std::fs::remove_dir_all(&dest);
            }
            let mut cmd = std::process::Command::new("git");
            cmd.args(["clone", "--depth", "1"]);
            if !branch.is_empty() {
                cmd.args(["--branch", branch]);
            }
            cmd.arg(&url).arg(&dest);

            match cmd.output() {
                Ok(out) if out.status.success() => return Ok(()),
                Ok(out) => {
                    last_err = String::from_utf8_lossy(&out.stderr).into_owned();
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "failed to run `git` — is git installed?: {e}"
                    ));
                }
            }
        }
        Err(anyhow::anyhow!("git clone failed:\n{last_err}"))
    })
    .await
    .context("failed to spawn git clone task")?
}

/// Fetch release info from the GitHub API. Returns `(release_version, json)`.
async fn fetch_github_release(
    repo: &str,
    version: &str,
) -> Result<(String, serde_json::Value), anyhow::Error> {
    let api_url = if version == "latest" {
        format!("https://api.github.com/repos/{repo}/releases/latest")
    } else {
        format!("https://api.github.com/repos/{repo}/releases/tags/v{version}")
    };

    println!("Fetching release info for {repo}...");

    let body: String = ureq::get(&api_url)
        .header("User-Agent", "ppdrive-plugin-manager")
        .call()
        .with_context(|| format!("failed to fetch release info for {repo}"))?
        .body_mut()
        .read_to_string()
        .context("failed to read GitHub API response")?;

    let json: serde_json::Value =
        serde_json::from_str(&body).context("failed to parse GitHub API response")?;

    let tag_name = json["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing tag_name in release response"))?;

    Ok((tag_name.trim_start_matches('v').to_string(), json))
}

/// Find a release asset whose base name matches `id`.
fn find_release_asset<'a>(
    json: &'a serde_json::Value,
    id: &str,
) -> Result<&'a serde_json::Value, anyhow::Error> {
    let assets = json["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("no assets found in release"))?;

    let expected = plugin_lib_name(id);
    let ext = expected.rsplit('.').next().unwrap_or("so");
    let suffix = format!(".{ext}");
    // Compare with `_` normalized to `-` so assets published under the
    // legacy underscore convention (`ppdrive_image_conversion-...`) still match.
    let norm = |s: &str| s.replace('_', "-");
    let expected_norm = norm(&expected);
    let prefix_norm = norm(&format!("{id}-"));

    assets
        .iter()
        .find(|a| a["name"].as_str() == Some(expected.as_str()))
        .or_else(|| {
            assets.iter().find(|a| {
                a["name"]
                    .as_str()
                    .is_some_and(|name| norm(name) == expected_norm)
            })
        })
        .or_else(|| {
            assets.iter().find(|a| {
                a["name"].as_str().is_some_and(|name| {
                    let name = norm(name);
                    name.starts_with(&prefix_norm) && name.ends_with(&suffix)
                })
            })
        })
        .ok_or_else(|| {
            let available: Vec<&str> = assets.iter().filter_map(|a| a["name"].as_str()).collect();
            anyhow::anyhow!(
                "no release asset based on '{id}' (expected '{expected}'). Available: {available:?}"
            )
        })
}

pub async fn execute_update(id: Option<&str>) -> Result<(), anyhow::Error> {
    let mut registry = PluginRegistry::load().await?;
    let libs_dir = PluginRegistry::libs_dir()?;
    tokio::fs::create_dir_all(&libs_dir).await?;

    let ids: Vec<String> = if let Some(id) = id {
        vec![plugin_short_id(id).to_string()]
    } else {
        registry.list().iter().map(|p| p.id.clone()).collect()
    };

    if ids.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    let mut updated = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for plugin_id in &ids {
        let entry = match registry.get(plugin_id) {
            Some(e) => e.clone(),
            None => {
                println!("  {plugin_id}: not installed, skipping");
                skipped += 1;
                continue;
            }
        };

        let source = entry.source.clone();
        let Some(src) = source.as_deref() else {
            println!("  {plugin_id}: unknown source, use 'ppdrive plugin add' to reinstall");
            skipped += 1;
            continue;
        };

        match source_kind(src) {
            SourceKind::Remote(remote) => {
                let remote = remote.store();
                println!("Updating {plugin_id} from {remote}...");
                match install_remote(plugin_id, &remote, "latest", entry.build, &libs_dir).await {
                    Ok(mut new_entry) => {
                        // Remove old lib file if filename changed
                        let old_lib = libs_dir.join(&entry.filename);
                        if old_lib.exists() && entry.filename != new_entry.filename {
                            let _ = tokio::fs::remove_file(&old_lib).await;
                        }
                        // Updating must not flip the activation state.
                        new_entry.active = entry.active;
                        registry.remove(plugin_id);
                        registry.add(new_entry);
                        updated += 1;
                        println!("  {plugin_id}: updated");
                    }
                    Err(e) => {
                        println!("  {plugin_id}: failed to update: {e}");
                        failed += 1;
                    }
                }
            }
            SourceKind::Local => {
                if entry.build {
                    let full_id = plugin_full_id(plugin_id);
                    println!("Rebuilding {plugin_id} from {src}...");
                    match build_from_source(src, &full_id).await {
                        Ok(()) => match find_built_lib_upwards(Path::new(src), &full_id) {
                            Ok(lib) => {
                                let dest = libs_dir.join(&entry.filename);
                                tokio::fs::copy(&lib, &dest).await?;
                                registry.remove(plugin_id);
                                let updated_entry = PluginEntry {
                                    version: "local".to_string(),
                                    installed_at: chrono::Utc::now().to_rfc3339(),
                                    ..entry
                                };
                                registry.add(updated_entry);
                                updated += 1;
                                println!("  {plugin_id}: rebuilt");
                            }
                            Err(e) => {
                                println!("  {plugin_id}: {e}");
                                failed += 1;
                            }
                        },
                        Err(e) => {
                            println!("  {plugin_id}: build failed: {e}");
                            failed += 1;
                        }
                    }
                } else {
                    // Re-copy from stored path
                    let p = Path::new(src);
                    if p.exists() {
                        let dest = libs_dir.join(&entry.filename);
                        tokio::fs::copy(p, &dest).await?;
                        registry.remove(plugin_id);
                        let updated_entry = PluginEntry {
                            version: "local".to_string(),
                            installed_at: chrono::Utc::now().to_rfc3339(),
                            ..entry
                        };
                        registry.add(updated_entry);
                        updated += 1;
                        println!("  {plugin_id}: re-copied from {src}");
                    } else {
                        println!("  {plugin_id}: source not found at {src}");
                        failed += 1;
                    }
                }
            }
        }
    }

    registry.save().await?;
    println!("\nDone. {updated} updated, {skipped} skipped, {failed} failed.");
    Ok(())
}

/// `cargo build --release --lib --package {package}` in `source_dir`.
///
/// The plugin package itself carries the cdylib entry point
/// (`ppdrive-image-conversion` produces `libimage_conversion.so`).
async fn build_from_source(source_dir: &str, package: &str) -> Result<(), anyhow::Error> {
    run_cargo_build(source_dir, package).await?;
    println!("cargo build --release --lib --package {package} finished.");
    Ok(())
}

/// Run `cargo build --release --lib --package {package}` in `source_dir`,
/// reporting the captured stderr when cargo exits non-zero.
async fn run_cargo_build(source_dir: &str, package: &str) -> Result<(), anyhow::Error> {
    println!("Building package '{package}'...");

    let source_dir = source_dir.to_string();
    let package = package.to_string();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("cargo")
            .args(["build", "--release", "--lib", "--package", &package])
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

    Ok(())
}

/// Locate the built library, walking up from `from` in case the workspace
/// `target/` directory sits above the package directory.
fn find_built_lib_upwards(from: &Path, name: &str) -> Result<PathBuf, anyhow::Error> {
    let mut dir = Some(from);
    while let Some(d) = dir {
        let target_dir = d.join("target/release");
        if target_dir.exists()
            && let Ok(found) = find_built_lib(&target_dir, name)
        {
            return Ok(found);
        }
        dir = d.parent();
    }

    Err(anyhow::anyhow!(
        "no built library found for '{name}' in any target/release directory above {}",
        from.display()
    ))
}

fn find_built_lib(target_dir: &Path, name: &str) -> Result<PathBuf, anyhow::Error> {
    let os = std::env::consts::OS;
    let ext = match os {
        "linux" => "so",
        "macos" => "dylib",
        "windows" => "dll",
        _ => "so",
    };
    let suffix = format!(".{ext}");

    // Common output names in both separator styles: cargo writes cdylib
    // file names with `-` converted to `_`.
    let underscored = name.replace('-', "_");
    let candidates = [
        target_dir.join(format!("lib{name}.{ext}")),
        target_dir.join(format!("{name}.{ext}")),
        target_dir.join(format!("lib{underscored}.{ext}")),
        target_dir.join(format!("{underscored}.{ext}")),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    // Match by normalized file stem against the full id (`ppdrive-dashboard`)
    // or the short id (`image-conversion`), accepting either separator.
    let norm = |s: &str| s.replace('_', "-");
    let wanted = [norm(name), norm(plugin_short_id(name))];

    let mut libs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(target_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if !file_name.ends_with(suffix.as_str()) {
                continue;
            }
            let stem = &file_name[..file_name.len() - suffix.len()];
            let stem = stem.strip_prefix("lib").unwrap_or(stem);
            let stem_norm = norm(stem);
            if wanted.contains(&stem_norm) {
                return Ok(entry.path());
            }
            if file_name.starts_with("lib") {
                libs.push(entry.path());
            }
        }
    }

    // A single cdylib in the directory is unambiguous (plugin lib target
    // names may differ from the package name).
    if libs.len() == 1 {
        return Ok(libs.remove(0));
    }

    Err(anyhow::anyhow!(
        "no built library found in {} matching '{name}' (found: {:?})",
        target_dir.display(),
        libs.iter()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy())
            .collect::<Vec<_>>()
    ))
}

/// The GitHub source archive extracts into a single directory; return it.
fn find_extracted_dir(temp_dir: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(temp_dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();

    match dirs.len() {
        1 => dirs.pop(),
        _ => None,
    }
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

fn extract_tarball(
    tarball_path: &std::path::Path,
    dest_dir: &std::path::Path,
) -> Result<(), anyhow::Error> {
    use flate2::read::GzDecoder;
    use std::fs::File;
    use tar::Archive;

    let file = File::open(tarball_path)
        .with_context(|| format!("failed to open {}", tarball_path.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(dest_dir)
        .context("failed to extract tarball")?;
    Ok(())
}

pub async fn execute_list() -> Result<(), anyhow::Error> {
    let registry = PluginRegistry::load().await?;
    let plugins = registry.list();

    if plugins.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    println!(
        "{:<30} {:<10} {:<40} {:<8} INSTALLED",
        "ID", "VERSION", "SOURCE", "ACTIVE"
    );
    println!("{}", "-".repeat(119));
    for p in plugins {
        let source = p.source.as_deref().unwrap_or("-");
        println!(
            "{:<30} {:<10} {:<40} {:<8} {}",
            p.full_id(),
            p.version,
            source,
            if p.active { "yes" } else { "no" },
            p.installed_at
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

/// Persist the `active` flag for an installed plugin.
async fn set_plugin_active(id: &str, active: bool) -> Result<(), anyhow::Error> {
    let mut registry = PluginRegistry::load().await?;

    let Some(entry) = registry.get(id) else {
        return Err(anyhow::anyhow!("plugin '{id}' is not installed"));
    };
    let current = entry.active;

    if current == active {
        let state = if active { "active" } else { "inactive" };
        println!("Plugin '{id}' is already {state}.");
        return Ok(());
    }

    // Guaranteed `Some`: the id was found above.
    let _ = registry.set_active(id, active);
    registry.save().await?;

    let action = if active { "activated" } else { "deactivated" };
    println!("Plugin '{id}' {action}. Restart the server to apply the change.");
    Ok(())
}

/// Activate an installed plugin so the server loads it on next start.
pub async fn execute_activate(id: &str) -> Result<(), anyhow::Error> {
    set_plugin_active(id, true).await
}

/// Deactivate an installed plugin so the server skips it on next start.
pub async fn execute_deactivate(id: &str) -> Result<(), anyhow::Error> {
    set_plugin_active(id, false).await
}
