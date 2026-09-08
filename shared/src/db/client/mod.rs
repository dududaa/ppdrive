//! Client management — creation, authentication, and token encryption.
//!
//! Client keys are encrypted at rest using ChaCha20Poly1305 and looked up
//! via a SHA-256 hash to avoid decrypting every row.

use crate::db::Database;
use crate::tools::secrets::AppSecrets;
use anyhow::anyhow;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::aead::common::Generate;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use models::{Client, ClientInsertArgs};
use crate::sql_safe;

pub(crate) mod models;

/// Encrypt a plaintext key using ChaCha20Poly1305 with the app secret.
/// Returns (encrypted_key_hex, nonce_bytes).
fn encrypt_key(secrets: &AppSecrets, plaintext: &str) -> anyhow::Result<(String, Vec<u8>)> {
    let key = Key::try_from(secrets.secret_key())?;
    let cipher = XChaCha20Poly1305::new(&key);
    let nonce = XNonce::generate();
    let ciphertext = cipher.encrypt(&nonce, plaintext.as_bytes())?;
    Ok((hex::encode(&ciphertext), nonce.as_slice().to_vec()))
}

/// Decrypt an encrypted key using ChaCha20Poly1305 with the app secret.
pub fn decrypt_key(secrets: &AppSecrets, encrypted_hex: &str, nonce_bytes: &[u8]) -> anyhow::Result<String> {
    let key = Key::try_from(secrets.secret_key())?;
    let cipher = XChaCha20Poly1305::new(&key);
    let nonce = XNonce::try_from(nonce_bytes)?;
    let ciphertext = hex::decode(encrypted_hex)?;
    let plaintext = cipher.decrypt(&nonce, ciphertext.as_slice())?;
    Ok(String::from_utf8(plaintext)?)
}

/// Generate a SHA-256 hash of a key for database lookups.
fn hash_key(key: &str) -> String {
    use sha2::{Sha256, Digest};
    let hash = Sha256::digest(key.as_bytes());
    hex::encode(hash)
}

/// Create a new client, encrypting its key at rest. Returns [`ClientDetails`]
/// with the public PID and the plaintext token (shown once to the operator).
pub async fn create_client(
    db: &Database,
    secrets: &AppSecrets,
    name: &str,
) -> anyhow::Result<ClientDetails> {
    let client_key = Client::generate_nano();
    let pid = Client::generate_nano();

    let (encrypted_key, nonce) = encrypt_key(secrets, &client_key)?;
    let key_hash = hash_key(&client_key);

    let args = ClientInsertArgs {
        name: name.to_string(),
        pid,
        encrypted_key,
        key_nonce: nonce,
        key_hash,
    };

    let token = client_token(secrets, &client_key)?;
    let id = Client::create(db, args).await?;
    Ok((id, token).into())
}

/// generate a cipher token for client's id.
/// Uses a random nonce per encryption to prevent nonce reuse attacks.
fn client_token(secrets: &AppSecrets, client_key: &str) -> anyhow::Result<String> {
    let key = secrets.secret_key();
    let cipher_key = Key::try_from(key)?;
    let cipher = XChaCha20Poly1305::new(&cipher_key);

    let nonce = XNonce::generate();
    let encrypt = cipher.encrypt(&nonce, client_key.as_bytes())?;

    let mut output = Vec::with_capacity(nonce.len() + encrypt.len());
    output.extend_from_slice(nonce.as_slice());
    output.extend_from_slice(&encrypt);

    let encode = hex::encode(&output);
    Ok(encode)
}

/// Decrypt a client token, look up the client by key hash, and return its numeric ID.
pub async fn verify_client(
    db: &Database,
    secrets: &AppSecrets,
    token: &str,
) -> anyhow::Result<i32> {
    let decode = hex::decode(token)?;

    let key = secrets.secret_key();
    let cipher_key = Key::try_from(key)?;
    let cipher = XChaCha20Poly1305::new(&cipher_key);

    let nonce_len = XNonce::default().len();
    if decode.len() < nonce_len {
        return Err(anyhow!("token too short: missing nonce"));
    }
    let (nonce_bytes, ciphertext) = decode.split_at(nonce_len);
    let nonce = XNonce::try_from(nonce_bytes)?;

    let decrypt = cipher.decrypt(&nonce, ciphertext)?;
    let key = String::from_utf8(decrypt)?;

    // Look up client by hashing the extracted key
    let key_hash = hash_key(&key);
    Client::id_by_key_hash(db, &key_hash).await
}

/// Rotate a client's key, re-encrypt it, and return a new token.
pub async fn regenerate_token(
    db: &Database,
    secrets: &AppSecrets,
    client_id: &str,
) -> anyhow::Result<String> {
    let plaintext_key = Client::generate_nano();
    let (encrypted_key, nonce) = encrypt_key(secrets, &plaintext_key)?;
    let key_hash = hash_key(&plaintext_key);

    Client::update_key(db, client_id, &encrypted_key, &nonce, &key_hash).await?;

    let token = client_token(secrets, &plaintext_key)?;
    Ok(token)
}

/// Return all clients.
pub async fn get_clients(db: &Database) -> anyhow::Result<Vec<Client>> {
    Client::all(db).await
}

/// Resolve a public client PID to its numeric ID.
pub async fn get_id(pid: &str, db: &Database) -> anyhow::Result<i32> {
    let query = sql_safe!("SELECT id FROM clients WHERE pid = {} LIMIT 1", db.placeholder(1));
    let id = sqlx::query_scalar(query).bind(pid).fetch_one(&**db).await?;
    
    Ok(id)
}

/// Retrieve the client's PID and decrypted key for signing purposes.
pub async fn get_claims_data(db: &Database, id: &i32, secrets: &AppSecrets) -> anyhow::Result<(String, String)> {
    Client::get_claims_data(db, id, secrets).await
}

/// Retrieve the decrypted client key by PID.
pub async fn get_key(db: &Database, pid: &str, secrets: &AppSecrets) -> anyhow::Result<String> {
    Client::get_key_encrypted(db, pid, secrets).await
}

/// The public PID and plaintext token for a newly created client.
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

        AppSecrets::init().await?;
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
