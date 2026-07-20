use crate::db::Database;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::hasher::errors::PayloadVerificationError;

#[derive(Clone, Deserialize, Serialize, Debug)]
pub enum Hasher {
    HMAC256,
    Blake3,
}

impl Hasher {
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

    pub async fn verify<T: Serialize + DeserializeOwned + Hashable>(
        &self,
        signed: &str,
        db: &Database,
    ) -> Result<T, PayloadVerificationError> {
        use Hasher::*;

        let decode = URL_SAFE.decode(signed)?;
        let (payload_len, data) = decode
            .split_at_checked(4)
            .ok_or(anyhow!("unable to decode payload_len"))?;

        let payload_len = u32::from_be_bytes(
            payload_len
                .try_into()
                .map_err(|_| anyhow!("unable to decode payload length"))?,
        );

        let (payload, hash) = data.split_at(payload_len as usize);
        let result: T = serde_json::from_slice(&payload)?;
        let key = result.key(db).await?;

        match self {
            HMAC256 => hmac256::verify(&key, payload, hash)?,
            Blake3 => {
                let payload = serde_json::to_string(&result)?;
                blake3::verify(&key, &payload, hash)?
            },
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("{e}"))?
            .as_secs() as i64;

        if now >= result.expires() {
            return Err(PayloadVerificationError::Expired);
        }

        Ok(result)
    }
}

pub trait Hashable {
    /// Describe how to retrieve the key
    fn key(&self, db: &Database) -> impl Future<Output = anyhow::Result<String>>;

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
                .map_err(|_| anyhow!("Key must be a 32-bit long string"))?,

            payload.as_bytes(),
        );

        let res = hash.as_bytes().to_vec();
        Ok(res)
    }

    pub fn verify(key: &str, payload: &str, hash_raw: &[u8]) -> anyhow::Result<()> {
        let hash = hash(key, payload)?;
        if &hash != hash_raw {
            return Err(anyhow!("Blake3: verification failed."));
        }

        Ok(())
    }
}

pub mod errors {
    use std::fmt::Display;

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
