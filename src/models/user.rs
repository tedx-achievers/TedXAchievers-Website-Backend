use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub password: String,
    pub role: UserRole,
    #[serde(default)]
    pub is_verified: bool,
    pub verify_token: Option<String>,
    pub verify_token_expiry: Option<DateTime<Utc>>,
    pub reset_token: Option<String>,
    pub reset_token_expiry: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum UserRole {
    Attendee,
    Volunteer,
    Admin,
}
impl UserRole {
    pub fn has_attendee_access(&self) -> bool {
        true
    }
    pub fn has_volunteer_access(&self) -> bool {
        matches!(self, Self::Volunteer | Self::Admin)
    }
    pub fn has_admin_access(&self) -> bool {
        matches!(self, Self::Admin)
    }
}
