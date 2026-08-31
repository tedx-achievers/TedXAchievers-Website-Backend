use super::dto::{
    DashboardResponse, EventInfoResponse, ProfileResponse, TicketResponse, VolunteerResponse,
};
use crate::{
    config::Config,
    errors::AppError,
    models::{
        ticket::{Ticket, TicketStatus},
        user::{User, UserRole},
        volunteer_application::{ApplicationStatus, PreferredRole, VolunteerApplication},
    },
};
use chrono::Utc;
use mongodb::{
    bson::{doc, oid::ObjectId},
    options::{FindOneOptions, ReturnDocument},
    Database,
};
use std::sync::Arc;
use tracing::{error, info};

const USERS: &str = "users";
const TICKETS: &str = "tickets";
const VOLUNTEERS: &str = "volunteer_applications";

fn db_error(error: mongodb::error::Error) -> AppError {
    error!(%error, "Dashboard database operation failed");
    AppError::Internal(anyhow::anyhow!(error))
}

fn role_name(role: &UserRole) -> &'static str {
    match role {
        UserRole::Attendee => "Attendee",
        UserRole::Volunteer => "Volunteer",
        UserRole::Admin => "Admin",
    }
}

fn status_name(status: &TicketStatus) -> &'static str {
    match status {
        TicketStatus::Pending => "Pending",
        TicketStatus::Paid => "Paid",
        TicketStatus::Cancelled => "Cancelled",
    }
}

fn application_status_name(status: &ApplicationStatus) -> &'static str {
    match status {
        ApplicationStatus::Pending => "Pending",
        ApplicationStatus::Approved => "Approved",
        ApplicationStatus::Rejected => "Rejected",
    }
}

fn preferred_role_name(role: &PreferredRole) -> &'static str {
    match role {
        PreferredRole::Technical => "Technical",
        PreferredRole::Videography => "Videography",
        PreferredRole::Photography => "Photography",
        PreferredRole::Content => "Content",
        PreferredRole::ProtocolAndUshering => "Protocol and Ushering",
        PreferredRole::Welfare => "Welfare",
        PreferredRole::GraphicAndDesign => "Graphic and Design",
        PreferredRole::VenueAndDecoration => "Venue and Decoration",
        PreferredRole::PartnershipAndSponsorship => "Partnership and Sponsorship",
    }
}

fn comma_number(value: u64) -> String {
    let digits = value.to_string();
    let first_len = digits.len() % 3;
    let mut result = String::new();
    if first_len > 0 {
        result.push_str(&digits[..first_len]);
    }
    let start = first_len;
    for (index, chunk) in digits[start..].as_bytes().chunks(3).enumerate() {
        if first_len > 0 || index > 0 {
            result.push(',');
        }
        if let Ok(text) = std::str::from_utf8(chunk) {
            result.push_str(text);
        }
    }
    result
}

fn amount(value: u64) -> String {
    format!("NGN {}.{:02}", comma_number(value / 100), value % 100)
}

fn profile(user: &User, id: ObjectId) -> ProfileResponse {
    ProfileResponse {
        id: id.to_hex(),
        name: user.name.clone(),
        email: user.email.clone(),
        phone: user.phone.clone(),
        role: role_name(&user.role).to_owned(),
        is_verified: user.is_verified,
        created_at: user.created_at,
    }
}

fn ticket(ticket: Ticket) -> TicketResponse {
    TicketResponse {
        ticket_code: ticket.ticket_code,
        tier: ticket.tier.display_name().to_owned(),
        amount_paid: amount(ticket.amount_kobo),
        status: status_name(&ticket.status).to_owned(),
        checked_in: ticket.checked_in,
        checked_in_at: ticket.checked_in_at,
        qr_code: ticket.qr_code,
        created_at: ticket.created_at,
    }
}

fn volunteer(application: VolunteerApplication) -> VolunteerResponse {
    VolunteerResponse {
        reference_code: application.reference_code,
        preferred_role: preferred_role_name(&application.preferred_role).to_owned(),
        department: application.department,
        status: application_status_name(&application.status).to_owned(),
        created_at: application.created_at,
        updated_at: application.updated_at,
    }
}

fn event(config: &Config) -> EventInfoResponse {
    EventInfoResponse {
        name: config.event_name.clone(),
        date: config.event_date.clone(),
        time: config.event_time.clone(),
        venue: config.event_venue.clone(),
        theme: config.event_theme.clone(),
    }
}

async fn find_user(db: &Database, user_id: &str) -> Result<(ObjectId, User), AppError> {
    let id = ObjectId::parse_str(user_id).map_err(|_| AppError::Unauthorized)?;
    let user = db
        .collection::<User>(USERS)
        .find_one(doc! {"_id": id}, None)
        .await
        .map_err(db_error)?
        .ok_or(AppError::Unauthorized)?;
    Ok((id, user))
}

async fn find_volunteer(
    db: &Database,
    email: &str,
) -> Result<Option<VolunteerApplication>, AppError> {
    db.collection::<VolunteerApplication>(VOLUNTEERS)
        .find_one(
            doc! {"email": email.trim().to_lowercase()},
            Some(
                FindOneOptions::builder()
                    .sort(doc! {"created_at": -1})
                    .build(),
            ),
        )
        .await
        .map_err(db_error)
}

pub async fn get_dashboard(
    db: &Database,
    config: &Arc<Config>,
    user_id: &str,
    email: &str,
) -> Result<DashboardResponse, AppError> {
    let (id, user) = find_user(db, user_id).await?;
    let ticket = db
        .collection::<Ticket>(TICKETS)
        .find_one(doc! {"user_id": id, "status": "paid"}, None)
        .await
        .map_err(db_error)?
        .map(ticket);
    let volunteer = find_volunteer(db, &user.email).await?.map(volunteer);
    info!(user_id, email, "Dashboard loaded");
    Ok(DashboardResponse {
        profile: profile(&user, id),
        ticket,
        volunteer,
        event: event(config),
    })
}

pub async fn get_profile(db: &Database, user_id: &str) -> Result<ProfileResponse, AppError> {
    let (id, user) = find_user(db, user_id).await?;
    Ok(profile(&user, id))
}

pub async fn update_profile(
    db: &Database,
    user_id: &str,
    name: Option<String>,
    phone: Option<String>,
) -> Result<ProfileResponse, AppError> {
    if name.is_none() && phone.is_none() {
        return Err(AppError::BadRequest("No fields to update".to_owned()));
    }
    let (id, _) = find_user(db, user_id).await?;
    let mut set = doc! {"updated_at": mongodb::bson::DateTime::from_chrono(Utc::now())};
    if let Some(value) = name {
        set.insert("name", value);
    }
    if let Some(value) = phone {
        set.insert("phone", value);
    }
    let user = db
        .collection::<User>(USERS)
        .find_one_and_update(
            doc! {"_id": id},
            doc! {"$set": set},
            Some(
                mongodb::options::FindOneAndUpdateOptions::builder()
                    .return_document(ReturnDocument::After)
                    .build(),
            ),
        )
        .await
        .map_err(db_error)?
        .ok_or(AppError::Unauthorized)?;
    Ok(profile(&user, id))
}

pub async fn get_ticket(db: &Database, user_id: &str) -> Result<TicketResponse, AppError> {
    let id = ObjectId::parse_str(user_id).map_err(|_| AppError::Unauthorized)?;
    db.collection::<Ticket>(TICKETS)
        .find_one(doc! {"user_id": id, "status": "paid"}, None)
        .await
        .map_err(db_error)?
        .map(ticket)
        .ok_or_else(|| AppError::NotFound("No ticket found".to_owned()))
}

pub async fn get_volunteer(db: &Database, email: &str) -> Result<VolunteerResponse, AppError> {
    find_volunteer(db, email)
        .await?
        .map(volunteer)
        .ok_or_else(|| AppError::NotFound("No volunteer application found".to_owned()))
}

pub fn get_event(config: &Config) -> EventInfoResponse {
    event(config)
}
