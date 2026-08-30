use std::collections::HashMap;
use std::sync::Arc;

use crate::{errors::AppError, middleware::role::RequireAdmin, AppState};
use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use super::service;

fn pagination_params(params: &HashMap<String, String>) -> Result<(u64, u64), AppError> {
    let page = params
        .get("page")
        .map(|value| value.parse::<u64>())
        .transpose()
        .map_err(|_| AppError::BadRequest("page must be a positive integer".to_owned()))?
        .filter(|value| *value > 0)
        .unwrap_or(1);
    let per_page = params
        .get("per_page")
        .map(|value| value.parse::<u64>())
        .transpose()
        .map_err(|_| AppError::BadRequest("per_page must be an integer".to_owned()))?
        .filter(|value| *value > 0)
        .unwrap_or(20)
        .min(100);
    Ok((page, per_page))
}

pub async fn dashboard_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_): RequireAdmin,
) -> Result<impl IntoResponse, AppError> {
    let stats = service::get_dashboard_stats(&state.db).await?;
    Ok((StatusCode::OK, Json(stats)))
}

pub async fn list_attendees_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_): RequireAdmin,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let (page, per_page) = pagination_params(&params)?;
    let attendees = service::list_attendees(
        &state.db,
        page,
        per_page,
        params.get("search").cloned(),
    )
    .await?;
    Ok((StatusCode::OK, Json(attendees)))
}

pub async fn export_attendees_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_): RequireAdmin,
) -> Result<Response, AppError> {
    let csv = service::export_attendees_csv(&state.db).await?;
    let mut response = Response::new(csv.into_response().into_body());
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"attendees.csv\""),
    );
    Ok(response)
}

pub async fn list_volunteers_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_): RequireAdmin,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let (page, per_page) = pagination_params(&params)?;
    let volunteers = service::list_volunteers(
        &state.db,
        params.get("status").cloned(),
        page,
        per_page,
    )
    .await?;
    Ok((StatusCode::OK, Json(volunteers)))
}
