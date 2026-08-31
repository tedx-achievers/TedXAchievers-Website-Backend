use serde::Deserialize;
use validator::Validate;

use crate::models::ticket::TicketTier;

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct InitiateTicketDto {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 7, max = 30))]
    pub phone: String,
    pub tier: TicketTier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyOtpDto {
    pub email: String,
    pub code: String,
    pub tier: TicketTier,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SetPasswordDto {
    pub token: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
}
