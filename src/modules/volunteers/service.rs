use chrono::Utc;
use mongodb::{
    bson::doc,
    options::{FindOneOptions, FindOptions},
    Database,
};
use tracing::{error, info};
use uuid::Uuid;

use super::dto::{ApplyVolunteerDto, UpdateApplicationStatusDto};
use crate::{
    errors::AppError,
    models::volunteer_application::{ApplicationStatus, VolunteerApplication},
};

const COLLECTION: &str = "volunteer_applications";

fn status_value(status: &ApplicationStatus) -> &'static str {
    match status {
        ApplicationStatus::Pending => "pending",
        ApplicationStatus::Approved => "approved",
        ApplicationStatus::Rejected => "rejected",
    }
}

fn application_from_dto(
    dto: ApplyVolunteerDto,
    status: ApplicationStatus,
    now: chrono::DateTime<Utc>,
    id: String,
) -> VolunteerApplication {
    VolunteerApplication {
        id,
        full_name: dto.full_name.trim().to_owned(),
        email: dto.email.trim().to_lowercase(),
        phone_number: dto.phone_number.trim().to_owned(),
        department: dto.department.trim().to_owned(),
        matric_number: dto.matric_number.trim().to_owned(),
        preferred_role: dto.preferred_role,
        motivation: dto.motivation.trim().to_owned(),
        status,
        created_at: now,
        updated_at: now,
    }
}

pub async fn apply(
    db: &Database,
    dto: ApplyVolunteerDto,
) -> Result<VolunteerApplication, AppError> {
    let email = dto.email.trim().to_lowercase();
    let collection = db.collection::<VolunteerApplication>(COLLECTION);
    if collection
        .find_one(
            doc! { "email": &email, "status": { "$in": ["pending", "approved"] } },
            None,
        )
        .await
        .map_err(|error| {
            error!(%error, "Failed to check active volunteer application");
            AppError::Internal(anyhow::anyhow!(error))
        })?
        .is_some()
    {
        return Err(AppError::Conflict(
            "An active application already exists for this email".to_owned(),
        ));
    }
    let now = Utc::now();
    if let Some(existing) = collection
        .find_one(
            doc! { "email": &email, "status": "rejected" },
            Some(
                FindOneOptions::builder()
                    .sort(doc! { "created_at": -1 })
                    .build(),
            ),
        )
        .await
        .map_err(|error| {
            error!(%error, "Failed to find rejected volunteer application");
            AppError::Internal(anyhow::anyhow!(error))
        })?
    {
        let mut replacement =
            application_from_dto(dto, ApplicationStatus::Pending, now, existing.id);
        replacement.created_at = existing.created_at;
        collection
            .replace_one(doc! { "_id": &replacement.id }, &replacement, None)
            .await
            .map_err(|error| {
                error!(%error, "Failed to update rejected volunteer application");
                AppError::Internal(anyhow::anyhow!(error))
            })?;
        info!(application_id = %replacement.id, "Volunteer application resubmitted");
        return Ok(replacement);
    }
    let application = application_from_dto(
        dto,
        ApplicationStatus::Pending,
        now,
        Uuid::new_v4().to_string(),
    );
    collection
        .insert_one(&application, None)
        .await
        .map_err(|error| {
            error!(%error, "Failed to insert volunteer application");
            AppError::Internal(anyhow::anyhow!(error))
        })?;
    info!(application_id = %application.id, "Volunteer application created");
    Ok(application)
}

pub async fn get_my_status(db: &Database, email: &str) -> Result<VolunteerApplication, AppError> {
    db.collection::<VolunteerApplication>(COLLECTION)
        .find_one(
            doc! { "email": email.trim().to_lowercase() },
            Some(
                FindOneOptions::builder()
                    .sort(doc! { "created_at": -1 })
                    .build(),
            ),
        )
        .await
        .map_err(|error| {
            error!(%error, "Failed to find volunteer application status");
            AppError::Internal(anyhow::anyhow!(error))
        })?
        .ok_or_else(|| AppError::NotFound("No application found for this email".to_owned()))
}

pub async fn list_applications(
    db: &Database,
    status_filter: Option<ApplicationStatus>,
) -> Result<Vec<VolunteerApplication>, AppError> {
    let filter = status_filter
        .as_ref()
        .map(|status| doc! { "status": status_value(status) })
        .unwrap_or_default();
    let mut cursor = db
        .collection::<VolunteerApplication>(COLLECTION)
        .find(
            filter,
            FindOptions::builder()
                .sort(doc! { "created_at": 1 })
                .build(),
        )
        .await
        .map_err(|error| {
            error!(%error, "Failed to list volunteer applications");
            AppError::Internal(anyhow::anyhow!(error))
        })?;
    let mut applications = Vec::new();
    while cursor
        .advance()
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
    {
        applications.push(
            cursor
                .deserialize_current()
                .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?,
        );
    }
    Ok(applications)
}

pub async fn update_status(
    db: &Database,
    application_id: &str,
    dto: UpdateApplicationStatusDto,
) -> Result<VolunteerApplication, AppError> {
    let collection = db.collection::<VolunteerApplication>(COLLECTION);
    let updated_at = Utc::now();
    let result = collection.update_one(doc! { "_id": application_id }, doc! { "$set": { "status": status_value(&dto.status), "updated_at": mongodb::bson::DateTime::from_millis(updated_at.timestamp_millis()) } }, None).await.map_err(|error| { error!(%error, "Failed to update volunteer application status"); AppError::Internal(anyhow::anyhow!(error)) })?;
    if result.matched_count == 0 {
        return Err(AppError::NotFound("Application not found".to_owned()));
    }
    collection
        .find_one(doc! { "_id": application_id }, None)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
        .ok_or_else(|| AppError::NotFound("Application not found".to_owned()))
}
