use chacha20poly1305::{aead::OsRng, AeadCore, KeyInit, XChaCha20Poly1305};

use crate::{
    errors::AppError,
    utils::{
        get_env,
        tools::keygen::{JWT_KEY, NONCE_KEY, SECRET_KEY},
    },
};

/// App configurations sharable across [AppState](crate::AppState).
pub struct AppConfig {
    secret_key: Vec<u8>,
    secret_nonce: Vec<u8>,
    jwt_secret: Vec<u8>,
}

impl AppConfig {
    pub fn build() -> Result<Self, AppError> {
        let public_key = get_env(SECRET_KEY)?;
        let public_key = Vec::from(public_key.as_bytes());

        let nonce = get_env(NONCE_KEY)?;
        let nonce = Vec::from(nonce.as_bytes());

        let jwt_secret = get_env(JWT_KEY)?;
        let jwt_secret = Vec::from(jwt_secret.as_bytes());

        Ok(Self {
            secret_key: public_key.to_vec(),
            secret_nonce: nonce.to_vec(),
            jwt_secret,
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
