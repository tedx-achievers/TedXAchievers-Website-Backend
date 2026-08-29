use std::sync::Arc;

use tokio::sync::mpsc;

use reqwest::Client;
use serde_json::json;
use tracing::{error, info};

use crate::{config::Config, errors::AppError};

pub struct EmailJob {
    pub to_email: String,
    pub to_name: String,
    pub subject: String,
    pub html: String,
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn volunteer_application_received_html(
    name: &str,
    site_url: &str,
    application_id: &str,
    preferred_role: &str,
    department: &str,
    submitted_at: &str,
) -> String {
    let safe_name = escape_html(name);
    let safe_application_id = escape_html(application_id);
    let safe_preferred_role = escape_html(preferred_role);
    let safe_department = escape_html(department);
    let safe_submitted_at = escape_html(submitted_at);
    let site_url = site_url.trim_end_matches('/');
    let hero_url = format!("{site_url}/AUO_TEDxHS.png");
    let volunteer_url = format!("{site_url}/volunteers");
    format!(
        r##"<!doctype html>
<html lang="en">
<head><meta name="color-scheme" content="dark"><meta name="supported-color-schemes" content="dark"><style>@import url('https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700&display=swap');</style></head>
<body style="margin:0;padding:0;background-color:#050505;color:#ffffff;font-family:'Outfit',Arial,Helvetica,sans-serif;">
  <table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="background-color:#050505;">
    <tr><td align="center" style="padding:32px 16px;">
      <table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="max-width:620px;background-color:#0f0f0f;border-radius:18px 18px 0 0;overflow:hidden;">
        <tr><td style="height:5px;background-color:#e62b1e;font-size:0;line-height:0;">&nbsp;</td></tr>
        <tr><td align="center" style="padding:28px 32px 20px;background-color:#0f0f0f;">
          <div style="color:#e62b1e;font-family:Arial,Helvetica,sans-serif;font-size:46px;line-height:46px;letter-spacing:-3px;font-weight:900;">TEDX</div>
        </td></tr>
        <tr><td style="padding:0 32px 28px;"><img src="{hero_url}" width="556" alt="Achievers University campus" style="display:block;width:100%;max-width:556px;height:auto;border:0;border-radius:12px;"></td></tr>
        <tr><td style="padding:0 32px 34px;">
          <table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0">
            <tr><td style="padding:0 0 0;">
              <div style="color:#ffffff;font-size:27px;line-height:35px;font-weight:bold;margin-bottom:16px;">Welcome to the TEDxAchievers community.</div>
              <div style="color:#f0f0f0;font-size:16px;line-height:27px;">Hi {safe_name},</div>
              <div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:10px;">Thank you for stepping forward to volunteer for TEDxAchievers. Your application is safely with our team and will be reviewed carefully.</div>
              <div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:12px;">We will contact you with the next steps. Keep bringing your ideas, energy, and curiosity.</div>
              <table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="margin-top:24px;border:1px solid #303030;border-radius:10px;background-color:#101010;">
                <tr><td colspan="2" style="padding:16px 18px 8px;color:#e62b1e;font-size:11px;line-height:16px;letter-spacing:2px;font-weight:bold;text-transform:uppercase;">Application summary</td></tr>
                <tr><td style="padding:6px 18px;color:#777777;font-size:13px;line-height:22px;width:38%;">Reference code</td><td style="padding:6px 18px;color:#f0f0f0;font-size:13px;line-height:22px;word-break:break-all;">{safe_application_id}</td></tr>
                <tr><td style="padding:6px 18px;color:#777777;font-size:13px;line-height:22px;">Preferred role</td><td style="padding:6px 18px;color:#f0f0f0;font-size:13px;line-height:22px;">{safe_preferred_role}</td></tr>
                <tr><td style="padding:6px 18px;color:#777777;font-size:13px;line-height:22px;">Department</td><td style="padding:6px 18px;color:#f0f0f0;font-size:13px;line-height:22px;">{safe_department}</td></tr>
                <tr><td style="padding:6px 18px 16px;color:#777777;font-size:13px;line-height:22px;">Submitted</td><td style="padding:6px 18px 16px;color:#f0f0f0;font-size:13px;line-height:22px;">{safe_submitted_at}</td></tr>
              </table>
              <table role="presentation" cellspacing="0" cellpadding="0" border="0" style="margin-top:26px;">
                <tr><td align="center" style="border-radius:999px;background-color:#e62b1e;">
                  <a href="{volunteer_url}" style="display:inline-block;padding:14px 24px;border-radius:999px;color:#ffffff;font-size:14px;font-weight:bold;letter-spacing:.4px;text-decoration:none;">Visit the volunteer page</a>
                </td></tr>
              </table>
            </td></tr>
          </table>
        </td></tr>
        <tr><td align="center" style="padding:0 32px 30px;color:#777777;font-size:12px;line-height:20px;">
          <div style="color:#a6a6a6;font-size:13px;font-weight:bold;margin-bottom:8px;">Stay connected</div>
          <a href="https://www.instagram.com/tedxachieversuniversity/" style="display:inline-block;margin:0 5px;color:#ffffff;text-decoration:none;background-color:#222222;border:1px solid #383838;border-radius:999px;padding:7px 12px;font-size:12px;"><img src="https://cdn.simpleicons.org/instagram/ffffff" width="15" height="15" alt="Instagram" style="display:inline-block;width:15px;height:15px;vertical-align:middle;border:0;margin-right:5px;">Instagram</a>
          <a href="https://www.tiktok.com/@tedxachieversuniversity" style="display:inline-block;margin:0 5px;color:#ffffff;text-decoration:none;background-color:#222222;border:1px solid #383838;border-radius:999px;padding:7px 12px;font-size:12px;"><img src="https://cdn.simpleicons.org/tiktok/ffffff" width="15" height="15" alt="TikTok" style="display:inline-block;width:15px;height:15px;vertical-align:middle;border:0;margin-right:5px;">TikTok</a>
          <div style="padding-top:16px;">Questions? <a href="mailto:admin@tedxachieversuniversity.com.ng" style="color:#a6a6a6;text-decoration:underline;">admin@tedxachieversuniversity.com.ng</a></div>
          <div>TEDxAchievers &bull; Achievers University</div>
          <a href="{site_url}" style="color:#a6a6a6;text-decoration:underline;">tedxachieversuniversity.com.ng</a>
        </td></tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"##
    )
}

pub fn start_worker(config: Arc<Config>) -> mpsc::Sender<EmailJob> {
    let (sender, mut receiver) = mpsc::channel::<EmailJob>(1_000);
    tokio::spawn(async move {
        while let Some(job) = receiver.recv().await {
            if let Err(error) = send_email(
                &job.to_email,
                &job.to_name,
                &job.subject,
                &job.html,
                &config,
            )
            .await
            {
                error!(%error, recipient = %job.to_email, "Queued email could not be sent");
            }
        }
    });
    sender
}

pub fn enqueue(sender: &mpsc::Sender<EmailJob>, job: EmailJob) {
    if let Err(error) = sender.try_send(job) {
        error!(%error, "Email queue is full; email was dropped");
    }
}

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
