use super::handlers::{
    audit_logs_handler, dashboard_handler, export_attendees_handler, list_attendees_handler,
    list_volunteers_handler,
};
use crate::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/dashboard", get(dashboard_handler))
        .route("/attendees", get(list_attendees_handler))
        .route("/attendees/export", get(export_attendees_handler))
        .route("/volunteers", get(list_volunteers_handler))
        .route("/audit-logs", get(audit_logs_handler))
}
