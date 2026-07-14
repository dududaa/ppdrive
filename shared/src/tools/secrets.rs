use std::io::SeekFrom;
use std::path::PathBuf;
use chacha20poly1305::{Key, XNonce};
use anyhow::anyhow;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use chacha20poly1305::aead::common::Generate;

pub const SECRETS_FILENAME: &str = ".ppdrive_secret";

#[derive(Clone)]
pub struct AppSecrets {
    secret_key: Vec<u8>,
    secret_nonce: Vec<u8>,
    jwt_secret: Vec<u8>,
}

impl AppSecrets {
    /// Read app secrets from secret file.
    pub async fn read() -> anyhow::Result<Self> {
        init_secrets().await?;
        let secret_file = secret_filename()?;
        let mut secrets = tokio::fs::File::open(&secret_file).await?;

        let mut secret_key = [0; 32];
        let mut nonce = [0; 24];
        let mut jwt_secret = [0; 32];

        secrets.read_exact(&mut secret_key).await?;

        secrets.seek(SeekFrom::Start(32)).await?;
        secrets.read_exact(&mut nonce).await?;

        secrets.seek(SeekFrom::Start(32 + 24)).await?;
        secrets.read_exact(&mut jwt_secret).await?;

        Ok(Self {
            secret_key: Vec::from(secret_key),
            secret_nonce: Vec::from(nonce),
            jwt_secret: Vec::from(jwt_secret),
        })
    }

    pub fn secret_key(&self) -> &[u8] {
        self.secret_key.as_slice()
    }

    pub fn nonce(&self) -> &[u8] {
        self.secret_nonce.as_slice()
    }

    pub fn jwt_secret(&self) -> &[u8] {
        self.jwt_secret.as_slice()
    }
}

fn secret_filename() -> anyhow::Result<PathBuf> {
    let path = crate::root_dir()?.join(SECRETS_FILENAME);
    Ok(path)
}

async fn generate_secret_file() -> anyhow::Result<()> {
    let secret_key = Key::generate();
    let nonce = XNonce::generate();
    let jwt_secret = Key::generate();

    let secret_file = secret_filename()?;
    let mut secret_file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&secret_file)
        .await?;

    secret_file.write_all(secret_key.as_slice()).await?;
    secret_file.write_all(nonce.as_slice()).await?;
    secret_file.write_all(jwt_secret.as_slice()).await?;

    Ok(())
}

/// If app secret file does not exist, generate it. Mostly useful
/// for app initialization.
async fn init_secrets() -> anyhow::Result<()> {
    let path = secret_filename()?;
    if !path.is_file() {
        generate_secret_file().await.map_err(|err| anyhow!(err))?;
    }

    Ok(())
}