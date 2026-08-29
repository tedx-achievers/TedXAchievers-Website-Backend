use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RegisterDto {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub phone: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct LoginDto {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailDto {
    #[validate(length(min = 1))]
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ForgotPasswordDto {
    #[validate(email)]
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordDto {
    #[validate(length(min = 1))]
    pub token: String,
    #[validate(length(min = 8))]
    pub new_password: String,
}
