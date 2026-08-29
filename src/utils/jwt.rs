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
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_access_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}
pub fn verify_refresh_token(token: &str, config: &Config) -> Result<RefreshClaims, AppError> {
    decode::<RefreshClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_refresh_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}
