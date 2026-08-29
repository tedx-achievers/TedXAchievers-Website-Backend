use axum::{response::IntoResponse, Json};
use chrono::Utc;

pub async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": Utc::now().to_rfc3339(),
    }))
}
