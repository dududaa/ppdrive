//! Cryptographic hashing and signed-token verification.
//!
//! Supports two hasher backends: HMAC-SHA256 and Blake3 (keyed).
//! Provides [`Hasher::hash`] for signing payloads and [`Hasher::verify`] /
//! [`Hasher::verify_upload_info`] / [`Hasher::verify_download_info`] for
//! verifying and decoding signed tokens.

use crate::db::{Database, client};
use crate::hasher::errors::PayloadVerificationError;
use crate::server::{DownloadInfo, UploadInfo};
use crate::tools::secrets::AppSecrets;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Deserialize, Serialize, Debug)]
pub enum Hasher {
    HMAC256,
    Blake3,
}

impl Hasher {
    /// Serialize `message`, compute a keyed hash, and return a base64url-encoded token
    /// containing `[4-byte length][payload][hash]`.
    pub fn hash<T: Serialize>(&self, key: &str, message: &T) -> anyhow::Result<String> {
        use Hasher::*;

        let payload = serde_json::to_string(message)?;
        let mut hash = match self {
            HMAC256 => hmac256::hash(key, &payload)?,
            Blake3 => blake3::hash(key, &payload)?,
        };

        let payload = payload.as_bytes();
        let mut data = (payload.len() as u32).to_be_bytes().to_vec();

        data.extend_from_slice(payload);
        data.append(&mut hash);

        let signed = URL_SAFE.encode(data);
        Ok(signed)
    }

    /// Decode a signed token, verify its hash using the key obtained via [`Hashable::key`],
    /// check expiration, and return the deserialized payload.
    pub async fn verify<T: Serialize + DeserializeOwned + Hashable>(
        &self,
        signed: &str,
    ) -> Result<T, PayloadVerificationError> {
        let decode = URL_SAFE.decode(signed)?;
        let (payload_len, data) = decode
            .split_at_checked(4)
            .ok_or(anyhow!("unable to decode payload_len"))?;

        let payload_len = u32::from_be_bytes(
            payload_len
                .try_into()
                .map_err(|_| anyhow!("unable to decode payload length"))?,
        );

        let (payload, hash) = data
            .split_at_checked(payload_len as usize)
            .ok_or(PayloadVerificationError::Error("malformed token".into()))?;
        let result: T = serde_json::from_slice(payload)?;
        let key = result.key().await?;
        self.verify_payload(&key, payload, hash)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("{e}"))?
            .as_secs() as i64;

        if now >= result.expires() {
            return Err(PayloadVerificationError::Expired);
        }

        Ok(result)
    }

    /// Verify a signed UploadInfo payload, decrypting the key from the database.
    pub async fn verify_upload_info(
        &self,
        signed: &str,
        db: &Database,
        secrets: &AppSecrets,
    ) -> Result<UploadInfo, PayloadVerificationError> {
        let decode = URL_SAFE.decode(signed)?;
        let (payload_len, data) = decode
            .split_at_checked(4)
            .ok_or(anyhow!("unable to decode payload_len"))?;

        let payload_len = u32::from_be_bytes(
            payload_len
                .try_into()
                .map_err(|_| anyhow!("unable to decode payload length"))?,
        );

        let (payload, hash) = data
            .split_at_checked(payload_len as usize)
            .ok_or(PayloadVerificationError::Error("malformed token".into()))?;
        let mut result: UploadInfo = serde_json::from_slice(payload)?;

        // Decrypt the client key from the database
        let row = client::get_description_keys(db, &result.client_id).await?;
        let key = client::decrypt_key(secrets, &row.0, &row.1)
            .map_err(|e| anyhow!("failed to decrypt client key: {e}"))?;

        self.verify_payload(&key, payload, hash)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("{e}"))?
            .as_secs() as i64;

        // Set the decrypted key on the result for downstream use
        result.client_key = Some(key);

        if now >= result.expires() {
            return Err(PayloadVerificationError::Expired);
        }

        Ok(result)
    }

    /// Verify a signed DownloadInfo payload, decrypting the key from the database.
    pub async fn verify_download_info(
        &self,
        signed: &str,
        db: &Database,
        secrets: &AppSecrets,
    ) -> Result<DownloadInfo, PayloadVerificationError> {
        let decode = URL_SAFE.decode(signed)?;
        let (payload_len, data) = decode
            .split_at_checked(4)
            .ok_or(anyhow!("unable to decode payload_len"))?;

        let payload_len = u32::from_be_bytes(
            payload_len
                .try_into()
                .map_err(|_| anyhow!("unable to decode payload length"))?,
        );

        let (payload, hash) = data
            .split_at_checked(payload_len as usize)
            .ok_or(PayloadVerificationError::Error("malformed token".into()))?;
        let mut result: DownloadInfo = serde_json::from_slice(payload)?;

        // Decrypt the client key from the database
        let row = client::get_description_keys(db, &result.client_id).await?;
        let key = client::decrypt_key(secrets, &row.0, &row.1)
            .map_err(|e| anyhow!("failed to decrypt client key: {e}"))?;

        self.verify_payload(&key, payload, hash)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("{e}"))?
            .as_secs() as i64;

        result.client_key = Some(key);

        if now >= result.expires() {
            return Err(PayloadVerificationError::Expired);
        }

        Ok(result)
    }

    fn verify_payload(&self, key: &str, payload: &[u8], hash: &[u8]) -> anyhow::Result<()> {
        use Hasher::*;

        match self {
            HMAC256 => hmac256::verify(&key, payload, hash),
            Blake3 => blake3::verify(&key, payload, hash),
        }
    }
}

/// Trait for types that can be signed and verified with a keyed hash.
pub trait Hashable {
    /// Describe how to retrieve the key
    fn key(&self) -> impl Future<Output = anyhow::Result<String>>;

    /// Time (seconds) assigned for the hash to expire.
    fn expires(&self) -> i64;
}

mod hmac256 {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    pub fn hash(key: &str, payload: &str) -> anyhow::Result<Vec<u8>> {
        let mut mac = HmacSha256::new_from_slice(key.as_bytes())?;
        mac.update(payload.as_bytes());

        let result = mac.finalize();
        Ok(result.into_bytes().to_vec())
    }

    /// Parse and verify base64 encoded [UploadInfo] payload.
    pub fn verify(key: &str, payload: &[u8], hash: &[u8]) -> anyhow::Result<()> {
        let mut mac = HmacSha256::new_from_slice(key.as_bytes())?;

        mac.update(payload);
        mac.verify_slice(hash)?;
        Ok(())
    }
}

mod blake3 {
    use anyhow::anyhow;
    use blake3;

    pub fn hash(key: &str, payload: &str) -> anyhow::Result<Vec<u8>> {
        let hash = blake3::keyed_hash(
            key.as_bytes()
                .try_into()
                .map_err(|_| anyhow!("Key must be a 32-byte long string"))?,
            payload.as_bytes(),
        );

        let res = hash.as_bytes().to_vec();
        Ok(res)
    }

    pub fn verify(key: &str, payload: &[u8], hash_raw: &[u8]) -> anyhow::Result<()> {
        use subtle::ConstantTimeEq;

        let payload_str =
            std::str::from_utf8(payload).map_err(|e| anyhow!("invalid payload utf8: {e}"))?;

        let hash = hash(key, payload_str)?;
        if hash.ct_eq(hash_raw).into() {
            Ok(())
        } else {
            Err(anyhow!("Blake3: verification failed."))
        }
    }
}

/// Errors that can occur during signed-payload verification.
pub mod errors {
    use std::fmt::Display;

    /// The signed payload has expired.
    #[derive(Debug)]
    pub enum PayloadVerificationError {
        Expired,
        Error(String),
    }

    impl<T: Display> From<T> for PayloadVerificationError {
        fn from(value: T) -> Self {
            PayloadVerificationError::Error(value.to_string())
        }
    }
}
