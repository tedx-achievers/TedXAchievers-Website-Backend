use chrono::Utc;
use mongodb::{
    bson::doc,
    options::{FindOneAndUpdateOptions, FindOneOptions, FindOptions, ReturnDocument},
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
const DEPARTMENT_COUNTERS: &str = "volunteer_department_counters";
const DEPARTMENT_CAP: u64 = 10;

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
    let reference_code = format!("TEDX-{}", &id.replace('-', "")[..8].to_uppercase());
    VolunteerApplication {
        id,
        reference_code,
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
    let collection = db.collection::<VolunteerApplication>(COLLECTION);
    let now = Utc::now();
    let application = application_from_dto(
        dto,
        ApplicationStatus::Pending,
        now,
        Uuid::new_v4().to_string(),
    );
    let department = application.department.clone();
    if !claim_department_slot(db, &department).await? {
        return Err(AppError::Conflict(
            "This volunteer department has reached its application limit".to_owned(),
        ));
    }
    if let Err(error) = collection.insert_one(&application, None).await {
        release_department_slot(db, &department).await;
        if error.to_string().contains("E11000") {
            return Err(AppError::Conflict(
                "An application already exists for this email".to_owned(),
            ));
        }
        error!(%error, "Failed to insert volunteer application");
        return Err(AppError::Internal(anyhow::anyhow!(error)));
    }
    info!(application_id = %application.id, "Volunteer application created");
    Ok(application)
}

async fn claim_department_slot(db: &Database, department: &str) -> Result<bool, AppError> {
    let applications = db.collection::<VolunteerApplication>(COLLECTION);
    let existing_count = applications
        .count_documents(doc! { "department": department }, None)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    if existing_count >= DEPARTMENT_CAP {
        return Ok(false);
    }
    let counters = db.collection::<mongodb::bson::Document>(DEPARTMENT_COUNTERS);
    let options = FindOneAndUpdateOptions::builder()
        .upsert(true)
        .return_document(ReturnDocument::After)
        .build();
    let result = counters
        .find_one_and_update(
            doc! { "_id": department, "count": { "$lt": DEPARTMENT_CAP as i64 } },
            doc! { "$inc": { "count": 1_i64 } },
            options,
        )
        .await;
    match result {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(error) if error.to_string().contains("E11000") => {
            let retry_options = FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();
            counters
                .find_one_and_update(
                    doc! { "_id": department, "count": { "$lt": DEPARTMENT_CAP as i64 } },
                    doc! { "$inc": { "count": 1_i64 } },
                    retry_options,
                )
                .await
                .map(|result| result.is_some())
                .map_err(|retry_error| AppError::Internal(anyhow::anyhow!(retry_error)))
        }
        Err(error) => {
            error!(%error, department, "Failed to claim volunteer department slot");
            Err(AppError::Internal(anyhow::anyhow!(error)))
        }
    }
}

async fn release_department_slot(db: &Database, department: &str) {
    if let Err(error) = db
        .collection::<mongodb::bson::Document>(DEPARTMENT_COUNTERS)
        .update_one(
            doc! { "_id": department, "count": { "$gt": 0 } },
            doc! { "$inc": { "count": -1_i64 } },
            None,
        )
        .await
    {
        error!(%error, "Failed to release volunteer department slot");
    }
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
