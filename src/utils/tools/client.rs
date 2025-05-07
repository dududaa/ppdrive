use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305, XNonce};
use uuid::Uuid;

use crate::{errors::AppError, models::client::Client, state::AppState};

pub async fn create_client(state: &AppState, name: &str) -> Result<String, AppError> {
    let client_id = Uuid::new_v4();
    let client_id = client_id.to_string();

    let config = state.config();
    let key = config.secret_key();
    let nonce_key = config.nonce();

    let nonce = XNonce::from_slice(nonce_key);
    let cipher = XChaCha20Poly1305::new(key.into());

    let encrypt = cipher.encrypt(nonce, client_id.as_bytes())?;
    let encode = hex::encode(&encrypt);

    Client::create(state, &client_id, name).await?;

    Ok(encode)
}

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
    use super::{create_client, verify_client};
    use crate::{errors::AppError, main_test::pretest};

    #[tokio::test]
    async fn test_keygen() -> Result<(), AppError> {
        let state = pretest().await?;
        let name = "Demo Client";

        let keygen = create_client(&state, name).await;
        assert!(keygen.is_ok());

        let verified = verify_client(&state, &keygen?).await?;
        assert!(verified);

        Ok(())
    }
}
