use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolunteerApplication {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub reference_code: String,
    pub full_name: String,
    pub email: String,
    pub phone_number: String,
    pub department: String,
    pub matric_number: String,
    pub preferred_role: PreferredRole,
    pub motivation: String,
    pub status: ApplicationStatus,
    #[serde(with = "crate::utils::datetime::bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "crate::utils::datetime::bson_datetime")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PreferredRole {
    Technical,
    Videography,
    Photography,
    Content,
    ProtocolAndUshering,
    Welfare,
    GraphicAndDesign,
    VenueAndDecoration,
    PartnershipAndSponsorship,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationStatus {
    Pending,
    Approved,
    Rejected,
}
