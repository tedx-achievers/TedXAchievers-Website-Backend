use super::handlers::placeholder;
use crate::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/initiate", post(placeholder))
        .route("/webhook", post(placeholder))
        .route("/mine", get(placeholder))
        .route("/:code/verify", get(placeholder))
        .route("/:code/checkin", patch(placeholder))
}
