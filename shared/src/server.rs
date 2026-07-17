use crate::client::models::Client;
use crate::db::Database;
use crate::server::errors::PayloadVerificationError;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sha3::{Digest, Sha3_256};
use std::time::{SystemTime, UNIX_EPOCH};
use validator::Validate;

type HmacSha256 = Hmac<Sha256>;

pub fn hash_payload(key: &str, payload: &str) -> anyhow::Result<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())?;
    mac.update(payload.as_bytes());

    let result = mac.finalize();
    Ok(result.into_bytes().to_vec())
}

fn sign_payload(key: &str, payload: &str) -> anyhow::Result<String> {
    let mut data = payload.as_bytes().to_vec();
    data.push(b'.');

    let mut hash_payload = hash_payload(key, payload)?;
    data.append(&mut hash_payload);

    let signed = URL_SAFE.encode(data);
    Ok(signed)
}

/// Parse and verify base64 encoded [UploadInfo] payload.
async fn verify_payload(
    signed: &str,
    db: &Database,
) -> Result<UploadInfo, PayloadVerificationError> {
    let decode = URL_SAFE.decode(signed)?;
    let split = decode.split(|c| b'.' == *c).collect::<Vec<&[u8]>>();

    let payload = split.first().ok_or("missing payload")?;
    let hash = split.get(1).ok_or("missing hash")?;
    let info: UploadInfo = serde_json::from_slice(payload)?;

    let key = Client::get_key(db, &info.client_id).await?;
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())?;
    mac.update(payload);
    mac.verify_slice(hash)?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow!("{e}"))?
        .as_secs() as i64;

    if info.exp >= now {
        return Err(PayloadVerificationError::Expired);
    }

    Ok(info)
}

pub fn seconds_from_now(seconds: i64) -> anyhow::Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow!("{e}"))?
        .as_secs() as i64;

    let res = now + seconds;

    Ok(res)
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct UploadInfo {
    pub client_id: String,
    pub session_id: Option<String>,
    pub exp: i64,
    pub chunk_index: u16,
    // Time each chunk token must expire
    pub chunk_session_expiration: i64,
    pub config: Option<UploadUrlConfig>,
}

impl UploadInfo {
    pub fn sign(&self, key: &str) -> anyhow::Result<String> {
        let payload = serde_json::to_string(self)?;
        let token = sign_payload(key, &payload)?;

        Ok(token)
    }

    pub fn resign(&mut self, key: &str) -> anyhow::Result<String> {
        self.chunk_index += 1;
        self.config = None;
        self.exp = seconds_from_now(self.exp)?;

        self.sign(key)
    }

    pub async fn verify(
        signed: &str,
        db: &Database,
    ) -> Result<UploadInfo, PayloadVerificationError> {
        verify_payload(signed, db).await
    }
}

pub fn make_password(password: &str) -> String {
    let hash_pass = Sha3_256::digest(password.to_string().as_bytes());
    hex::encode(hash_pass)
}

pub fn check_password(password: &str, hashed: &str) -> anyhow::Result<String> {
    let h = make_password(password);

    if *hashed != h {
        return Err(anyhow!("wrong password!"));
    }

    Ok(h)
}

#[derive(Serialize, Deserialize, Validate, Default, Clone)]
pub struct UploadUrlConfig {
    pub method: UploadUrlMethod,
    pub asset_type: AssetType,
    #[validate(range(min = 30))]
    pub expires: i64,
    pub path: String,
    pub target_filesize: Option<u64>,
    /// Create asset parent folders if they don't exist, else error will be returned.
    pub create_parents: Option<bool>,
    /// overwrite asset if it already exists.
    pub overwrite: Option<bool>,
    pub resumable: Option<bool>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub enum UploadUrlMethod {
    #[default]
    Post,
    Put,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub enum AssetType {
    #[default]
    File,
    Folder,
}

pub mod errors {
    use std::fmt::Display;

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

#[cfg(test)]
mod tests {
    use crate::client;
    use crate::client::create_client;
    use crate::config::AppConfig;
    use crate::db::Database;
    use crate::secrets::AppSecrets;
    use crate::server::errors::PayloadVerificationError;
    use crate::server::{UploadInfo, verify_payload};
    use anyhow::anyhow;

    #[tokio::test]
    async fn test_upload_info_signing() -> anyhow::Result<()> {
        let config = AppConfig::read().await?;
        let secrets = AppSecrets::read().await?;
        let db = Database::new(&config.database_url).await?;
        let client_details = create_client(&db, &secrets, "Signed Client", None).await?;

        let info = UploadInfo {
            client_id: client_details.id().to_string(),
            ..Default::default()
        };

        let key = client::get_key(&db, client_details.id()).await?;
        let mut signed = info.sign(&key)?;

        let mut verified = verify_payload(&signed, &db).await;
        assert!(verified.is_ok());

        // is tampered, this should fail
        signed.push_str("mod");
        verified = verify_payload(&signed, &db).await;
        assert!(verified.is_err());

        Ok(())
    }
}
