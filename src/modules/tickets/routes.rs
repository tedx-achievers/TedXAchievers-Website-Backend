use super::handlers::*;
use crate::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/initiate", post(initiate_ticket))
        .route("/verify-otp", post(verify_otp))
        .route("/webhook", post(webhook))
        .route("/mine", get(my_ticket))
        .route("/:code/verify", get(verify_ticket))
        .route("/:code/checkin", patch(checkin_ticket))
}
