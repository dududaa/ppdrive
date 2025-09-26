use std::str::from_utf8;

use axum::http::HeaderValue;
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::errors::HandlerError;

use ppd_shared::opts::ServiceConfig;

pub const BEARER_KEY: &str = "PPDRIVE_BEARER_KEY";
pub const BEARER_VALUE: &str = "Bearer";

#[derive(Deserialize, Serialize)]
pub enum TokenType {
    Access,
    Refresh,
}

#[derive(Deserialize, Serialize)]
pub struct Claims {
    pub sub: u64,
    pub exp: i64,
    pub ty: TokenType,
}

pub(crate) fn decode_jwt(
    header_value: &HeaderValue,
    secret: &[u8],
    config: &ServiceConfig,
) -> Result<Claims, HandlerError> {
    let token = extract_jwt(header_value, &config.auth.bearer)?;

    let mut validation = Validation::default();
    validation.algorithms = vec![Algorithm::HS512];

    let decoded = decode::<Claims>(&token, &DecodingKey::from_secret(secret), &validation)
        .map_err(|err| HandlerError::AuthorizationError(format!("invalid token: {err}")))?;

    Ok(decoded.claims)
}

fn extract_jwt(header_value: &HeaderValue, bearer: &str) -> Result<String, HandlerError> {
    let bearer = format!("{} ", bearer);
    let bearer = bearer.as_str();

    if let Ok(v) = from_utf8(header_value.as_bytes()) {
        if v.starts_with(bearer) {
            let ext = v.trim_start_matches(bearer);
            return Ok(ext.to_owned());
        }
    }

    Err(HandlerError::AuthorizationError(
        "Error extracting jwt".to_string(),
    ))
}

pub fn create_jwt(
    user_id: &u64,
    secret: &[u8],
    exp: i64,
    ty: TokenType,
) -> Result<String, HandlerError> {
    let exp = Utc::now()
        .checked_add_signed(chrono::Duration::seconds(exp))
        .expect("Invalid timestamp")
        .timestamp();

    let claims = Claims {
        sub: user_id.to_owned(),
        exp,
        ty,
    };

    let header = Header::new(Algorithm::HS512);
    encode(&header, &claims, &EncodingKey::from_secret(secret))
        .map_err(|err| HandlerError::AuthorizationError(format!("unable to create token: {err}")))
}
