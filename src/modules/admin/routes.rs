use super::handlers::placeholder;
use crate::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/dashboard", get(placeholder))
        .route("/attendees", get(placeholder))
        .route("/attendees/export", get(placeholder))
        .route("/volunteers", get(placeholder))
}
