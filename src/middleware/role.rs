use crate::{errors::AppError, middleware::auth::AuthUser, AppState};
use async_trait::async_trait;
use axum::{extract::FromRequestParts, http::request::Parts};
use std::sync::Arc;
pub struct RequireAttendee(pub AuthUser);
#[allow(dead_code)]
pub struct RequireVolunteer(pub AuthUser);
#[allow(dead_code)]
pub struct RequireAdmin(pub AuthUser);
macro_rules! role_extractor {
    ($name:ident, $check:ident) => {
        #[async_trait]
        impl FromRequestParts<Arc<AppState>> for $name {
            type Rejection = AppError;
            async fn from_request_parts(
                parts: &mut Parts,
                state: &Arc<AppState>,
            ) -> Result<Self, Self::Rejection> {
                let user = AuthUser::from_request_parts(parts, state).await?;
                if !user.role.$check() {
                    return Err(AppError::Forbidden);
                }
                Ok(Self(user))
            }
        }
    };
}
role_extractor!(RequireAttendee, has_attendee_access);
role_extractor!(RequireVolunteer, has_volunteer_access);
role_extractor!(RequireAdmin, has_admin_access);
