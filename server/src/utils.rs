use crate::routers::payloads::UploadUrlConfig;
use anyhow::anyhow;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use shared::secrets::AppSecrets;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn create_jwt(secrets: &AppSecrets, claims: &Claims) -> anyhow::Result<String> {
    let header = Header::new(Algorithm::HS512);
    encode(
        &header,
        claims,
        &EncodingKey::from_secret(secrets.jwt_secret()),
    )
    .map_err(|e| anyhow!(e))
}

pub(crate) fn decode_jwt(secrets: &AppSecrets, token: &str) -> anyhow::Result<Claims> {
    let mut validation = Validation::default();
    validation.algorithms = vec![Algorithm::HS512];

    let decoded = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secrets.jwt_secret()),
        &validation,
    )?;

    Ok(decoded.claims)
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    sub: i32,
    exp: i64,
    data: ClaimsData,
}

impl Claims {
    pub fn new(sub: i32, exp: i64, data: ClaimsData) -> anyhow::Result<Self> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let exp = now + exp;

        Ok(Self { sub, exp, data })
    }

    pub fn sub(&self) -> &i32 {
        &self.sub
    }

    pub fn exp(&self) -> &i64 {
        &self.exp
    }

    pub fn data(&self) -> &ClaimsData {
        &self.data
    }

    pub fn with_session_resume(mut self, value: bool) -> Self {
        let ClaimsData::Upload {
            session_id, config, ..
        } = self.data;

        self.data = ClaimsData::Upload {
            session_id,
            config,
            session_resume: value,
        };

        self
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ClaimsData {
    Upload {
        session_id: Option<String>,
        session_resume: bool,
        config: UploadUrlConfig,
    },
}
