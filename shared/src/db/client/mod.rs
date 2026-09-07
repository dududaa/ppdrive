use crate::db::Database;
use crate::tools::secrets::AppSecrets;
use anyhow::anyhow;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::aead::common::Generate;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use models::{Client, ClientInsertArgs};
use crate::sql_safe;

pub(crate) mod models;

/// generate a cipher token for client's id.
/// Uses a random nonce per encryption to prevent nonce reuse attacks.
fn client_token(secrets: &AppSecrets, client_key: &str) -> anyhow::Result<String> {
    let key = secrets.secret_key();
    let cipher_key = Key::try_from(key)?;
    let cipher = XChaCha20Poly1305::new(&cipher_key);

    // Generate a fresh random nonce for each encryption
    let nonce = XNonce::generate();
    let encrypt = cipher.encrypt(&nonce, client_key.as_bytes())?;

    // Prepend the nonce (24 bytes) to the ciphertext so decryption can recover it
    let mut output = Vec::with_capacity(nonce.len() + encrypt.len());
    output.extend_from_slice(nonce.as_slice());
    output.extend_from_slice(&encrypt);

    let encode = hex::encode(&output);
    Ok(encode)
}

/// creates a new client and return the details
pub async fn create_client(
    db: &Database,
    secrets: &AppSecrets,
    name: &str,
) -> anyhow::Result<ClientDetails> {
    let client_key = Client::generate_nano();
    let pid = Client::generate_nano();

    let args = ClientInsertArgs {
        name: name.to_string(),
        pid,
        key: client_key.clone(),
    };

    let encode = client_token(secrets, &client_key)?;
    let id = Client::create(db, args).await?;
    Ok((id, encode).into())
}

/// decrypt client's cipher token, validate client token and return client id
pub async fn verify_client(
    db: &Database,
    secrets: &AppSecrets,
    token: &str,
) -> anyhow::Result<i32> {
    let decode = hex::decode(token)?;

    let key = secrets.secret_key();
    let cipher_key = Key::try_from(key)?;
    let cipher = XChaCha20Poly1305::new(&cipher_key);

    // Extract the nonce (first 24 bytes) and ciphertext (remaining bytes)
    let nonce_len = XNonce::default().len();
    if decode.len() < nonce_len {
        return Err(anyhow!("token too short: missing nonce"));
    }
    let (nonce_bytes, ciphertext) = decode.split_at(nonce_len);
    let nonce = XNonce::try_from(nonce_bytes)?;

    let decrypt = cipher.decrypt(&nonce, ciphertext)?;
    let key = String::from_utf8(decrypt)?;

    Client::id_by_key(db, &key).await
}

/// Regenerate token for a given client.
pub async fn regenerate_token(
    db: &Database,
    secrets: &AppSecrets,
    client_id: &str,
) -> anyhow::Result<String> {
    let key = Client::update_key(db, client_id).await?;
    let token = client_token(secrets, &key)?;

    Ok(token)
}

pub async fn get_clients(db: &Database) -> anyhow::Result<Vec<Client>> {
    Client::all(db).await
}

pub async fn get_id(pid: &str, db: &Database) -> anyhow::Result<i32> {
    let query = sql_safe!("SELECT id FROM clients WHERE pid = {} LIMIT 1", db.placeholder(1));
    let id = sqlx::query_scalar(query).bind(pid).fetch_one(&**db).await?;
    
    Ok(id)
}

pub async fn get_claims_data(db: &Database, id: &i32) -> anyhow::Result<(String, String)> {
    Client::get_claims_data(db, id).await
}

pub async fn get_key(db: &Database, pid: &str) -> anyhow::Result<String> {
    Client::get_key(db, pid).await
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
    use crate::db::client::{create_client, verify_client};
    use crate::db::Database;
    use crate::tools::secrets::AppSecrets;
    use std::env;

    #[tokio::test]
    async fn test_token_validation() -> anyhow::Result<()> {
        dotenvy::dotenv()?;
        let url = env::var("DATABASE_URL")?;
        let db = Database::new(&url).await?;

        let secrets = AppSecrets::read().await?;
        let details = create_client(&db, &secrets, "Token Validation Test").await?;

        let id: i32 = sqlx::query_scalar("SELECT id FROM clients WHERE pid = $1 LIMIT 1")
            .bind(details.id)
            .fetch_one(&*db)
            .await?;

        let verify = verify_client(&db, &secrets, &details.token).await?;
        assert_eq!(id, verify);

        Ok(())
    }
}
