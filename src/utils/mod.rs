use chacha20poly1305::{
    aead::{rand_core::RngCore, Aead, OsRng},
    AeadCore, KeyInit, XChaCha20Poly1305, XNonce,
};
use hex::decode;
use sqlx::AnyPool;

use crate::{
    errors::AppError,
    models::client::{Client, CreateClientOpts},
    state::create_db_pool,
};

pub fn get_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|err| {
        tracing::error!("unable to get var {key}: {err}");
        err.into()
    })
}

/// Details of [ClientAccessKeys] will be used to authenticate and verify the client when accessing
/// administrative routes.
pub struct ClientAccessKeys {
    pub client_id: String,
    pub public: String,
    pub private: String,
}

/// Generates ne
pub async fn client_keygen() -> Result<ClientAccessKeys, AppError> {
    let key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let cipher = XChaCha20Poly1305::new(&key);

    let mut payload = [0u8; 16];
    OsRng.fill_bytes(&mut payload);

    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let enc = cipher.encrypt(&nonce, payload.as_slice())?;

    let ns = hex::encode(nonce);
    let nx = hex::encode(&enc);

    let copts = CreateClientOpts {
        key: key.to_vec(),
        payload: payload.to_vec(),
    };

    let conn = create_db_pool().await?;
    let client = Client::create(&conn, copts).await?;

    let keys = ClientAccessKeys {
        client_id: client.cid,
        public: ns,
        private: nx,
    };

    Ok(keys)
}

/// Verifies the provided [ClientAccessKeys] and authenticates the client.
pub async fn verify_client(conn: &AnyPool, keys: ClientAccessKeys) -> Result<bool, AppError> {
    let ClientAccessKeys {
        client_id: id,
        public,
        private,
    } = keys;

    let client = Client::get(conn, &id).await?;

    let enc_key = client.enc_key.as_slice();
    let cipher = XChaCha20Poly1305::new(enc_key.into());

    let nonce_data = decode(public).map_err(|err| AppError::ParsingError(err.to_string()))?;
    let enc_data = decode(private).map_err(|err| AppError::ParsingError(err.to_string()))?;

    let nonce = XNonce::from_slice(nonce_data.as_slice());
    let enc = enc_data.as_slice();
    let decrypt = cipher.decrypt(nonce, enc)?;

    Ok(client.enc_payload == decrypt)
}
