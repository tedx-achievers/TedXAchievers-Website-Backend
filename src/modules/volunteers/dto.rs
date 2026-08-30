use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::models::volunteer_application::{ApplicationStatus, PreferredRole};

pub const DEPARTMENTS: [&str; 8] = [
    "technical",
    "videography",
    "photography",
    "content",
    "protocol_and_ushering",
    "welfare",
    "graphic_and_design",
    "venue_and_decoration",
];

pub fn canonical_department(value: &str) -> Option<&'static str> {
    DEPARTMENTS
        .iter()
        .copied()
        .find(|department| department.eq_ignore_ascii_case(value.trim()))
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ApplyVolunteerDto {
    #[validate(length(min = 1))]
    pub full_name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub phone_number: String,
    #[validate(length(min = 1))]
    pub department: String,
    #[validate(length(min = 1))]
    pub matric_number: String,
    pub preferred_role: PreferredRole,
    #[validate(length(min = 20))]
    pub motivation: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApplicationStatusDto {
    pub status: ApplicationStatus,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CheckStatusDto {
    pub email: String,
}
