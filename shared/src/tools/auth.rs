use crate::models::{Client, ClientInfo, ClientInsertArgs};
use crate::tools::AppSecrets;
use anyhow::{anyhow, Result};
use chacha20poly1305::{Error as XError, KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};
use sha3::{Digest, Sha3_256};
use sqlx_qb::DbPool;

/// generate a cipher token for client's id
fn client_token(secrets: &AppSecrets, client_key: &str) -> Result<String> {
    let key = secrets.secret_key();
    let nonce_key = secrets.nonce();

    let nonce = XNonce::try_from(nonce_key)?;
    let cipher = XChaCha20Poly1305::new(key.into());

    let encrypt = cipher.encrypt(&nonce, client_key.as_bytes())?;
    let encode = hex::encode(&encrypt);

    Ok(encode)
}

/// creates a new client and return the details
pub async fn create_client(
    db: &DbPool,
    secrets: &AppSecrets,
    name: &str,
    max_bucket_size: Option<f64>,
) -> anyhow::Result<ClientDetails> {
    let client_key = Client::generate_key();
    let encode = client_token(secrets, &client_key)?;

    let args = ClientInsertArgs {
        name,
        key: &client_key,
        max_bucket_size,
    };
    let id = Client::create(db, args).await?;
    Ok((id, encode).into())
}

/// decrypt client's cipher token and validate client token
pub async fn verify_client(db: &DbPool, secrets: &AppSecrets, token: &str) -> Result<ClientInfo> {
    let decode = hex::decode(token)?;

    let key = secrets.secret_key();
    let nonce_key = secrets.nonce();

    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::try_from(nonce_key)?;

    let decrypt = cipher.decrypt(&nonce, decode.as_slice())?;

    let key = String::from_utf8(decrypt)?;

    Client::get_info(db, &key).await
}

/// Regenerate token for a given client.
pub async fn regenerate_token(
    db: &DbPool,
    secrets: &AppSecrets,
    client_id: &str,
) -> Result<String> {
    let key = Client::update_key(db, client_id).await?;
    let token = client_token(secrets, &key)?;

    Ok(token)
}

pub async fn get_clients(db: &DbPool) -> Result<Vec<ClientInfo>> {
    Client::all(db).await
}

pub fn make_password(password: &str) -> String {
    let hash_pass = Sha3_256::digest(password.to_string().as_bytes());
    hex::encode(hash_pass)
}

pub fn check_password(password: &str, hashed: &str) -> Result<String> {
    let h = make_password(password);

    if *hashed != h {
        return Err(anyhow!("wrong password!"))
    }

    Ok(h)
}

pub struct ClientDetails {
    id: String,
    token: String,
}

#[cfg(test)]
mod tests {
    use std::env;
    use sqlx_qb::DbPool;
    use crate::create_pool;
    use crate::models::Client;
    use crate::tools::AppSecrets;
    use super::{create_client, client_token, verify_client};

    #[tokio::test]
    async fn test_token_validation() -> anyhow::Result<()> {
        let url = env::var("DATABASE_URL")?;
        let db = create_pool(&url).await? as DbPool;
        let secrets = AppSecrets::read().await?;

        let details = create_client(&db, &secrets, "Token Validation Test", None).await?;
        let client = Client::get_info(&db, &details.id).await?;

        let verify = verify_client(&db, &secrets, &details.token).await?;
        assert_eq!(client.id(), verify.id());

        let token = client_token(&secrets, client.key())?;
        let verify = verify_client(&db, &secrets, &token).await?;
        assert_eq!(client.id(), verify.id());

        Ok(())
    }
}
