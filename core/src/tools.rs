use crate::{HandlerResult, errors::HandlerError};
use chacha20poly1305::{Error as XError, KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};
use ppd_bk::{RBatis, models::client::Clients};
use ppd_shared::{
    opts::{ClientDetails, ClientInfo},
    tools::AppSecrets,
};
use sha3::{Digest, Sha3_256};

/// generate a token for client's id
fn generate_token(secrets: &AppSecrets, client_key: &str) -> HandlerResult<String> {
    let key = secrets.secret_key();
    let nonce_key = secrets.nonce();

    let nonce = XNonce::from_slice(nonce_key);
    let cipher = XChaCha20Poly1305::new(key.into());

    let encrypt = cipher.encrypt(nonce, client_key.as_bytes())?;
    let encode = hex::encode(&encrypt);

    Ok(encode)
}

/// creates a new client and returns the client's key
pub async fn create_client(
    rb: &RBatis,
    secrets: &AppSecrets,
    name: &str,
    max_bucket_size: Option<f64>,
) -> HandlerResult<ClientDetails> {
    let client_key = Clients::new_key();
    let token = generate_token(secrets, &client_key)?;

    let id = Clients::create(rb, client_key, name.to_string(), max_bucket_size).await?;
    Ok((id, token).into())
}

/// validate that a given client token exists
pub async fn verify_client(rb: &RBatis, secrets: &AppSecrets, token: &str) -> HandlerResult<(u64, Option<f64>)> {
    let decode =
        hex::decode(token).map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

    let key = secrets.secret_key();
    let nonce_key = secrets.nonce();

    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::from_slice(nonce_key);

    let decrypt = cipher.decrypt(nonce, decode.as_slice())?;

    let key = String::from_utf8(decrypt)
        .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

    let client = Clients::get_with_key(rb, &key).await?;
    Ok((client.id(), *client.max_bucket_size()))
}

/// Regenerate token for a given client.
pub async fn regenerate_token(
    db: &RBatis,
    secrets: &AppSecrets,
    client_id: &str,
) -> HandlerResult<String> {
    let mut client = Clients::get(db, client_id).await?;
    client.update_key(db).await?;

    let token = generate_token(secrets, client.key())?;
    Ok(token)
}

pub async fn get_clients(db: &RBatis) -> HandlerResult<Vec<ClientInfo>> {
    let clients = Clients::select_all(db)
        .await
        .map_err(|err| HandlerError::InternalError(err.to_string()))?;
    let results = clients.iter().map(|c| c.into()).collect();
    Ok(results)
}

pub fn make_password(password: &str) -> String {
    let hash_pass = Sha3_256::digest(password.to_string().as_bytes());
    hex::encode(hash_pass)
}

pub fn check_password(password: &str, hashed: &str) -> HandlerResult<String> {
    let h = make_password(password);

    if *hashed != h {
        return Err(HandlerError::AuthorizationError(
            "wrong password!".to_string(),
        ));
    }

    Ok(h)
}

impl From<XError> for HandlerError {
    fn from(value: XError) -> Self {
        HandlerError::InternalError(value.to_string())
    }
}
