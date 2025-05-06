use std::env::set_var;

use chacha20poly1305::{aead::OsRng, AeadCore, KeyInit, XChaCha20Poly1305};

use crate::errors::AppError;

pub const SECRET_KEY: &str = "PPDRIVE_SECRET";
pub const NONCE_KEY: &str = "PPDRIVE_NONCE";
pub const JWT_KEY: &str = "PPDRIVE_JWT_SECRET";
pub const BEARER_KEY: &str = "PPDRIVE_BEARER_KEY";

pub fn secret_generator() -> Result<(), AppError> {
    let secret_key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let secret_api = XChaCha20Poly1305::generate_key(&mut OsRng);

    let secret_key = String::from_utf8(secret_key.to_vec())?;
    let nonce = String::from_utf8(nonce.to_vec())?;
    let secret_api = String::from_utf8(secret_api.to_vec())?;

    set_var(SECRET_KEY, secret_key);
    set_var(NONCE_KEY, nonce);
    set_var(JWT_KEY, secret_api);
    set_var(BEARER_KEY, "Bearer");

    Ok(())
}
