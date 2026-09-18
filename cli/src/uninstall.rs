use ppdrive::root_dir;
use std::path::{Path, PathBuf};

pub async fn execute(purge: bool) -> Result<(), anyhow::Error> {
    let install_dir = root_dir()?;
    println!("Install directory: {}", install_dir.display());

    // Find and list symlinks
    let symlink_dir = symlink_directory();
    let symlinks = find_symlinks(&symlink_dir, &install_dir);

    if !symlinks.is_empty() {
        println!("\nSymlinks found:");
        for (link, target) in &symlinks {
            println!("  {} -> {}", link.display(), target.display());
        }
    }

    // Data files to optionally remove
    let data_files = vec![
        install_dir.join("ppd_config.toml"),
        install_dir.join(".ppdrive_secret"),
        install_dir.join("data.db"),
    ];

    let existing_data: Vec<&PathBuf> = data_files.iter().filter(|f| f.exists()).collect();

    if !purge && !existing_data.is_empty() {
        println!("\nData files found:");
        for f in &existing_data {
            println!("  {}", f.display());
        }
        print!("\nRemove data files? [y/N]: ");
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "y" {
            println!("Keeping data files.");
        } else {
            remove_data_files(&existing_data);
        }
    } else if purge {
        remove_data_files(&existing_data);
    }

    // Remove symlinks
    for (link, _) in &symlinks {
        if let Err(e) = std::fs::remove_file(link) {
            println!("Warning: failed to remove {}: {e}", link.display());
        } else {
            println!("Removed {}", link.display());
        }
    }

    // Remove binaries
    for name in &["ppdrive", "server"] {
        let path = install_dir.join(name);
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                println!("Warning: failed to remove {}: {e}", path.display());
            } else {
                println!("Removed {}", path.display());
            }
        }
    }

    println!("\nUninstalled ppdrive successfully.");

    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let bin_dir = format!("{home}/.local/bin");
        if !std::path::Path::new(&bin_dir).exists() {
            return Ok(());
        }
        if let Ok(path) = std::env::var("PATH") {
            if !path.split(':').any(|p| p == bin_dir) {
                println!("Note: {bin_dir} is in your PATH but no longer contains ppdrive binaries.");
            }
        }
    }

    Ok(())
}

fn symlink_directory() -> PathBuf {
    #[cfg(unix)]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".local/bin");
        }
    }
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("ppdrive");
        }
    }
    PathBuf::new()
}

fn find_symlinks(symlink_dir: &Path, _install_dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut result = Vec::new();
    if !symlink_dir.exists() {
        return result;
    }

    for name in &["ppdrive", "server"] {
        #[cfg(unix)]
        {
            let link = symlink_dir.join(name);
            if let Ok(meta) = std::fs::symlink_metadata(&link) {
                if meta.file_type().is_symlink() {
                    if let Ok(target) = std::fs::read_link(&link) {
                        if target.exists() {
                            result.push((link, target));
                        }
                    }
                }
            }
        }
    }

    result
}

fn remove_data_files(files: &[&PathBuf]) {
    for f in files {
        if f.exists() {
            if let Err(e) = std::fs::remove_file(f) {
                println!("Warning: failed to remove {}: {e}", f.display());
            } else {
                println!("Removed {}", f.display());
            }
        }
    }
}
