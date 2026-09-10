//! Application secrets management.
//!
//! Generates, stores, and reads a 56-byte binary secrets file (`.ppdrive_secret`)
//! containing the ChaCha20 encryption key and nonce. On Unix the
//! file is created with mode `0600`. Secret memory is zeroed on drop.

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
}

impl Drop for AppSecrets {
    fn drop(&mut self) {
        // Zero out secret material on drop to minimize exposure in freed memory
        for byte in self.secret_key.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0); }
        }
        for byte in self.secret_nonce.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0); }
        }
    }
}

impl AppSecrets {
    /// Initialize secrets file, generating it if it does not exist.
    /// Call this once at application startup before `read()`.
    pub async fn init() -> anyhow::Result<()> {
        init_secrets().await
    }

    /// Read app secrets from the binary secrets file (56 bytes: 32 key + 24 nonce).
    pub async fn read() -> anyhow::Result<Self> {
        let secret_file = secret_filename()?;
        let mut secrets = tokio::fs::File::open(&secret_file).await?;

        let mut secret_key = [0; 32];
        let mut nonce = [0; 24];

        secrets.read_exact(&mut secret_key).await?;

        secrets.seek(SeekFrom::Start(32)).await?;
        secrets.read_exact(&mut nonce).await?;

        Ok(Self {
            secret_key: Vec::from(secret_key),
            secret_nonce: Vec::from(nonce),
        })
    }

    /// 32-byte ChaCha20-Poly1305 encryption key.
    pub fn secret_key(&self) -> &[u8] {
        self.secret_key.as_slice()
    }

    /// 24-byte XChaCha20 nonce used for key encryption.
    pub fn nonce(&self) -> &[u8] {
        self.secret_nonce.as_slice()
    }
}

fn secret_filename() -> anyhow::Result<PathBuf> {
    let path = crate::root_dir()?.join(SECRETS_FILENAME);
    Ok(path)
}

async fn generate_secret_file() -> anyhow::Result<()> {
    let secret_key = Key::generate();
    let nonce = XNonce::generate();

    let secret_file = secret_filename()?;
    let mut secret_file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&secret_file)
        .await?;

    secret_file.write_all(secret_key.as_slice()).await?;
    secret_file.write_all(nonce.as_slice()).await?;
    drop(secret_file);

    // Set restrictive permissions on Unix systems (owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = secret_filename()?;
        let perms = std::fs::Permissions::from_mode(0o600);
        tokio::fs::set_permissions(&path, perms).await?;
    }

    Ok(())
}

/// If app secret file does not exist, generate it. Mostly useful
/// for app initialization.
async fn init_secrets() -> anyhow::Result<()> {
    let path = secret_filename()?;
    let exists = tokio::fs::metadata(&path).await.map(|m| m.is_file()).unwrap_or(false);
    if !exists {
        generate_secret_file().await.map_err(|err| anyhow!(err))?;
    }

    // Ensure existing secrets file has restrictive permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        tokio::fs::set_permissions(&path, perms).await?;
    }

    Ok(())
}
