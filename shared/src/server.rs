use crate::db::Database;
use crate::hasher::{Hashable, Hasher, errors::PayloadVerificationError};
use anyhow::anyhow;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use validator::Validate;

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
    /// This can be derived from [UploadUrlConfig]'s `expires` property and later used by broker
    /// to determine resumable chunk's url expiration.
    pub chunk_session_expiration: i64,
    pub config: Option<UploadUrlConfig>,
    /// The decrypted client signing key, populated when creating a session.
    /// Not serialized into the signed token for security.
    #[serde(skip)]
    pub client_key: Option<String>,
}

impl UploadInfo {
    pub fn sign(&self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        hasher.hash(key, self)
    }

    pub fn resign(&mut self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        self.chunk_index += 1;
        self.config = None;
        self.exp = seconds_from_now(self.exp)?;

        self.sign(key, hasher)
    }

    pub async fn verify(
        signed: &str,
        db: &Database,
        secrets: &crate::tools::secrets::AppSecrets,
        hasher: &Hasher,
    ) -> Result<UploadInfo, PayloadVerificationError> {
        hasher.verify_upload_info(signed, db, secrets).await
    }
}

impl Hashable for UploadInfo {
    fn key(&self, _db: &Database) -> impl Future<Output = anyhow::Result<String>> {
        async {
            // If client_key is already populated (e.g., from create_session), use it directly
            if let Some(key) = &self.client_key {
                return Ok(key.clone());
            }
            // Otherwise, look up and decrypt from database
            Err(anyhow!("client_key not available on deserialized UploadInfo; use verify_with_secrets"))
        }
    }

    fn expires(&self) -> i64 {
        self.exp
    }
}

pub fn make_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("argon2 hashing should not fail");
    hash.to_string()
}

pub fn check_password(password: &str, hashed: &str) -> anyhow::Result<()> {
    let parsed_hash = PasswordHash::new(hashed)
        .map_err(|e| anyhow!("invalid password hash format: {e}"))?;

    let argon2 = Argon2::default();
    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| anyhow!("wrong password!"))?;

    Ok(())
}

#[derive(Serialize, Deserialize, Validate, Default, Clone)]
pub struct UploadUrlConfig {
    pub method: UploadUrlMethod,
    pub asset_type: AssetType,
    #[validate(range(min = 30))]
    pub expires: i64,
    #[validate(length(min = 4))]
    pub path: String,
    pub target_filesize: Option<u64>,
    /// Create asset parent folders if they don't exist, else error will be returned.
    pub create_parents: Option<bool>,
    /// overwrite asset if it already exists.
    pub overwrite: Option<bool>,
    pub resumable: Option<bool>,
    /// The bucket to which the asset belongs
    pub bucket: Option<String>,
}

impl UploadUrlConfig {
    pub fn test() -> Self {
        UploadUrlConfig {
            method: UploadUrlMethod::Post,
            asset_type: AssetType::File,
            path: "test-assets/uploads/creator.jpg".to_string(),
            expires: 120,
            ..Default::default()
        }
    }
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

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::db::{client, Database};
    use crate::secrets::AppSecrets;
    use crate::server::{UploadInfo, UploadUrlConfig, seconds_from_now};
    use std::sync::Arc;
    use tokio::sync::{Mutex, OnceCell};
    use crate::hasher::Hasher;

    type SharedConfig = Arc<Mutex<AppConfig>>;
    static APP_CONFIG: OnceCell<SharedConfig> = OnceCell::const_new();

    async fn get_config() -> &'static SharedConfig {
        APP_CONFIG
            .get_or_init(|| async {
                let config = AppConfig::read().await.unwrap();
                Arc::new(Mutex::new(config))
            })
            .await
    }

    async fn run_sign_info_test(config: AppConfig) -> anyhow::Result<()> {
        let secrets = AppSecrets::read().await?;

        let db = Database::new(&config.database_url).await?;
        let hasher = config.hasher.clone();
        let client_details = client::create_client(&db, &secrets, "Signed Client").await?;

        let config = UploadUrlConfig::test();
        let plaintext_key = client::get_key(&db, client_details.id(), &secrets).await?;
        let info = UploadInfo {
            client_id: client_details.id().to_string(),
            exp: seconds_from_now(config.expires)?,
            config: Some(config),
            client_key: Some(plaintext_key),
            ..Default::default()
        };

        let key = info.client_key.clone().unwrap();
        let mut signed = info.sign(&key, &hasher)?;

        let mut verified = UploadInfo::verify(&signed, &db, &secrets, &hasher).await;
        assert!(verified.is_ok());

        // is tampered, this should fail
        signed.push_str("mod");
        verified = UploadInfo::verify(&signed, &db, &secrets, &hasher).await;
        assert!(verified.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_info_hmac256_signing() -> anyhow::Result<()> {
        let config = get_config().await.lock().await;
        run_sign_info_test(config.clone()).await?;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_upload_info_blake3_signing() -> anyhow::Result<()> {
        let mut config = get_config().await.lock().await;
        config.hasher = Hasher::Blake3;

        run_sign_info_test(config.clone()).await?;
        Ok(())
    }
}
