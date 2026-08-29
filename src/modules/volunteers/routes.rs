use super::handlers::{
    admin_list_handler, admin_update_status_handler, apply_handler, my_status_handler,
};
use crate::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/apply", post(apply_handler))
        .route("/me", get(my_status_handler))
        .route("/admin/list", get(admin_list_handler))
        .route("/admin/:id", patch(admin_update_status_handler))
}
