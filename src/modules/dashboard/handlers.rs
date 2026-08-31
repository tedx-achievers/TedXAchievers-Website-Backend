use super::service;
use crate::{errors::AppError, middleware::role::RequireAttendee, AppState};
use axum::{extract::State, response::IntoResponse, Json};
use std::sync::Arc;

pub async fn dashboard_handler(
    State(state): State<Arc<AppState>>,
    RequireAttendee(user): RequireAttendee,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(
        service::get_dashboard(&state.db, &state.config, &user.id, &user.email).await?,
    ))
}

pub async fn profile_handler(
    State(state): State<Arc<AppState>>,
    RequireAttendee(user): RequireAttendee,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(service::get_profile(&state.db, &user.id).await?))
}

pub async fn update_profile_handler(
    State(state): State<Arc<AppState>>,
    RequireAttendee(user): RequireAttendee,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, AppError> {
    let name = body
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(|value| value.trim().to_owned());
    let phone = body
        .get("phone")
        .and_then(serde_json::Value::as_str)
        .map(|value| value.trim().to_owned());
    if name.as_ref().is_some_and(String::is_empty) || phone.as_ref().is_some_and(String::is_empty) {
        return Err(AppError::BadRequest(
            "Name and phone cannot be empty".to_owned(),
        ));
    }
    Ok(Json(
        service::update_profile(&state.db, &user.id, name, phone).await?,
    ))
}

pub async fn my_ticket_handler(
    State(state): State<Arc<AppState>>,
    RequireAttendee(user): RequireAttendee,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(service::get_ticket(&state.db, &user.id).await?))
}

pub async fn my_volunteer_handler(
    State(state): State<Arc<AppState>>,
    RequireAttendee(user): RequireAttendee,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(service::get_volunteer(&state.db, &user.email).await?))
}

pub async fn event_info_handler(
    State(state): State<Arc<AppState>>,
    RequireAttendee(_user): RequireAttendee,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(service::get_event(&state.config)))
}
