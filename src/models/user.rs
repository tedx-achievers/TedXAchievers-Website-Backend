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
    #[serde(default, rename = "securityVersion")]
    pub security_version: u64,
    #[serde(default, rename = "emailVerificationCodeHash")]
    pub email_verification_code_hash: Option<String>,
    #[serde(default, rename = "emailVerificationCodeExpiry")]
    pub email_verification_code_expiry: Option<DateTime<Utc>>,
    #[serde(default, rename = "emailVerificationAttempts")]
    pub email_verification_attempts: u32,
    #[serde(default, rename = "passwordResetCodeHash")]
    pub password_reset_code_hash: Option<String>,
    #[serde(default, rename = "passwordResetCodeExpiry")]
    pub password_reset_code_expiry: Option<DateTime<Utc>>,
    #[serde(default, rename = "passwordResetAttempts")]
    pub password_reset_attempts: u32,
    #[serde(default, rename = "setPasswordToken")]
    pub set_password_token: Option<String>,
    #[serde(default, rename = "setPasswordTokenExpiry")]
    pub set_password_token_expiry: Option<DateTime<Utc>>,
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
