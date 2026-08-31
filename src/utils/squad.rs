use std::sync::Arc;

use reqwest::Client;
use serde::Deserialize;

use crate::{config::Config, errors::AppError};

#[derive(Debug, Deserialize)]
struct InitiateResponse {
    data: Option<InitiateData>,
}

#[derive(Debug, Deserialize)]
struct InitiateData {
    checkout_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerifyResponse {
    data: Option<VerifyData>,
}

#[derive(Debug, Deserialize)]
struct VerifyData {
    transaction_status: Option<String>,
}

pub async fn initiate_payment(
    email: &str,
    name: &str,
    transaction_ref: &str,
    amount_kobo: u64,
    config: &Arc<Config>,
) -> Result<String, AppError> {
    let response = Client::new()
        .post(format!("{}/transaction/initiate", config.squad_base_url.trim_end_matches('/')))
        .bearer_auth(&config.squad_secret_key)
        .json(&serde_json::json!({
            "email": email,
            "amount": amount_kobo,
            "currency": "NGN",
            "initiate_type": "inline",
            "transaction_ref": transaction_ref,
            "customer_name": name,
            "callback_url": format!("{}/payment/callback", config.frontend_url.trim_end_matches('/')),
        }))
        .send()
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    if !response.status().is_success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "Squad payment initiation failed"
        )));
    }
    let payload = response
        .json::<InitiateResponse>()
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    payload
        .data
        .and_then(|data| data.checkout_url)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Squad checkout URL missing")))
}

pub async fn verify_transaction(
    transaction_ref: &str,
    config: &Arc<Config>,
) -> Result<bool, AppError> {
    let response = Client::new()
        .get(format!(
            "{}/transaction/verify/{}",
            config.squad_base_url.trim_end_matches('/'),
            transaction_ref
        ))
        .bearer_auth(&config.squad_secret_key)
        .send()
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    if !response.status().is_success() {
        return Ok(false);
    }
    let payload = response
        .json::<VerifyResponse>()
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    Ok(payload
        .data
        .and_then(|data| data.transaction_status)
        .is_some_and(|status| status.eq_ignore_ascii_case("Success")))
}
