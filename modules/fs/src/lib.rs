use std::path::Path;

use mime_guess::Mime;
use ppd_bk::{
    RBatis,
    models::asset::{AssetType, Assets},
};

use crate::errors::Error;

#[cfg(feature = "auth")]
pub mod auth;
mod errors;
pub mod free;
pub mod opts;
pub mod utils;

pub type FsResult<T> = Result<T, crate::errors::Error>;

pub enum FileResponse {
    File(Mime, Vec<u8>),
    Folder(String),
}

pub async fn read_asset(
    db: &RBatis,
    asset_path: &str,
    asset_type: &AssetType,
    user_id: &Option<u64>,
) -> FsResult<FileResponse> {
    let asset = Assets::get_by_path(db, asset_path, asset_type).await?;

    // if asset has custom path and custom path is not provided in url,
    // we return an error. The purpose of custom path is to conceal the
    // original path
    if let Some(custom_path) = asset.custom_path() {
        if custom_path != asset_path {
            return Err(Error::NotFound("asset not found".to_string()));
        }
    }

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

                let resp = FileResponse::File(mime_type, content);
                Ok(resp)
            } else {
                Err(Error::NotFound(format!(
                    "asset record found but path '{asset_path}' does not exist if filesystem for '{asset_type}'."
                )))
            }
        }
        AssetType::Folder => {
            if path.exists() && path.is_dir() {
                let mut contents = tokio::fs::read_dir(path).await?;
                let mut filenames = Vec::new();

                // let's attempt to read folder contents, checking for
                // asset ownership all along
                while let Ok(Some(entry)) = contents.next_entry().await {
                    let path = entry.path();
                    let filename = entry.file_name();

                    if let (Some(path_str), Some(filename)) = (path.to_str(), filename.to_str()) {
                        let asset_type = if path.is_file() {
                            AssetType::File
                        } else {
                            AssetType::Folder
                        };

                        let asset = Assets::get_by_path(db, path_str, &asset_type).await;
                        if let Ok(asset) = asset {
                            let html =
                                format!("<li><a href='/{}'>{filename}</a></li>", asset.url_path());

                            if *asset.public() {
                                filenames.push(html);
                            } else {
                                if let Some(user_id) = user_id {
                                    let can_read = asset.can_read(db, user_id).await;
                                    if (user_id == asset.user_id()) || can_read.is_ok() {
                                        filenames.push(html);
                                    }
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

                let resp = FileResponse::Folder(body);
                Ok(resp)
            } else {
                Err(Error::NotFound(format!(
                    "asset record found but path '{asset_path}' does not exist for '{asset_type}'."
                )))
            }
        }
    }
}
