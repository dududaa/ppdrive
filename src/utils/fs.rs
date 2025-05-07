use std::path::Path;

use crate::errors::AppError;

/// compute total size (in bytes) of a folder.
pub async fn check_folder_size(folder_path: &str, size: &mut u64) -> Result<(), AppError> {
    let path = Path::new(folder_path);

    if path.is_file() {
        return Err(AppError::IOError(
            "provided path is not a folder path".to_string(),
        ));
    }

    let mut rd = tokio::fs::read_dir(path).await?;

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();

        if path.is_file() {
            let m = path.metadata()?;
            *size += m.len()
        } else {
            if let Some(folder) = path.to_str() {
                Box::pin(check_folder_size(folder, size)).await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{errors::AppError, utils::fs::check_folder_size};

    #[tokio::test]
    async fn test_check_folder_size() -> Result<(), AppError> {
        let mut size = 0;
        let check = check_folder_size("docs", &mut size).await;

        if let Err(err) = &check {
            println!("check folder failed: {err}");
        }

        println!("folder size {size}");

        Ok(())
    }
}
