use std::sync::Arc;

use reqwest::Client;
use serde_json::json;
use tracing::{error, info};

use crate::{config::Config, errors::AppError};

pub async fn send_email(
    to_email: &str,
    to_name: &str,
    subject: &str,
    html: &str,
    config: &Arc<Config>,
) -> Result<(), AppError> {
    let response = Client::new()
        .post("https://api.brevo.com/v3/smtp/email")
        .header("api-key", &config.brevo_api_key)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&json!({
            "sender": {
                "name": config.brevo_sender_name,
                "email": config.brevo_sender_email,
            },
            "to": [{ "email": to_email, "name": to_name }],
            "subject": subject,
            "htmlContent": html,
        }))
        .send()
        .await
        .map_err(|error| {
            error!(%error, recipient = to_email, "Brevo email request failed");
            AppError::Internal(anyhow::anyhow!(error))
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|error| format!("Unable to read response body: {error}"));
        error!(%status, %body, recipient = to_email, "Brevo rejected email request");
        return Err(AppError::Internal(anyhow::anyhow!(
            "Brevo returned {status}: {body}"
        )));
    }
    info!(recipient = to_email, "Email sent successfully");
    Ok(())
}
