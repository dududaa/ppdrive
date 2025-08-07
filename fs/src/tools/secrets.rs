use std::{io::SeekFrom, path::PathBuf};

use chacha20poly1305::{AeadCore, KeyInit, XChaCha20Poly1305, aead::OsRng};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::CoreResult;

use super::install_dir;
pub const SECRETS_FILENAME: &str = ".ppdrive_secret";

/// App secrets sharable across [AppState](crate::AppState).
pub struct AppSecrets {
    secret_key: Vec<u8>,
    secret_nonce: Vec<u8>,
    jwt_secret: Vec<u8>,
}

impl AppSecrets {
    /// Read app secrets from secret file
    pub async fn read() -> CoreResult<Self> {
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

pub fn secret_filename() -> CoreResult<PathBuf> {
    let path = if cfg!(debug_assertions) {
        SECRETS_FILENAME.into()
    } else {
        install_dir()?.join(SECRETS_FILENAME)
    };

    Ok(path)
}

pub async fn generate_secret_file() -> CoreResult<()> {
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
