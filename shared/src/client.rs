use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use chacha20poly1305::aead::Aead;
use crate::DbPool;
use crate::models::clients::{Client, ClientInsertArgs};
use crate::tools::secrets::AppSecrets;

/// generate a cipher token for client's id.
fn client_token(secrets: &AppSecrets, client_key: &str) -> anyhow::Result<String> {
    let key = secrets.secret_key();
    let nonce_key = secrets.nonce();

    let cipher_key = Key::try_from(key)?;
    let nonce = XNonce::try_from(nonce_key)?;
    let cipher = XChaCha20Poly1305::new(&cipher_key);

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
    let client_key = Client::generate_nano();
    let pid = Client::generate_nano();

    let args = ClientInsertArgs {
        name: name.to_string(),
        pid,
        key: client_key.clone(),
        max_bucket_size,
    };

    let encode = client_token(secrets, &client_key)?;
    let id = Client::create(db, args).await?;
    Ok((id, encode).into())
}

/// decrypt client's cipher token, validate client token and return client id
pub async fn verify_client(db: &DbPool, secrets: &AppSecrets, token: &str) -> anyhow::Result<i32> {
    let decode = hex::decode(token)?;

    let key = secrets.secret_key();
    let nonce_key = secrets.nonce();

    let cipher_key = Key::try_from(key)?;
    let cipher = XChaCha20Poly1305::new(&cipher_key);
    let nonce = XNonce::try_from(nonce_key)?;

    let decrypt = cipher.decrypt(&nonce, decode.as_slice())?;
    let key = String::from_utf8(decrypt)?;

    Client::id_by_key(db, &key).await
}

/// Regenerate token for a given client.
pub async fn regenerate_token(
    db: &DbPool,
    secrets: &AppSecrets,
    client_id: &str,
) -> anyhow::Result<String> {
    let key = Client::update_key(db, client_id).await?;
    let token = client_token(secrets, &key)?;

    Ok(token)
}

pub async fn get_clients(db: &DbPool) -> anyhow::Result<Vec<Client>> {
    Client::all(db).await
}

pub struct ClientDetails {
    id: String,
    token: String,
}

impl ClientDetails {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

impl From<(String, String)> for ClientDetails {
    fn from((id, token): (String, String)) -> Self {
        Self { id, token }
    }
}

#[cfg(test)]
mod tests {
    use crate::create_pool;
    use crate::tools::secrets::AppSecrets;
    use std::env;
    use sqlx_qb::prelude::*;
    use crate::client::{create_client, verify_client};

    #[tokio::test]
    async fn test_token_validation() -> anyhow::Result<()> {
        dotenvy::dotenv()?;
        let url = env::var("DATABASE_URL")?;
        let db = create_pool(&url).await?;

        let secrets = AppSecrets::read().await?;
        let details = create_client(&db, &secrets, "Token Validation Test", None).await?;

        let modifiers = Modifiers::new().with_filter(("pid", details.id)).with_limit(1);
        let mut qb = QB::new(&db)
            .with_table_name("clients")
            .with_modifiers(&modifiers);

        let id: i32 = qb.select_scalar("id").await?;
        let verify = verify_client(&db, &secrets, &details.token).await?;
        assert_eq!(id, verify);

        Ok(())
    }
}