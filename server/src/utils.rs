use anyhow::anyhow;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use shared::secrets::AppSecrets;

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
}