use std::path::PathBuf;

use chacha20poly1305::{aead::OsRng, AeadCore, KeyInit, XChaCha20Poly1305};
use tokio::io::AsyncWriteExt;

use crate::{errors::AppError, utils::install_dir};

pub const BEARER_KEY: &str = "PPDRIVE_BEARER_KEY";
pub const BEARER_VALUE: &str = "Bearer";
pub const SECRETS_FILENAME: &str = ".ppdrive_secret";

pub fn secret_filename() -> Result<PathBuf, AppError> {
    let path = if cfg!(debug_assertions) {
        SECRETS_FILENAME.into()
    } else {
        install_dir()?.join(SECRETS_FILENAME)
    };

    Ok(path)
}

pub async fn generate_secret() -> Result<(), AppError> {
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

/// If app secret file does not exist, generate it. Mostly useful
/// on app initialization.
pub async fn generate_secrets_init() -> Result<(), AppError> {
    let path = secret_filename()?;
    if !path.is_file() {
        generate_secret().await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{errors::AppError, utils::tools::secrets::generate_secret};

    #[tokio::test]
    async fn test_client_keygen() -> Result<(), AppError> {
        let keygen = generate_secret().await;

        if let Err(err) = &keygen {
            println!("keygen err: {err}")
        }

        assert!(keygen.is_ok());
        Ok(())
    }
}
