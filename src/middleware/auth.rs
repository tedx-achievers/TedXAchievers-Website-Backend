use crate::{
    errors::AppError,
    models::user::{User, UserRole},
    utils::jwt::verify_access_token,
    AppState,
};
use async_trait::async_trait;
use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::CookieJar;
use mongodb::{bson::{doc, oid::ObjectId}, options::FindOneOptions};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub role: UserRole,
}

#[derive(Clone)]
pub struct CachedAuthUser {
    pub user: AuthUser,
    pub security_version: u64,
    pub cached_at: Instant,
}

const AUTH_CACHE_TTL: Duration = Duration::from_secs(60);

pub fn invalidate_user_cache(
    cache: &dashmap::DashMap<String, CachedAuthUser>,
    user_id: &ObjectId,
) {
    cache.remove(&user_id.to_hex());
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
        if let Some(cached) = state.cache.get(&claims.sub) {
            if cached.cached_at.elapsed() < AUTH_CACHE_TTL
                && cached.security_version == claims.security_version
            {
                return Ok(cached.user.clone());
            }
        }
        let user_id = ObjectId::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
        let user = state
            .db
            .collection::<User>("users")
            .find_one(doc! { "_id": user_id }, Some(FindOneOptions::default()))
            .await
            .map_err(|_| AppError::Unauthorized)?
            .ok_or(AppError::Unauthorized)?;
        if !user.is_verified {
            return Err(AppError::Unauthorized);
        }
        if user.security_version != claims.security_version {
            return Err(AppError::Unauthorized);
        }
        let auth_user = Self {
            id: user_id.to_hex(),
            email: user.email,
            role: user.role,
        };
        state.cache.insert(
            claims.sub,
            CachedAuthUser {
                user: auth_user.clone(),
                security_version: user.security_version,
                cached_at: Instant::now(),
            },
        );
        Ok(auth_user)
    }
}
