use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::models::{ticket::Ticket, user::User};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserView {
    pub id: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub role: String,
    pub is_verified: bool,
    pub created_at: Option<DateTime<Utc>>,
}

impl From<User> for AdminUserView {
    fn from(user: User) -> Self {
        let role = match user.role {
            crate::models::user::UserRole::Attendee => "attendee",
            crate::models::user::UserRole::Volunteer => "volunteer",
            crate::models::user::UserRole::Admin => "admin",
        };
        Self {
            id: user.id.map(|id| id.to_hex()).unwrap_or_default(),
            name: user.name,
            email: user.email,
            phone: user.phone,
            role: role.to_owned(),
            is_verified: user.is_verified,
            created_at: user.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAttendeeView {
    pub user: AdminUserView,
    pub ticket: Option<AdminTicketView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminTicketView {
    pub ticket_code: String,
    pub payment_ref: String,
    pub status: String,
    pub checked_in: bool,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

impl From<Ticket> for AdminTicketView {
    fn from(ticket: Ticket) -> Self {
        let status = match ticket.status {
            crate::models::ticket::TicketStatus::Pending => "Pending",
            crate::models::ticket::TicketStatus::Paid => "Paid",
            crate::models::ticket::TicketStatus::Cancelled => "Cancelled",
        };
        Self {
            ticket_code: ticket.ticket_code,
            payment_ref: ticket.payment_ref,
            status: status.to_owned(),
            checked_in: ticket.checked_in,
            checked_in_at: ticket.checked_in_at,
            created_at: ticket.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub total_registered: u64,
    pub total_verified: u64,
    pub total_tickets_paid: u64,
    pub total_checked_in: u64,
    pub total_volunteer_applications: u64,
    pub pending_volunteers: u64,
    pub approved_volunteers: u64,
    pub rejected_volunteers: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}
