use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305, XNonce, Error as XError};
use ppd_bk::{models::client::Clients, RBatis};
use ppd_shared::{opts::ClientDetails, tools::AppSecrets};
use sha3::{Digest, Sha3_256};
use uuid::Uuid;
use crate::{errors::HandlerError, HandlerResult};

/// creates a new client and returns the client's key
pub async fn create_client(rb: &RBatis, secrets: &AppSecrets, name: &str) -> HandlerResult<ClientDetails> {
    let client_key = Uuid::new_v4().to_string();
    let token = generate_token(secrets, &client_key)?;
    
    Clients::create(rb, client_key.clone(), name.to_string()).await?;
    Ok((client_key, token).into())
}

/// generate a token for client's id
fn generate_token(secrets: &AppSecrets, client_id: &str) -> HandlerResult<String> {
    let key = secrets.secret_key();
    let nonce_key = secrets.nonce();

    let nonce = XNonce::from_slice(nonce_key);
    let cipher = XChaCha20Poly1305::new(key.into());

    let encrypt = cipher.encrypt(nonce, client_id.as_bytes())?;
    let encode = hex::encode(&encrypt);

    Ok(encode)
}

/// validate that a given client token exists
pub async fn verify_client(rb: &RBatis, secrets: &AppSecrets, token: &str) -> HandlerResult<u64> {
    let decode =
        hex::decode(token).map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

    let key = secrets.secret_key();
    let nonce_key = secrets.nonce();

    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::from_slice(nonce_key);

    let decrypt = cipher.decrypt(nonce, decode.as_slice())?;

    let id =
        String::from_utf8(decrypt)
        .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

    let client = Clients::get(rb, &id).await?;
    Ok(client.id())
}

/// Regenerate token for a given client.
pub async fn regenerate_token(
    db: &RBatis,
    secrets: &AppSecrets,
    current_key: &str,
) -> HandlerResult<ClientDetails> {
    let mut client = Clients::get(db, current_key).await?;
    let new_key = Uuid::new_v4().to_string();

    client.update_key(db, new_key.clone()).await?;
    let token = generate_token(secrets, client.key())?;

    Ok((new_key, token).into())
}

impl From<XError> for HandlerError {
    fn from(value: XError) -> Self {
        HandlerError::InternalError(value.to_string())
    }
}

pub fn make_password(password: &str) -> String {
    let hash_pass = Sha3_256::digest(password.to_string().as_bytes());
    hex::encode(hash_pass)
}

pub fn check_password(password: &str, hashed: &str) -> HandlerResult<String> {
    let h = make_password(password);

    if *hashed != h {
        return Err(HandlerError::AuthorizationError("wrong password!".to_string()));
    }

    Ok(h)
}