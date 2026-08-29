use crate::{config::Config, errors::AppError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaystackVerifyResponse {
    pub status: String,
    pub metadata: Option<serde_json::Value>,
}
pub async fn initialize_payment(
    _email: &str,
    _amount: u64,
    _reference: &str,
    _callback_url: &str,
    _config: &Arc<Config>,
) -> Result<String, AppError> {
    todo!()
}
pub async fn verify_payment(
    _reference: &str,
    _config: &Arc<Config>,
) -> Result<PaystackVerifyResponse, AppError> {
    todo!()
}
