use std::path::{Path, PathBuf};


use crate::{AppResult, errors::Error};
use std::{io::SeekFrom};

use chacha20poly1305::{aead::OsRng, AeadCore, KeyInit, XChaCha20Poly1305};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

pub const SECRETS_FILENAME: &str = ".ppdrive_secret";

/// App secrets sharable across [AppState](crate::AppState).
pub struct AppSecrets {
    secret_key: Vec<u8>,
    secret_nonce: Vec<u8>,
    jwt_secret: Vec<u8>,
}

impl AppSecrets {
    /// Read app secrets from secret file
    pub async fn read() -> AppResult<Self> {
        let secret_file = secret_filename()?;
        let mut secrets = tokio::fs::File::open(&secret_file).await?;
        
        let mut secret_key = [0; 32];
        let mut nonce = [0; 24];
        let mut jwt_secret = [0; 32];
        
        secrets.read_exact(&mut secret_key).await?;
        
        secrets.seek(SeekFrom::Start(32)).await?;
        secrets.read_exact(&mut nonce).await?;
        
        secrets.seek(SeekFrom::Start(32 + 24)).await?;
        secrets.read_exact(&mut jwt_secret).await?;

        Ok(Self {
            secret_key: Vec::from(secret_key),
            secret_nonce: Vec::from(nonce),
            jwt_secret: Vec::from(jwt_secret),
        })
    }

    pub fn secret_key(&self) -> &[u8] {
        self.secret_key.as_slice()
    }

    pub fn nonce(&self) -> &[u8] {
        self.secret_nonce.as_slice()
    }

    pub fn jwt_secret(&self) -> &[u8] {
        self.jwt_secret.as_slice()
    }
}

pub fn secret_filename() -> AppResult<PathBuf> {
    let path = root_dir()?.join(SECRETS_FILENAME);
    Ok(path)
}

pub async fn generate_secret_file() -> AppResult<()> {
    let secret_key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let jwt_secret = XChaCha20Poly1305::generate_key(&mut OsRng);

    let secret_file = secret_filename()?;
    let mut secrets = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&secret_file)
        .await?;

    secrets.write_all(secret_key.as_slice()).await?;
    secrets.write_all(nonce.as_slice()).await?;
    secrets.write_all(jwt_secret.as_slice()).await?;

    Ok(())
}

pub fn root_dir() -> AppResult<PathBuf> {
    let path = if cfg!(debug_assertions) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = path.parent().ok_or(Error::ServerError("unable to get root dir".to_string()))?;

        path.to_path_buf()
    } else {
        let exec_path = std::env::current_exe()?;
        let path = exec_path
            .parent()
            .ok_or(Error::ServerError("unable to get install dir".to_string()))?;

        path.to_owned()
    };

    Ok(path)
}

/// compute total size (in bytes) of a folder.
pub async fn get_folder_size(folder_path: &str, size: &mut u64) -> Result<(), Error> {
    let path = Path::new(folder_path);

    if path.is_file() {
        return Err(Error::ServerError(
            "provided path is not a folder path".to_string(),
        ));
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

pub fn mb_to_bytes(value: usize) -> usize {
    value * 1024 * 1000
}

/// If app secret file does not exist, generate it. Mostly useful
/// for app initialization.
pub async fn init_secrets() -> Result<(), Error> {
    let path = secret_filename()?;
    if !path.is_file() {
        generate_secret_file().await.map_err(|err| Error::ServerError(err.to_string()))?;
    }

    Ok(())
}