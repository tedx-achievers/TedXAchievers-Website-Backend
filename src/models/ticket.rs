use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: ObjectId,
    pub event_id: ObjectId,
    pub ticket_code: String,
    pub qr_code: Option<String>,
    pub payment_ref: String,
    pub status: TicketStatus,
    pub tier: TicketTier,
    pub amount_kobo: u64,
    #[serde(default)]
    pub checked_in: bool,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
#[derive(PartialEq)]
pub enum TicketStatus {
    Pending,
    Paid,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TicketTier {
    Student,
    General,
    Vip,
}

impl TicketTier {
    pub fn price_kobo(&self) -> u64 {
        match self {
            Self::Student => 500_000,
            Self::General => 750_000,
            Self::Vip => 1_000_000,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Student => "Student",
            Self::General => "General Admission",
            Self::Vip => "VIP",
        }
    }
}
