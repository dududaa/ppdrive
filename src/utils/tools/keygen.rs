use std::env::set_var;

use chacha20poly1305::{aead::OsRng, AeadCore, KeyInit, XChaCha20Poly1305};
use tokio::io::AsyncWriteExt;

use crate::errors::AppError;

pub const BEARER_KEY: &str = "PPDRIVE_BEARER_KEY";
pub const SECRET_FILE: &str = ".ppdrive_secret";

pub async fn secret_generator() -> Result<(), AppError> {
    let secret_key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let jwt_secret = XChaCha20Poly1305::generate_key(&mut OsRng);

    let mut secrets = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(SECRET_FILE)
        .await?;

    secrets.write_all(secret_key.as_slice()).await?;
    secrets.write_all(nonce.as_slice()).await?;
    secrets.write_all(jwt_secret.as_slice()).await?;

    set_var(BEARER_KEY, "Bearer");

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{errors::AppError, utils::tools::keygen::secret_generator};

    #[tokio::test]
    async fn test_client_keygen() -> Result<(), AppError> {
        let keygen = secret_generator().await;

        if let Err(err) = &keygen {
            println!("keygen err: {err}")
        }

        assert!(keygen.is_ok());
        Ok(())
    }
}
