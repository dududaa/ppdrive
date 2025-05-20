use std::io::SeekFrom;

use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::{errors::AppError, utils::tools::secrets::SECRETS_FILENAME};

/// App secrets sharable across [AppState](crate::AppState).
pub struct AppSecrets {
    secret_key: Vec<u8>,
    secret_nonce: Vec<u8>,
    jwt_secret: Vec<u8>,
}

impl AppSecrets {
    /// Read app secrets from secret file
    pub async fn read() -> Result<Self, AppError> {
        let mut secrets = tokio::fs::File::open(SECRETS_FILENAME).await?;

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

#[cfg(test)]
mod test {
    use crate::{config::secrets::AppSecrets, errors::AppError};

    #[tokio::test]
    async fn test_config_build() -> Result<(), AppError> {
        let config = AppSecrets::read().await?;

        let secret_key = config.secret_key();
        let nonce = config.nonce();
        let jwt = config.jwt_secret();

        assert_eq!((secret_key.len(), nonce.len(), jwt.len()), (32, 24, 32));
        assert!(secret_key != jwt);

        Ok(())
    }
}
