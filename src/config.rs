use chacha20poly1305::{aead::OsRng, AeadCore, KeyInit, XChaCha20Poly1305};

use crate::errors::AppError;

/// App configurations sharable across [AppState](crate::AppState).
pub struct AppConfig {
    secret_key: Vec<u8>,
    secret_nonce: Vec<u8>,
}

impl AppConfig {
    pub fn build() -> Result<Self, AppError> {
        let public_key = XChaCha20Poly1305::generate_key(&mut OsRng);
        let private_key = XChaCha20Poly1305::generate_nonce(&mut OsRng);

        Ok(Self {
            secret_key: public_key.to_vec(),
            secret_nonce: private_key.to_vec(),
        })
    }

    pub fn secret_key(&self) -> &[u8] {
        self.secret_key.as_slice()
    }

    pub fn nonce(&self) -> &[u8] {
        self.secret_nonce.as_slice()
    }
}
