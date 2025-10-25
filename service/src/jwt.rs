use std::str::from_utf8;

use axum::http::HeaderValue;
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::{HandlerResult, errors::HandlerError, prelude::opts::LoginTokens};

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
    let bearer = format!("{bearer} ");
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

pub struct LoginOpts<'a> {
    pub config: &'a ServiceConfig,
    pub jwt_secret: &'a [u8],
    pub access_exp: Option<i64>,
    pub refresh_exp: Option<i64>,
    pub user_id: &'a u64,
}

impl<'a> LoginOpts<'a> {
    pub fn tokens(self) -> HandlerResult<LoginTokens> {
        let LoginOpts {
            config,
            jwt_secret,
            access_exp,
            refresh_exp,
            user_id,
        } = self;
    
        let default_access = config.auth.access_exp;
        let default_refresh = config.auth.refresh_exp;
    
        let access_exp = access_exp.unwrap_or(default_access);
        let refresh_exp = refresh_exp.unwrap_or(default_refresh);
    
        let access = if access_exp > 0 {
            let access_token = create_jwt(user_id, jwt_secret, access_exp, TokenType::Access)?;
    
            Some((access_token, access_exp))
        } else {
            None
        };
    
        let refresh = if refresh_exp > 0 {
            let refresh_token = create_jwt(user_id, jwt_secret, access_exp, TokenType::Refresh)?;
    
            Some((refresh_token, refresh_exp))
        } else {
            None
        };
    
        Ok(LoginTokens { access, refresh })
    }
}

