use std::fmt::Write;

use mongodb::{
    bson::{doc, Regex},
    options::FindOptions,
    Database,
};
use tracing::info;

use crate::{
    errors::AppError,
    models::{ticket::Ticket, user::User, volunteer_application::VolunteerApplication},
};

use super::dto::{
    AdminAttendeeView, AdminTicketView, AdminUserView, DashboardStats, PaginatedResponse,
};

const USERS: &str = "users";
const TICKETS: &str = "tickets";
const VOLUNTEERS: &str = "volunteer_applications";

fn database_error(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!(error.to_string()))
}

fn pages(total: u64, per_page: u64) -> u64 {
    if total == 0 {
        0
    } else {
        (total + per_page - 1) / per_page
    }
}

fn escape_regex(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

pub async fn get_dashboard_stats(db: &Database) -> Result<DashboardStats, AppError> {
    let users = db.collection::<mongodb::bson::Document>(USERS);
    let tickets = db.collection::<mongodb::bson::Document>(TICKETS);
    let volunteers = db.collection::<mongodb::bson::Document>(VOLUNTEERS);
    let (
        total_registered,
        total_verified,
        total_tickets_paid,
        total_checked_in,
        total_volunteer_applications,
        pending_volunteers,
        approved_volunteers,
        rejected_volunteers,
    ) = tokio::join!(
        users.count_documents(doc! {}, None),
        users.count_documents(doc! { "isVerified": true }, None),
        tickets.count_documents(doc! { "status": "paid" }, None),
        tickets.count_documents(doc! { "status": "paid", "checkedIn": true }, None),
        volunteers.count_documents(doc! {}, None),
        volunteers.count_documents(doc! { "status": "pending" }, None),
        volunteers.count_documents(doc! { "status": "approved" }, None),
        volunteers.count_documents(doc! { "status": "rejected" }, None),
    );
    let counts = [
        total_registered,
        total_verified,
        total_tickets_paid,
        total_checked_in,
        total_volunteer_applications,
        pending_volunteers,
        approved_volunteers,
        rejected_volunteers,
    ]
    .into_iter()
    .map(|result| result.map_err(database_error))
    .collect::<Result<Vec<_>, _>>()?;
    info!("Admin dashboard statistics loaded");
    Ok(DashboardStats {
        total_registered: counts[0],
        total_verified: counts[1],
        total_tickets_paid: counts[2],
        total_checked_in: counts[3],
        total_volunteer_applications: counts[4],
        pending_volunteers: counts[5],
        approved_volunteers: counts[6],
        rejected_volunteers: counts[7],
    })
}

pub async fn list_attendees(
    db: &Database,
    page: u64,
    per_page: u64,
    search: Option<String>,
) -> Result<PaginatedResponse<AdminAttendeeView>, AppError> {
    let users = db.collection::<User>(USERS);
    let filter = search
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            let regex = Regex {
                pattern: escape_regex(value.trim()),
                options: "i".to_owned(),
            };
            doc! { "$or": [
                { "name": regex.clone() },
                { "email": regex }
            ] }
        })
        .unwrap_or_default();
    let total = users
        .count_documents(filter.clone(), None)
        .await
        .map_err(database_error)?;
    let mut cursor = users
        .find(
            filter,
            FindOptions::builder()
                .sort(doc! { "createdAt": -1 })
                .skip((page - 1).saturating_mul(per_page))
                .limit(per_page as i64)
                .build(),
        )
        .await
        .map_err(database_error)?;
    let tickets = db.collection::<Ticket>(TICKETS);
    let mut data = Vec::new();
    while cursor.advance().await.map_err(database_error)? {
        let user: User = cursor.deserialize_current().map_err(database_error)?;
        let ticket = match user.id {
            Some(user_id) => tickets
                .find_one(doc! { "userId": user_id }, None)
                .await
                .map_err(database_error)?
                .map(AdminTicketView::from),
            None => None,
        };
        data.push(AdminAttendeeView {
            user: AdminUserView::from(user),
            ticket,
        });
    }
    info!(page, per_page, total, "Admin attendee list loaded");
    Ok(PaginatedResponse {
        data,
        total,
        page,
        per_page,
        total_pages: pages(total, per_page),
    })
}

pub async fn export_attendees_csv(db: &Database) -> Result<String, AppError> {
    let mut cursor = db
        .collection::<User>(USERS)
        .find(
            doc! {},
            FindOptions::builder()
                .sort(doc! { "createdAt": -1 })
                .build(),
        )
        .await
        .map_err(database_error)?;
    let tickets = db.collection::<Ticket>(TICKETS);
    let mut csv = String::from(
        "name,email,phone,role,is_verified,ticket_code,ticket_status,checked_in,registered_at\n",
    );
    while cursor.advance().await.map_err(database_error)? {
        let user: User = cursor.deserialize_current().map_err(database_error)?;
        let ticket = match user.id {
            Some(user_id) => tickets
                .find_one(doc! { "userId": user_id }, None)
                .await
                .map_err(database_error)?,
            None => None,
        };
        let role = match user.role {
            crate::models::user::UserRole::Attendee => "attendee",
            crate::models::user::UserRole::Volunteer => "volunteer",
            crate::models::user::UserRole::Admin => "admin",
        };
        let (ticket_code, ticket_status, checked_in) = match ticket {
            Some(ticket) => {
                let status = match ticket.status {
                    crate::models::ticket::TicketStatus::Pending => "Pending",
                    crate::models::ticket::TicketStatus::Paid => "Paid",
                    crate::models::ticket::TicketStatus::Cancelled => "Cancelled",
                };
                (
                    ticket.ticket_code,
                    status.to_owned(),
                    ticket.checked_in.to_string(),
                )
            }
            None => (String::new(), String::new(), String::new()),
        };
        writeln!(
            csv,
            "{},{},{},{},{},{},{},{},{}",
            csv_field(&user.name),
            csv_field(&user.email),
            csv_field(&user.phone),
            role,
            user.is_verified,
            csv_field(&ticket_code),
            csv_field(&ticket_status),
            checked_in,
            user.created_at
                .map(|date| date.to_rfc3339())
                .unwrap_or_default(),
        )
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    }
    info!("Admin attendee CSV exported");
    Ok(csv)
}

pub async fn list_volunteers(
    db: &Database,
    status_filter: Option<String>,
    page: u64,
    per_page: u64,
) -> Result<PaginatedResponse<VolunteerApplication>, AppError> {
    let volunteers = db.collection::<VolunteerApplication>(VOLUNTEERS);
    let filter = status_filter
        .filter(|value| !value.trim().is_empty())
        .map(|status| doc! { "status": status.trim().to_lowercase() })
        .unwrap_or_default();
    let total = volunteers
        .count_documents(filter.clone(), None)
        .await
        .map_err(database_error)?;
    let mut cursor = volunteers
        .find(
            filter,
            FindOptions::builder()
                .sort(doc! { "createdAt": -1 })
                .skip((page - 1).saturating_mul(per_page))
                .limit(per_page as i64)
                .build(),
        )
        .await
        .map_err(database_error)?;
    let mut data = Vec::new();
    while cursor.advance().await.map_err(database_error)? {
        data.push(cursor.deserialize_current().map_err(database_error)?);
    }
    info!(page, per_page, total, "Admin volunteer list loaded");
    Ok(PaginatedResponse {
        data,
        total,
        page,
        per_page,
        total_pages: pages(total, per_page),
    })
}
