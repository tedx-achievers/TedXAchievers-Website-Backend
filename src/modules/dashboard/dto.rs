use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub id: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub role: String,
    pub is_verified: bool,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct TicketResponse {
    pub ticket_code: String,
    pub tier: String,
    pub amount_paid: String,
    pub status: String,
    pub checked_in: bool,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub qr_code: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct VolunteerResponse {
    pub reference_code: String,
    pub preferred_role: String,
    pub department: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct EventInfoResponse {
    pub name: String,
    pub date: String,
    pub time: String,
    pub venue: String,
    pub theme: String,
}

#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    pub profile: ProfileResponse,
    pub ticket: Option<TicketResponse>,
    pub volunteer: Option<VolunteerResponse>,
    pub event: EventInfoResponse,
}
