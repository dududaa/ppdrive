//! Utility functions and configuration for PPDRIVE.
//!
//! Provides filesystem helpers, configuration loading, secrets management,
//! and cryptographic hashing.

pub mod config;
pub mod secrets;

#[cfg(feature = "server")]
pub mod hasher;

use anyhow::anyhow;
use std::path::{Path, PathBuf};

/// Returns the project root directory.
///
/// In debug builds this is the workspace root (`CARGO_MANIFEST_DIR` parent).
/// In release builds it is the directory containing the running executable.
pub fn root_dir() -> anyhow::Result<PathBuf> {
    let path = if cfg!(debug_assertions) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = path.parent().ok_or(anyhow!("unable to get root dir"))?;

        path.to_path_buf()
    } else {
        let exec_path = std::env::current_exe()?;
        let path = exec_path
            .parent()
            .ok_or(anyhow!("unable to get install dir"))?;

        path.to_owned()
    };

    Ok(path)
}

/// Compute the total size in bytes of a folder recursively.
pub async fn get_folder_size(folder_path: &str, size: &mut u64) -> anyhow::Result<()> {
    let path = std::path::Path::new(folder_path);

    let meta = tokio::fs::metadata(path).await?;
    if meta.is_file() {
        return Err(anyhow!("provided path is not a folder path"));
    }

    let mut rd = tokio::fs::read_dir(path).await?;

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let m = tokio::fs::metadata(&path).await?;
        if m.is_file() {
            *size += m.len();
        } else if let Some(folder) = path.to_str() {
            Box::pin(get_folder_size(folder, size)).await?;
        }
    }

    Ok(())
}

/// Convert megabytes to bytes.
pub fn mb_to_bytes(value: f64) -> usize {
    (value * 1024.0 * 1024.0).round() as usize
}

/// Check whether two paths cross each other (one is a prefix/parent of the other, or identical).
///
/// Paths are normalized by stripping leading `/` and compared component-by-component.
/// For example, `"uploads"` crosses `"uploads/images"`, and `"/assets"` crosses `"assets"`.
pub fn paths_cross(a: &str, b: &str) -> bool {
    fn normalize(s: &str) -> Vec<&str> {
        s.trim_start_matches('/')
            .split('/')
            .filter(|c| !c.is_empty())
            .collect()
    }

    let a_parts = normalize(a);
    let b_parts = normalize(b);

    if a_parts == b_parts {
        return true;
    }

    if a_parts.len() < b_parts.len() {
        a_parts.iter().zip(b_parts.iter()).all(|(x, y)| x == y)
    } else {
        b_parts.iter().zip(a_parts.iter()).all(|(x, y)| x == y)
    }
}

/// Generate a cryptographically random hex-encoded ID of the given byte length.
pub fn generate_nano_id(size: usize) -> String {
    let alphabet: [char; 16] = [
        '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    nanoid::nanoid!(size, &alphabet)
}

/// Cleanup stale temporary files older than the given duration.
///
/// Removes files in the `tmp/` directory that haven't been modified
/// within `max_age`. Returns the number of files removed.
pub async fn cleanup_tmp_files(max_age: std::time::Duration) -> anyhow::Result<usize> {
    let tmp_dir = root_dir()?.join("tmp");
    if !tokio::fs::metadata(&tmp_dir).await.map(|m| m.is_dir()).unwrap_or(false) {
        return Ok(0);
    }

    let mut removed = 0;
    let mut entries = tokio::fs::read_dir(&tmp_dir).await?;
    let now = std::time::SystemTime::now();

    while let Ok(Some(entry)) = entries.next_entry().await {
        if let Ok(metadata) = entry.metadata().await
            && let Ok(modified) = metadata.modified()
                && now.duration_since(modified).unwrap_or_default() > max_age {
                    if let Err(err) = tokio::fs::remove_file(entry.path()).await {
                        tracing::warn!("failed to remove stale tmp file {:?}: {err}", entry.path());
                    } else {
                        removed += 1;
                    }
                }
    }

    if removed > 0 {
        tracing::info!(removed, "cleaned up stale temporary files");
    }

    Ok(removed)
}


