use super::{
    dto::{ApplyVolunteerDto, CheckStatusDto, UpdateApplicationStatusDto},
    service,
};
use crate::{errors::AppError, middleware::role::RequireAdmin, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::{collections::HashMap, sync::Arc};
use validator::Validate;

fn validation_response(error: validator::ValidationErrors) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({ "success": false, "message": error.to_string() })),
    )
        .into_response()
}

pub async fn apply_handler(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<ApplyVolunteerDto>,
) -> Result<impl IntoResponse, AppError> {
    body.full_name = body.full_name.trim().to_owned();
    body.email = body.email.trim().to_lowercase();
    body.phone_number = body.phone_number.trim().to_owned();
    body.department = body.department.trim().to_owned();
    body.matric_number = body.matric_number.trim().to_owned();
    body.motivation = body.motivation.trim().to_owned();
    if let Err(error) = body.validate() {
        return Ok(validation_response(error));
    }
    let application = service::apply(&state.db, body).await?;
    let preferred_role = match &application.preferred_role {
        crate::models::volunteer_application::PreferredRole::LogisticsAndVenue => {
            "Logistics and Venue"
        }
        crate::models::volunteer_application::PreferredRole::FinanceAndSponsorship => {
            "Finance and Sponsorship"
        }
        crate::models::volunteer_application::PreferredRole::Welfare => "Welfare",
        crate::models::volunteer_application::PreferredRole::ProtocolAndUshering => {
            "Protocol and Ushering"
        }
        crate::models::volunteer_application::PreferredRole::Technical => "Technical",
        crate::models::volunteer_application::PreferredRole::Media => "Media",
    };
    crate::utils::email::enqueue(
        &state.email_queue,
        crate::utils::email::EmailJob {
            to_email: application.email.clone(),
            to_name: application.full_name.clone(),
            subject: "Your TEDxAchievers volunteer application is in".to_owned(),
            html: crate::utils::email::volunteer_application_received_html(
                &application.full_name,
                &state.config.frontend_url,
                &application.reference_code,
                preferred_role,
                &application.department,
                &application.created_at.format("%B %d, %Y").to_string(),
            ),
        },
    );
    Ok((StatusCode::CREATED, Json(application)).into_response())
}

pub async fn my_status_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CheckStatusDto>,
) -> Result<impl IntoResponse, AppError> {
    let application = service::get_my_status(&state.db, &params.email).await?;
    Ok((StatusCode::OK, Json(application)).into_response())
}

pub async fn admin_list_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(..): RequireAdmin,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let status_filter = params
        .get("status")
        .map(|status| match status.to_lowercase().as_str() {
            "pending" => Ok(crate::models::volunteer_application::ApplicationStatus::Pending),
            "approved" => Ok(crate::models::volunteer_application::ApplicationStatus::Approved),
            "rejected" => Ok(crate::models::volunteer_application::ApplicationStatus::Rejected),
            _ => Err(AppError::BadRequest("Invalid status filter".to_owned())),
        })
        .transpose()?;
    let applications = service::list_applications(&state.db, status_filter).await?;
    Ok((StatusCode::OK, Json(applications)).into_response())
}

pub async fn admin_update_status_handler(
    State(state): State<Arc<AppState>>,
    RequireAdmin(..): RequireAdmin,
    Path(id): Path<String>,
    Json(body): Json<UpdateApplicationStatusDto>,
) -> Result<impl IntoResponse, AppError> {
    if let Err(error) = body.validate() {
        return Ok(validation_response(error));
    }
    if !matches!(
        body.status,
        crate::models::volunteer_application::ApplicationStatus::Approved
            | crate::models::volunteer_application::ApplicationStatus::Rejected
    ) {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "success": false, "message": "Status must be approved or rejected" }))).into_response());
    }
    let application = service::update_status(&state.db, &id, body).await?;
    Ok((StatusCode::OK, Json(application)).into_response())
}
