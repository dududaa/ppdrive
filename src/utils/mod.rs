use chacha20poly1305::{
    aead::{rand_core::RngCore, Aead, OsRng},
    AeadCore, KeyInit, XChaCha20Poly1305,
};
use uuid::Uuid;

use crate::{errors::AppError, models::client::{Client, CreateClientOpts}, state::create_db_pool};

pub fn get_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|err| {
        tracing::error!("unable to get var {key}: {err}");
        err.into()
    })
}

pub struct ClientKeys {
    pub id: Uuid,
    pub public: String,
    pub private: String
}

pub async fn keygen() -> Result<ClientKeys, AppError> {
    let key = XChaCha20Poly1305::generate_key(&mut OsRng);
    let cipher = XChaCha20Poly1305::new(&key);
    
    let mut payload = [0u8; 16];
    OsRng.fill_bytes(&mut payload);
    
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let encryption = cipher.encrypt(&nonce, payload.as_slice())?;

    // let nv = nonce.to_vec();
    let ns = hex::encode(&nonce);
    let nx = hex::encode(&encryption);

    let copts = CreateClientOpts {
        key: key.to_vec(),
        payload: payload.to_vec()
    };

    let pool = create_db_pool().await?;
    let mut conn = pool.get().await?;
    let client = Client::create(&mut conn, copts).await?;

    let keys = ClientKeys {
        id: client.client_id,
        public: ns,
        private: nx
    };

    Ok(keys)
}
