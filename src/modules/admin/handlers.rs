use chrono::{DateTime, Utc};
use std::{collections::HashMap, sync::Arc};

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
    RequireAdmin(user): RequireAdmin,
) -> Result<impl IntoResponse, AppError> {
    let stats = service::get_dashboard_stats(&state.db).await?;
    let audit_db = state.db.clone();
    let audit_email = user.email;
    tokio::spawn(async move {
        let _ = crate::utils::audit::log_event(
            &audit_db,
            "admin.viewed_dashboard",
            Some(&audit_email),
            serde_json::json!({"adminEmail": audit_email}),
        )
        .await;
    });
    Ok((StatusCode::OK, Json(stats)))
}

pub async fn list_attendees_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_): RequireAdmin,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let (page, per_page) = pagination_params(&params)?;
    let attendees =
        service::list_attendees(&state.db, page, per_page, params.get("search").cloned()).await?;
    Ok((StatusCode::OK, Json(attendees)))
}

pub async fn export_attendees_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(user): RequireAdmin,
) -> Result<Response, AppError> {
    let csv = service::export_attendees_csv(&state.db).await?;
    let audit_db = state.db.clone();
    let audit_email = user.email;
    tokio::spawn(async move {
        let _ = crate::utils::audit::log_event(
            &audit_db,
            "admin.exported_csv",
            Some(&audit_email),
            serde_json::json!({"adminEmail": audit_email}),
        )
        .await;
    });
    let mut response = Response::new(csv.into_response().into_body());
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/csv"));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"attendees.csv\""),
    );
    Ok(response)
}

fn audit_date(value: Option<&String>, name: &str) -> Result<Option<DateTime<Utc>>, AppError> {
    value
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|date| date.with_timezone(&Utc))
                .map_err(|_| AppError::BadRequest(format!("{name} must be an ISO date")))
        })
        .transpose()
}

pub async fn audit_logs_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_): RequireAdmin,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let (page, per_page) = pagination_params(&params)?;
    let from = audit_date(params.get("from"), "from")?;
    let to = audit_date(params.get("to"), "to")?;
    let logs = service::get_audit_logs(
        &state.db,
        params.get("event_type").cloned(),
        from,
        to,
        page,
        per_page,
    )
    .await?;
    Ok((StatusCode::OK, Json(logs)))
}

pub async fn list_volunteers_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(_): RequireAdmin,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let (page, per_page) = pagination_params(&params)?;
    let volunteers =
        service::list_volunteers(&state.db, params.get("status").cloned(), page, per_page).await?;
    Ok((StatusCode::OK, Json(volunteers)))
}
