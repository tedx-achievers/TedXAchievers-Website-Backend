use chrono::{DateTime, Duration, Utc};
use mongodb::{
    bson::doc,
    options::{FindOneOptions, FindOptions},
    Database,
};
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

use super::dto::{ApplyVolunteerDto, ChangePreferredRoleDto, UpdateApplicationStatusDto};
use crate::{
    config::Config,
    errors::AppError,
    models::{
        user::{User, UserRole},
        volunteer_application::{ApplicationStatus, PreferredRole, VolunteerApplication},
    },
};
use std::sync::Arc;

const COLLECTION: &str = "volunteer_applications";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicVolunteerStatus {
    pub reference_code: String,
    pub status: String,
    pub preferred_role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn status_value(status: &ApplicationStatus) -> &'static str {
    match status {
        ApplicationStatus::Pending => "pending",
        ApplicationStatus::Approved => "approved",
        ApplicationStatus::Rejected => "rejected",
    }
}

fn preferred_role_value(role: &PreferredRole) -> &'static str {
    match role {
        PreferredRole::Technical => "technical",
        PreferredRole::Videography => "videography",
        PreferredRole::Photography => "photography",
        PreferredRole::Content => "content",
        PreferredRole::ProtocolAndUshering => "protocol_and_ushering",
        PreferredRole::Welfare => "welfare",
        PreferredRole::GraphicAndDesign => "graphic_and_design",
        PreferredRole::VenueAndDecoration => "venue_and_decoration",
        PreferredRole::PartnershipAndSponsorship => "partnership_and_sponsorship",
    }
}

fn application_from_dto(
    dto: ApplyVolunteerDto,
    status: ApplicationStatus,
    now: chrono::DateTime<Utc>,
    id: String,
) -> VolunteerApplication {
    let reference_code = format!("TEDxACH-{}", &id.replace('-', "")[..8].to_uppercase());
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
    if let Err(error) = collection.insert_one(&application, None).await {
        if error.to_string().contains("E11000") {
            return Err(AppError::Conflict(
                "An application already exists for this email".to_owned(),
            ));
        }
        error!(%error, "Failed to insert volunteer application");
        return Err(AppError::Internal(anyhow::anyhow!(error)));
    }
    info!(application_id = %application.id, "Volunteer application created");
    let audit_db = db.clone();
    let audit_email = application.email.clone();
    let audit_name = application.full_name.clone();
    let audit_role = format!("{:?}", application.preferred_role);
    tokio::spawn(async move {
        let _ = crate::utils::audit::log_event(
            &audit_db,
            "volunteer.applied",
            Some(&audit_email),
            json!({
                "fullName": audit_name,
                "email": audit_email,
                "preferredRole": audit_role
            }),
        )
        .await;
    });
    Ok(application)
}

pub async fn get_my_status(db: &Database, email: &str) -> Result<PublicVolunteerStatus, AppError> {
    let application = db
        .collection::<VolunteerApplication>(COLLECTION)
        .find_one(
            doc! { "email": email.trim().to_lowercase() },
            Some(
                FindOneOptions::builder()
                    .sort(doc! { "createdAt": -1 })
                    .build(),
            ),
        )
        .await
        .map_err(|error| {
            error!(%error, "Failed to find volunteer application status");
            AppError::Internal(anyhow::anyhow!(error))
        })?
        .ok_or_else(|| AppError::NotFound("No application found for this email".to_owned()))?;
    Ok(PublicVolunteerStatus {
        reference_code: application.reference_code,
        status: status_value(&application.status).to_owned(),
        preferred_role: preferred_role_value(&application.preferred_role).to_owned(),
        created_at: application.created_at,
        updated_at: application.updated_at,
    })
}

pub async fn change_preferred_role(
    db: &Database,
    dto: ChangePreferredRoleDto,
) -> Result<VolunteerApplication, AppError> {
    let collection = db.collection::<VolunteerApplication>(COLLECTION);
    let email = dto.email.trim().to_lowercase();
    let updated_at = Utc::now();
    let result = collection
        .update_one(
            doc! { "email": &email, "roleChangeCount": { "$ne": 1_i64 } },
            doc! {
                "$set": {
                    "preferredRole": preferred_role_value(&dto.preferred_role),
                    "updatedAt": mongodb::bson::DateTime::from_millis(updated_at.timestamp_millis())
                },
                "$inc": { "roleChangeCount": 1_i64 }
            },
            None,
        )
        .await
        .map_err(|error| {
            error!(%error, "Failed to update volunteer preferred role");
            AppError::Internal(anyhow::anyhow!(error))
        })?;
    if result.matched_count == 0 {
        let exists = collection
            .find_one(doc! { "email": &email }, None)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
            .is_some();
        return if exists {
            Err(AppError::Conflict(
                "The preferred role has already been changed for this email".to_owned(),
            ))
        } else {
            Err(AppError::NotFound(
                "No application found for this email".to_owned(),
            ))
        };
    }
    collection
        .find_one(doc! { "email": email }, None)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
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
            FindOptions::builder().sort(doc! { "createdAt": 1 }).build(),
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
    config: &Arc<Config>,
    application_id: &str,
    dto: UpdateApplicationStatusDto,
    admin_email: &str,
) -> Result<VolunteerApplication, AppError> {
    let collection = db.collection::<VolunteerApplication>(COLLECTION);
    let application = collection
        .find_one(doc! { "_id": application_id }, None)
        .await
        .map_err(|error| {
            error!(%error, "Failed to find volunteer application");
            AppError::Internal(anyhow::anyhow!(error))
        })?
        .ok_or_else(|| AppError::NotFound("Application not found".to_owned()))?;
    let previous_status = application.status.clone();
    let updated_at = Utc::now();
    collection
        .update_one(
            doc! { "_id": application_id },
            doc! { "$set": { "status": status_value(&dto.status), "updatedAt": mongodb::bson::DateTime::from_millis(updated_at.timestamp_millis()) } },
            None,
        )
        .await
        .map_err(|error| {
            error!(%error, "Failed to update volunteer application status");
            AppError::Internal(anyhow::anyhow!(error))
        })?;
    let updated = collection
        .find_one(doc! { "_id": application_id }, None)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
        .ok_or_else(|| AppError::NotFound("Application not found".to_owned()))?;

    let event_type = match &dto.status {
        ApplicationStatus::Approved => Some("volunteer.approved"),
        ApplicationStatus::Rejected => Some("volunteer.rejected"),
        ApplicationStatus::Pending => None,
    };
    if let Some(event_type) = event_type {
        let mut metadata = json!({
            "applicantEmail": updated.email.clone(),
            "adminEmail": admin_email
        });
        if event_type == "volunteer.rejected" {
            metadata["previousStatus"] = json!(format!("{:?}", previous_status));
        }
        crate::utils::audit::log_event(db, event_type, Some(admin_email), metadata).await?;
    }

    let preferred_role = match &updated.preferred_role {
        PreferredRole::Technical => "Technical",
        PreferredRole::Videography => "Videography",
        PreferredRole::Photography => "Photography",
        PreferredRole::Content => "Content",
        PreferredRole::ProtocolAndUshering => "Protocol and Ushering",
        PreferredRole::Welfare => "Welfare",
        PreferredRole::GraphicAndDesign => "Graphic and Design",
        PreferredRole::VenueAndDecoration => "Venue and Decoration",
        PreferredRole::PartnershipAndSponsorship => "Partnership and Sponsorship",
    };
    let users = db.collection::<User>("users");
    if dto.status == ApplicationStatus::Approved {
        if let Some(user) = users
            .find_one(doc! { "email": updated.email.to_lowercase() }, None)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
        {
            if let Some(id) = user.id {
                users
                    .update_one(
                        doc! { "_id": id },
                        doc! { "$set": { "role": "volunteer", "updatedAt": mongodb::bson::DateTime::from_chrono(Utc::now()) } },
                        None,
                    )
                    .await
                    .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
            }
            spawn_approval_email(
                Arc::clone(config),
                user.name,
                user.email,
                preferred_role.to_owned(),
                None,
            );
            info!(email = %updated.email, "Upgraded existing user to volunteer role");
        } else {
            let token = Uuid::new_v4().to_string();
            let now = Utc::now();
            let user = User {
                id: Some(mongodb::bson::oid::ObjectId::new()),
                name: updated.full_name.clone(),
                email: updated.email.to_lowercase(),
                phone: updated.phone_number.clone(),
                password: String::new(),
                role: UserRole::Volunteer,
                is_verified: true,
                security_version: 0,
                email_verification_code_hash: None,
                email_verification_code_expiry: None,
                email_verification_attempts: 0,
                password_reset_code_hash: None,
                password_reset_code_expiry: None,
                password_reset_attempts: 0,
                set_password_token: Some(token.clone()),
                set_password_token_expiry: Some(now + Duration::days(7)),
                created_at: Some(now),
                updated_at: Some(now),
            };
            users
                .insert_one(&user, None)
                .await
                .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
            let url = format!(
                "{}/set-password?token={token}",
                config.frontend_url.trim_end_matches('/')
            );
            spawn_approval_email(
                Arc::clone(config),
                user.name,
                user.email,
                preferred_role.to_owned(),
                Some(url),
            );
            info!(email = %updated.email, "Created new volunteer account and sent magic link");
        }
    } else if dto.status == ApplicationStatus::Rejected
        && matches!(
            previous_status,
            ApplicationStatus::Pending | ApplicationStatus::Approved
        )
    {
        if matches!(previous_status, ApplicationStatus::Approved) {
            if let Some(user) = users
                .find_one(doc! { "email": updated.email.to_lowercase() }, None)
                .await
                .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
            {
                if user.role == UserRole::Volunteer {
                    if let Some(id) = user.id {
                        users
                            .update_one(
                                doc! { "_id": id },
                                doc! { "$set": { "role": "attendee", "updatedAt": mongodb::bson::DateTime::from_chrono(Utc::now()) } },
                                None,
                            )
                            .await
                            .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
                        info!(email = %updated.email, "Downgraded volunteer role to attendee");
                    }
                }
            }
        }
        spawn_rejection_email(
            Arc::clone(config),
            updated.full_name.clone(),
            updated.email.clone(),
        );
    }
    Ok(updated)
}

fn spawn_approval_email(
    config: Arc<Config>,
    name: String,
    email: String,
    preferred_role: String,
    set_password_url: Option<String>,
) {
    let site_url = config.frontend_url.clone();
    tokio::spawn(async move {
        let url = set_password_url.as_deref();
        let html = crate::utils::email::volunteer_approval_email_html(
            &name,
            &site_url,
            &preferred_role,
            url,
        );
        if let Err(error) = crate::utils::email::send_email(
            &email,
            &name,
            "You've been approved as a TEDxAchievers volunteer!",
            &html,
            &config,
        )
        .await
        {
            error!(%error, "Volunteer approval email failed");
        }
    });
}

fn spawn_rejection_email(config: Arc<Config>, name: String, email: String) {
    let site_url = config.frontend_url.clone();
    tokio::spawn(async move {
        let html = crate::utils::email::volunteer_rejection_email_html(&name, &site_url);
        if let Err(error) = crate::utils::email::send_email(
            &email,
            &name,
            "Your TEDxAchievers volunteer application update",
            &html,
            &config,
        )
        .await
        {
            error!(%error, "Volunteer rejection email failed");
        }
    });
}
