use chacha20poly1305::{
    aead::{rand_core::RngCore, Aead, OsRng},
    KeyInit, XChaCha20Poly1305, XNonce,
};

pub mod sqlx_ext;
pub mod sqlx_utils;

use crate::{errors::AppError, models::client::Client, state::AppState};

pub fn get_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|err| {
        tracing::error!("unable to get var {key}: {err}");
        err.into()
    })
}

/// Creates new [Client] and returns the client's keys
pub async fn client_keygen() -> Result<String, AppError> {
    let state = AppState::new().await?;
    let client_id = Client::create(&state).await?;

    let config = state.config();
    let key = config.secret_key();
    let nonce_key = config.nonce();

    let nonce = XNonce::from_slice(nonce_key);
    let cipher = XChaCha20Poly1305::new(key.into());

    let mut payload = [0u8; 16];
    OsRng.fill_bytes(&mut payload);

    let encrypt = cipher.encrypt(&nonce, client_id.as_bytes())?;
    let encode = hex::encode(&encrypt);

    Ok(encode)
}

/// Verifies the provided [ClientAccessKeys] and authenticates the client.
pub async fn verify_client(state: &AppState, payload: &str) -> Result<bool, AppError> {
    let decode =
        hex::decode(payload).map_err(|err| AppError::AuthorizationError(err.to_string()))?;

    let config = state.config();
    let key = config.secret_key();
    let nonce_key = config.nonce();

    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::from_slice(nonce_key);

    let decrypt = cipher.decrypt(nonce, decode.as_slice())?;
    let id =
        String::from_utf8(decrypt).map_err(|err| AppError::AuthorizationError(err.to_string()))?;
    let ok = Client::get(state, &id).await.is_ok();
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use crate::{errors::AppError, main_test::pretest, utils::client_keygen};

    #[tokio::test]
    async fn test_keygen() -> Result<(), AppError> {
        pretest().await?;
        let keygen = client_keygen().await;

        match &keygen {
            Ok(payload) => println!("id generated: {payload}"),
            Err(err) => println!("keygen failed: {err}"),
        }

        assert!(keygen.is_ok());
        Ok(())
    }
}
