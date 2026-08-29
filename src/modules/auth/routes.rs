use super::handlers::{
    forgot_password_handler, login_handler, logout_handler, refresh_handler, register_handler,
    reset_password_handler, verify_email_handler,
};
use crate::AppState;
use axum::{routing::post, Router};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .route("/refresh", post(refresh_handler))
        .route("/logout", post(logout_handler))
        .route("/verify-email", post(verify_email_handler))
        .route("/forgot-password", post(forgot_password_handler))
        .route("/reset-password", post(reset_password_handler))
}
