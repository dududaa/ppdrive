use std::path::Path;

use crate::FsResult;

pub async fn move_file(src: impl AsRef<Path>, dest: impl AsRef<Path>) -> FsResult<()> {
    if !dest.as_ref().is_file() {
        tokio::fs::File::create(&dest).await?;
    }

    tokio::fs::copy(&src, &dest).await?;
    tokio::fs::remove_file(&src).await?;

    Ok(())
}
