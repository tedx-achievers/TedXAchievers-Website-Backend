use chrono::Utc;
use mongodb::bson::doc;
use mongodb::options::FindOptions;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use tedxachievers::config::db::connect_db;
use tedxachievers::config::Config;
use tedxachievers::errors::AppError;
use tedxachievers::models::volunteer_application::{PreferredRole, VolunteerApplication};

const TEST_MODE: bool = false;
const TEST_EMAIL: &str = "adeyekunadelola0@gmail.com";
const TEST_NAME: &str = "Test Volunteer";
const SUBJECT: &str = "Update Your TEDxAchievers Volunteer Role";
const SITE_URL: &str = "https://www.tedxachieversuniversity.com.ng";

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn role_name(role: &PreferredRole) -> &'static str {
    match role {
        PreferredRole::Technical => "Technical",
        PreferredRole::Videography => "Videography",
        PreferredRole::Photography => "Photography",
        PreferredRole::Content => "Content",
        PreferredRole::ProtocolAndUshering => "Protocol and Ushering",
        PreferredRole::Welfare => "Welfare",
        PreferredRole::GraphicAndDesign => "Graphic and Design",
        PreferredRole::VenueAndDecoration => "Venue and Decoration",
        PreferredRole::PartnershipAndSponsorship => "Partnership and Sponsorship",
    }
}

fn volunteer_email_html(name: &str, preferred_role: &str, reference_code: &str) -> String {
    let name = escape_html(name);
    let role = escape_html(preferred_role);
    let reference = escape_html(reference_code);
    format!(
        r##"<!doctype html><html lang="en"><head><meta name="color-scheme" content="dark"><meta name="supported-color-schemes" content="dark"><style>@import url('https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700&display=swap');</style></head><body style="margin:0;padding:0;background:#050505;color:#fff;font-family:'Outfit',Arial,sans-serif;"><table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="background:#050505;"><tr><td align="center" style="padding:32px 16px;"><table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="max-width:620px;background:#101010;"><tr><td style="height:5px;background:#e62b1e;font-size:0;line-height:0;">&nbsp;</td></tr><tr><td style="background:#0f0f0f;padding:30px 32px 24px;text-align:center;"><div style="font-size:32px;line-height:38px;font-weight:700;"><span style="color:#e62b1e;">TEDx</span> <span style="color:#fff;">Achievers</span></div></td></tr><tr><td style="background:#e62b1e;padding:25px 24px;text-align:center;color:#fff;font-size:25px;line-height:32px;font-weight:700;">Your Application Was Received 🎉</td></tr><tr><td style="padding:32px;"><div style="color:#fff;font-size:17px;line-height:27px;">Hi {name},</div><div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:12px;">Thank you for applying to volunteer at TEDxAchievers. Your application has been received and is currently under review.</div><div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:12px;">If you would like to update your preferred role, visit the link below and scroll down to the role-update section. You do not need to submit a new application; simply enter your email there and update your preferred role.</div><div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:12px;">Please note: you can only change your role once, so choose carefully.</div><table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="margin-top:24px;background:#151515;border:1px solid #303030;"><tr><td style="padding:18px;color:#a6a6a6;font-size:14px;line-height:24px;"><div>Current preferred role: <strong style="color:#fff;">{role}</strong></div><div>Reference code: <strong style="color:#fff;">{reference}</strong></div></td></tr></table><table role="presentation" align="center" cellspacing="0" cellpadding="0" border="0" style="margin-top:28px;"><tr><td align="center" style="border-radius:999px;background:#e62b1e;"><a href="{SITE_URL}/volunteers" style="display:inline-block;padding:15px 25px;color:#fff;font-size:15px;font-weight:700;text-decoration:none;border-radius:999px;">Update My Preferred Role</a></td></tr></table><div style="color:#777;font-size:12px;line-height:20px;text-align:center;padding-top:14px;">If you are satisfied with your current role, no action is needed.</div></td></tr><tr><td style="background:#0f0f0f;padding:24px 32px 29px;text-align:center;color:#a6a6a6;font-size:12px;line-height:20px;border-bottom:5px solid #e62b1e;">TEDxAchievers &bull; Achievers University<br><a href="{SITE_URL}" style="color:#a6a6a6;text-decoration:underline;">{SITE_URL}</a></td></tr></table></td></tr></table></body></html>"##,
    )
}

async fn send_email(
    config: &Config,
    to_email: &str,
    to_name: &str,
    html: &str,
) -> Result<(), AppError> {
    let response = Client::new()
        .post("https://api.brevo.com/v3/smtp/email")
        .header("api-key", &config.brevo_api_key)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&json!({
            "sender": { "name": config.brevo_sender_name, "email": config.brevo_sender_email },
            "to": [{ "email": to_email, "name": to_name }],
            "subject": SUBJECT,
            "htmlContent": html,
        }))
        .send()
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|error| format!("Unable to read response body: {error}"));
        return Err(AppError::Internal(anyhow::anyhow!(
            "Brevo returned {status}: {body}"
        )));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let _ = dotenvy::dotenv();
    let config = Arc::new(Config::from_env());
    let db = connect_db(&config).await;
    let mut cursor = db
        .collection::<VolunteerApplication>("volunteer_applications")
        .find(
            doc! {},
            FindOptions::builder()
                .sort(doc! { "createdAt": 1 })
                .limit(44)
                .build(),
        )
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    let mut applications = Vec::new();
    while cursor
        .advance()
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?
    {
        applications.push(
            cursor
                .deserialize_current()
                .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?,
        );
    }
    if applications.is_empty() {
        return Err(AppError::NotFound(
            "No volunteer applications were found".to_owned(),
        ));
    }
    if TEST_MODE {
        println!("TEST MODE: sending to TEST_EMAIL only");
        let application = &applications[0];
        let html = volunteer_email_html(
            TEST_NAME,
            role_name(&application.preferred_role),
            &application.reference_code,
        );
        send_email(&config, TEST_EMAIL, TEST_NAME, &html).await?;
        println!("Sent test email to {TEST_EMAIL}");
        return Ok(());
    }
    let total = applications.len();
    for (index, application) in applications.iter().enumerate() {
        println!("Sending [{}/{}]: {}", index + 1, total, application.email);
        let html = volunteer_email_html(
            &application.full_name,
            role_name(&application.preferred_role),
            &application.reference_code,
        );
        if let Err(error) =
            send_email(&config, &application.email, &application.full_name, &html).await
        {
            eprintln!("Failed to send to {}: {}", application.email, error);
        }
        if index + 1 < total {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    println!(
        "Completed sending {total} volunteer emails at {}",
        Utc::now().to_rfc3339()
    );
    Ok(())
}
