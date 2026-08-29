use crate::{config::Config, errors::AppError, models::user::UserRole};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: UserRole,
    pub is_verified: bool,
    pub exp: usize,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefreshClaims {
    pub sub: String,
    pub exp: usize,
}
pub fn sign_access_token(
    user_id: &str,
    email: &str,
    role: &UserRole,
    config: &Config,
) -> Result<String, AppError> {
    let claims = Claims {
        sub: user_id.to_owned(),
        email: email.to_owned(),
        role: role.clone(),
        is_verified: true,
        exp: (Utc::now().timestamp() as u64 + config.jwt_access_expires_secs) as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_access_secret.as_bytes()),
    )
    .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
pub fn sign_refresh_token(user_id: &str, config: &Config) -> Result<String, AppError> {
    let claims = RefreshClaims {
        sub: user_id.to_owned(),
        exp: (Utc::now().timestamp() as u64 + config.jwt_refresh_expires_secs) as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_refresh_secret.as_bytes()),
    )
    .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
pub fn verify_access_token(token: &str, config: &Config) -> Result<Claims, AppError> {
    decode_with_rotation(
        token,
        &config.jwt_access_secret,
        config.jwt_access_secret_previous.as_deref(),
    )
}
pub fn verify_refresh_token(token: &str, config: &Config) -> Result<RefreshClaims, AppError> {
    decode_with_rotation(
        token,
        &config.jwt_refresh_secret,
        config.jwt_refresh_secret_previous.as_deref(),
    )
}

fn decode_with_rotation<T: for<'de> Deserialize<'de>>(
    token: &str,
    current_secret: &str,
    previous_secret: Option<&str>,
) -> Result<T, AppError> {
    let validation = Validation::default();
    let current = decode::<T>(
        token,
        &DecodingKey::from_secret(current_secret.as_bytes()),
        &validation,
    );
    if let Ok(data) = current {
        return Ok(data.claims);
    }
    previous_secret
        .and_then(|secret| {
            decode::<T>(
                token,
                &DecodingKey::from_secret(secret.as_bytes()),
                &validation,
            )
            .ok()
        })
        .map(|data| data.claims)
        .ok_or(AppError::Unauthorized)
}
