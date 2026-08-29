use crate::{errors::AppError, models::user::UserRole, utils::jwt::verify_access_token, AppState};
use async_trait::async_trait;
use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::CookieJar;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub role: UserRole,
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get("access_token")
            .map(|cookie| cookie.value().to_owned())
            .ok_or(AppError::Unauthorized)?;
        let claims = verify_access_token(&token, &state.config)?;
        if !claims.is_verified {
            return Err(AppError::Unauthorized);
        }
        Ok(Self {
            id: claims.sub,
            email: claims.email,
            role: claims.role,
        })
    }
}
