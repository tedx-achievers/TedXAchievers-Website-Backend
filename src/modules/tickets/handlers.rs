use crate::errors::AppError;
use axum::{http::StatusCode, response::IntoResponse, Json};
pub async fn placeholder() -> Result<impl IntoResponse, AppError> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"success": true, "message": "module ok"})),
    ))
}
