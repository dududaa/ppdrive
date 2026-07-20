pub mod config;
pub mod secrets;
pub mod hasher;

use anyhow::anyhow;
use std::path::{Path, PathBuf};
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

/// compute total size (in bytes) of a folder.
pub async fn get_folder_size(folder_path: &str, size: &mut u64) -> anyhow::Result<()> {
    let path = Path::new(folder_path);

    if path.is_file() {
        return Err(anyhow!("provided path is not a folder path",));
    }

    let mut rd = tokio::fs::read_dir(path).await?;

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();

        if path.is_file() {
            let m = path.metadata()?;
            *size += m.len()
        } else if let Some(folder) = path.to_str() {
            Box::pin(get_folder_size(folder, size)).await?;
        }
    }

    Ok(())
}

pub fn mb_to_bytes(value: f64) -> usize {
    let bytes = (value * 1024.0 * 1000.0).round();
    let bytes = bytes.to_le_bytes();

    usize::from_le_bytes(bytes)
}

pub fn generate_nano_id(size: usize) -> String {
    let alphabet: [char; 16] = [
        '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    nanoid::nanoid!(size, &alphabet)
}


