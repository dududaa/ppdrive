use anyhow::anyhow;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use shared::secrets::AppSecrets;
use crate::payloads::UploadUrlConfig;

pub fn create_jwt(secrets: &AppSecrets, claims: &Claims) -> anyhow::Result<String> {
    let header = Header::new(Algorithm::HS512);
    encode(&header, claims, &EncodingKey::from_secret(secrets.jwt_secret())).map_err(|e| anyhow!(e))
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
    pub sub: i32,
    pub exp: i32,
    pub data: ClaimsData
}

#[derive(Serialize, Deserialize)]
pub enum ClaimsData {
    Upload {
        id: String,
        config: UploadUrlConfig,
    }
}