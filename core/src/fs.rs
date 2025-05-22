use std::path::{Path, PathBuf};

use crate::{CoreResult, errors::CoreError};

pub(super) async fn move_file(src: &Option<PathBuf>, dest: &Path) -> CoreResult<()> {
    if let Err(err) = tokio::fs::File::create(dest).await {
        tracing::info!("unable to create destination file: {err}");
        return Err(CoreError::IoError(err));
    }

    if let Some(src) = src {
        tokio::fs::copy(&src, dest).await?;
        tokio::fs::remove_file(&src).await?;
    }

    Ok(())
}
