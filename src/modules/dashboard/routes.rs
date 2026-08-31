use super::handlers::{
    dashboard_handler, event_info_handler, my_ticket_handler, my_volunteer_handler,
    profile_handler, update_profile_handler,
};
use crate::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(dashboard_handler))
        .route(
            "/profile",
            get(profile_handler).patch(update_profile_handler),
        )
        .route("/ticket", get(my_ticket_handler))
        .route("/volunteer", get(my_volunteer_handler))
        .route("/event", get(event_info_handler))
}
