use std::path::Path;

use mime_guess::Mime;
use ppd_bk::{
    RBatis,
    models::asset::{AssetType, Assets},
};

use crate::errors::Error;

#[cfg(feature = "auth")]
pub mod auth;

#[cfg(not(feature = "auth"))]
pub mod free;

pub mod errors;
pub mod opts;
mod utils;

pub type FsResult<T> = Result<T, Error>;

pub enum AssetBody {
    File(Mime, Vec<u8>),
    Folder(String),
}

pub async fn read_asset(
    db: &RBatis,
    slug: &str,
    user_id: &Option<u64>,
) -> FsResult<AssetBody> {
    
    let asset = Assets::get_by_slug(db, slug).await?;
    let asset_type = asset.asset_type();

    // check if current user has read permission
    if !asset.public() {
        match user_id {
            Some(user_id) => {
                let can_read = asset.can_read(db, user_id).await;
                if (user_id != asset.user_id()) && can_read.is_err() {
                    return Err(Error::PermissionError("permission denied".to_string()));
                }
            }
            None => {
                return Err(Error::PermissionError("permission denied".to_string()));
            }
        }
    }

    let path = Path::new(asset.path());
    match asset_type {
        AssetType::File => {
            if path.exists() && path.is_file() {
                let content = tokio::fs::read(path).await?;
                let mime_type = mime_guess::from_path(path).first_or_octet_stream();

                let resp = AssetBody::File(mime_type, content);
                Ok(resp)
            } else {
                Err(Error::NotFound(format!(
                    "asset record found but path '{slug}' does not exist if filesystem for '{asset_type}'."
                )))
            }
        }
        AssetType::Folder => {
            if path.exists() && path.is_dir() {
                let mut contents = tokio::fs::read_dir(path).await?;
                let mut filenames = Vec::new();

                // let's attempt to read folder contents, checking for asset ownership all along.
                while let Ok(Some(entry)) = contents.next_entry().await {
                    let path = entry.path();
                    let filename = entry.file_name();

                    if let (Some(path_str), Some(filename)) = (path.to_str(), filename.to_str()) {
                        let asset_type = if path.is_file() {
                            AssetType::File
                        } else {
                            AssetType::Folder
                        };

                        let asset = Assets::select_by_path(db, path_str, (&asset_type).into())
                            .await
                            .map_err(|err| Error::ServerError(err.to_string()))?;
                        if let Some(asset) = asset {
                            let html =
                                format!("<li><a href='/{}'>{filename}</a></li>", asset.slug());

                            if *asset.public() {
                                filenames.push(html);
                            } else if let Some(user_id) = user_id {
                                let can_read = asset.can_read(db, user_id).await;
                                if (user_id == asset.user_id()) || can_read.is_ok() {
                                    filenames.push(html);
                                }
                            }
                        }
                    }
                }

                let content = if filenames.is_empty() {
                    "<p>No content found.</p>".to_string()
                } else {
                    format!(r#"<ul>{}</ul>"#, filenames.join("\n"))
                };

                let body = format!(
                    r#"
                    <DOCTYPE! html>
                    <html>
                        {content}
                    </html>
                "#
                );

                let resp = AssetBody::Folder(body);
                Ok(resp)
            } else {
                Err(Error::NotFound(format!(
                    "asset record found but path '{slug}' does not exist for '{asset_type}'."
                )))
            }
        }
    }
}
