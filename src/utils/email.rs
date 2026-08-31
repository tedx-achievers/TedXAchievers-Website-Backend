use std::sync::Arc;

use qrcode::{Color, QrCode};
use reqwest::Client;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::{config::Config, errors::AppError};

pub struct EmailJob {
    pub to_email: String,
    pub to_name: String,
    pub subject: String,
    pub html: String,
}

pub(crate) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn qr_grid_html(ticket_code: &str) -> String {
    let code = match QrCode::new(ticket_code.as_bytes()) {
        Ok(code) => code,
        Err(_) => {
            return "<div style=\"color:#777;padding:80px 20px;\">QR unavailable</div>".to_owned()
        }
    };
    let modules = code.width();
    let colors = code.to_colors();
    let mut html = String::from("<table role=\"presentation\" cellspacing=\"0\" cellpadding=\"0\" border=\"0\" style=\"width:220px;height:220px;background:#fff;\"><tbody>");
    for y in 0..modules {
        html.push_str("<tr>");
        for x in 0..modules {
            let color = match colors[y * modules + x] {
                Color::Dark => "#000",
                Color::Light => "#fff",
            };
            html.push_str(&format!("<td style=\"width:{}px;height:{}px;background:{};font-size:0;line-height:0;\">&nbsp;</td>", 220 / modules, 220 / modules, color));
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table>");
    html
}

fn social_icons_html() -> String {
    r#"<a href="https://www.instagram.com/tedxachieversuniversity/" style="display:inline-block;margin:0 6px;background:#222;border:1px solid #383838;border-radius:999px;padding:8px;line-height:0;"><img src="https://img.icons8.com/fluency/48/instagram-new.png" width="22" height="22" alt="Instagram" style="display:block;width:22px;height:22px;border:0;"></a><a href="https://www.tiktok.com/@tedxachieversuniversity" style="display:inline-block;margin:0 6px;background:#222;border:1px solid #383838;border-radius:999px;padding:8px;line-height:0;"><img src="https://img.icons8.com/color/48/tiktok--v1.png" width="22" height="22" alt="TikTok" style="display:block;width:22px;height:22px;border:0;"></a>"#.to_owned()
}

fn email_shell(site_url: &str, body: &str, cta: Option<(&str, &str)>) -> String {
    let site_url = site_url.trim_end_matches('/');
    let logo_url = format!("{site_url}/logo-white.png");
    let hero_url = format!("{site_url}/AUO_TEDxHS.png");
    let social_icons = social_icons_html();
    let cta_html = cta
        .map(|(label, url)| {
            format!(
                r#"<table role="presentation" align="center" cellspacing="0" cellpadding="0" border="0" style="margin-top:26px;"><tr><td style="border-radius:999px;background:#e62b1e;"><a href="{}" style="display:inline-block;padding:14px 24px;border-radius:999px;color:#fff;font-size:14px;font-weight:700;text-decoration:none;">{}</a></td></tr></table>"#,
                escape_html(url),
                escape_html(label)
            )
        })
        .unwrap_or_default();
    format!(
        r##"<!doctype html><html lang="en"><head><meta name="color-scheme" content="dark"><meta name="supported-color-schemes" content="dark"><style>@import url('https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700&display=swap');</style></head><body style="margin:0;padding:0;background:#050505;color:#fff;font-family:'Outfit',Arial,Helvetica,sans-serif;"><table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="background:#050505;"><tr><td align="center" style="padding:32px 16px;"><table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="max-width:620px;background:#0f0f0f;border-radius:18px 18px 0 0;overflow:hidden;"><tr><td style="height:5px;background:#e62b1e;font-size:0;line-height:0;">&nbsp;</td></tr><tr><td align="center" style="padding:28px 32px 20px;"><img src="{logo_url}" width="420" alt="TEDxAchievers" style="display:block;width:100%;max-width:420px;height:auto;border:0;"></td></tr><tr><td><img src="{hero_url}" width="620" alt="Achievers University campus" style="display:block;width:100%;height:auto;border:0;"></td></tr><tr><td style="padding:32px;">{body}{cta_html}</td></tr><tr><td align="center" style="padding:24px 32px 30px;color:#777;font-size:12px;line-height:20px;"><div style="color:#a6a6a6;font-size:13px;font-weight:700;margin-bottom:8px;">Stay connected</div>{social_icons}<div style="padding-top:16px;">Questions? <a href="mailto:admin@tedxachieversuniversity.com.ng" style="color:#a6a6a6;text-decoration:underline;">admin@tedxachieversuniversity.com.ng</a></div><div>TEDxAchievers &bull; Achievers University</div><a href="{site_url}" style="color:#a6a6a6;text-decoration:underline;">tedxachieversuniversity.com.ng</a></td></tr><tr><td style="height:5px;background:#e62b1e;font-size:0;line-height:0;">&nbsp;</td></tr></table></td></tr></table></body></html>"##
    )
}

pub fn auth_code_email_html(
    name: &str,
    site_url: &str,
    heading: &str,
    message: &str,
    code: &str,
    cta_label: &str,
    cta_url: &str,
    expires_in_minutes: u32,
) -> String {
    let body = format!(
        r#"<div style="color:#fff;font-size:27px;line-height:35px;font-weight:700;margin-bottom:16px;">{}</div><div style="color:#f0f0f0;font-size:16px;line-height:27px;">Hi {},</div><div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:10px;">{}</div><table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="margin-top:24px;background:#151515;border:1px solid #303030;"><tr><td align="center" style="padding:22px 18px 8px;color:#a6a6a6;font-size:12px;letter-spacing:1.5px;text-transform:uppercase;">Your secure code</td></tr><tr><td align="center" style="padding:0 18px 20px;color:#e62b1e;font-size:36px;line-height:44px;font-weight:700;letter-spacing:9px;">{}</td></tr></table><div style="color:#888;font-size:13px;line-height:21px;padding-top:16px;">This code expires in {} minutes. If you did not request this, you can safely ignore this email.</div>"#,
        escape_html(heading),
        escape_html(name),
        escape_html(message),
        escape_html(code),
        expires_in_minutes,
    );
    email_shell(site_url, &body, Some((cta_label, cta_url)))
}

pub fn ticket_otp_email_html(name: &str, site_url: &str, code: &str) -> String {
    auth_code_email_html(
        name,
        site_url,
        "Verify your ticket purchase",
        "Use this code to continue securely with your TEDxAchievers ticket purchase.",
        code,
        "Visit TEDxAchievers",
        site_url,
        10,
    )
}

pub fn volunteer_admin_notification_html(
    site_url: &str,
    application_id: &str,
    name: &str,
    email: &str,
    department: &str,
    preferred_role: &str,
    submitted_at: &str,
) -> String {
    let body = format!(
        r#"<div style="color:#fff;font-size:27px;line-height:35px;font-weight:700;">New volunteer application</div><div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:12px;">A new volunteer application has been submitted and is ready for review.</div><table width="100%" cellspacing="0" cellpadding="0" border="0" style="margin-top:24px;border:1px solid #303030;background:#101010;"><tr><td style="padding:16px 18px 8px;color:#e62b1e;font-size:11px;letter-spacing:2px;font-weight:700;text-transform:uppercase;">Application summary</td></tr><tr><td style="padding:6px 18px;color:#aaa;font-size:14px;">Name: <span style="color:#fff;">{}</span></td></tr><tr><td style="padding:6px 18px;color:#aaa;font-size:14px;">Email: <span style="color:#fff;">{}</span></td></tr><tr><td style="padding:6px 18px;color:#aaa;font-size:14px;">Reference: <span style="color:#fff;">{}</span></td></tr><tr><td style="padding:6px 18px;color:#aaa;font-size:14px;">Department: <span style="color:#fff;">{}</span></td></tr><tr><td style="padding:6px 18px;color:#aaa;font-size:14px;">Preferred role: <span style="color:#fff;">{}</span></td></tr><tr><td style="padding:6px 18px 16px;color:#aaa;font-size:14px;">Submitted: <span style="color:#fff;">{}</span></td></tr></table>"#,
        escape_html(name),
        escape_html(email),
        escape_html(application_id),
        escape_html(department),
        escape_html(preferred_role),
        escape_html(submitted_at),
    );
    email_shell(
        site_url,
        &body,
        Some((
            "Review application",
            &format!("{}/admin/volunteers", site_url),
        )),
    )
}

pub fn volunteer_application_received_html(
    name: &str,
    site_url: &str,
    application_id: &str,
    preferred_role: &str,
    department: &str,
    submitted_at: &str,
) -> String {
    let body = format!(
        r#"<div style="color:#fff;font-size:27px;line-height:35px;font-weight:700;margin-bottom:16px;">Welcome to the TEDxAchievers community.</div><div style="color:#f0f0f0;font-size:16px;line-height:27px;">Hi {},</div><div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:10px;">Thank you for stepping forward to volunteer for TEDxAchievers. Your application is safely with our team and will be reviewed carefully.</div><div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:12px;">We will contact you with the next steps. Keep bringing your ideas, energy, and curiosity.</div><table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="margin-top:24px;border:1px solid #303030;border-radius:10px;background:#101010;"><tr><td colspan="2" style="padding:16px 18px 8px;color:#e62b1e;font-size:11px;line-height:16px;letter-spacing:2px;font-weight:700;text-transform:uppercase;">Application summary</td></tr><tr><td style="padding:6px 18px;color:#777;font-size:13px;width:38%;">Reference code</td><td style="padding:6px 18px;color:#f0f0f0;font-size:13px;word-break:break-all;">{}</td></tr><tr><td style="padding:6px 18px;color:#777;font-size:13px;">Preferred role</td><td style="padding:6px 18px;color:#f0f0f0;font-size:13px;">{}</td></tr><tr><td style="padding:6px 18px;color:#777;font-size:13px;">Department</td><td style="padding:6px 18px;color:#f0f0f0;font-size:13px;">{}</td></tr><tr><td style="padding:6px 18px 16px;color:#777;font-size:13px;">Submitted</td><td style="padding:6px 18px 16px;color:#f0f0f0;font-size:13px;">{}</td></tr></table>"#,
        escape_html(name),
        escape_html(application_id),
        escape_html(preferred_role),
        escape_html(department),
        escape_html(submitted_at),
    );
    email_shell(
        site_url,
        &body,
        Some((
            "Visit the volunteer page",
            &format!("{}/volunteers", site_url.trim_end_matches('/')),
        )),
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
            "sender": { "name": config.brevo_sender_name, "email": config.brevo_sender_email },
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

pub async fn send_email_with_attachment(
    to_email: &str,
    to_name: &str,
    subject: &str,
    html: &str,
    attachment_name: &str,
    attachment_base64: &str,
    config: &Arc<Config>,
) -> Result<(), AppError> {
    let response = Client::new()
        .post("https://api.brevo.com/v3/smtp/email")
        .header("api-key", &config.brevo_api_key)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&json!({
            "sender": { "name": config.brevo_sender_name, "email": config.brevo_sender_email },
            "to": [{ "email": to_email, "name": to_name }],
            "subject": subject,
            "htmlContent": html,
            "attachment": [{ "content": attachment_base64, "name": attachment_name }]
        }))
        .send()
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
    if !response.status().is_success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "Brevo rejected ticket email"
        )));
    }
    Ok(())
}

pub fn ticket_confirmation_email_html(
    name: &str,
    site_url: &str,
    ticket_code: &str,
    tier: &str,
    amount: &str,
    event_name: &str,
    event_date: &str,
    event_time: &str,
    event_venue: &str,
    qr_base64: &str,
    set_password_url: &str,
) -> String {
    let _ = qr_base64;
    let qr_html = qr_grid_html(ticket_code);
    let body = format!(
        r#"<div style="background:#e62b1e;padding:28px 24px;color:#fff;text-align:center;margin:0 -32px 28px;"><div style="font-size:30px;line-height:38px;font-weight:700;">Your ticket is confirmed 🎟</div><div style="font-size:15px;line-height:24px;padding-top:6px;">See you at {}!</div></div><div style="color:#f0f0f0;font-size:16px;line-height:27px;">Hi {},</div><div style="color:#a6a6a6;font-size:15px;line-height:25px;padding-top:10px;">Your ticket has been confirmed. Present the QR code below at the entrance on event day. Save this email or screenshot the QR code.</div><table width="100%" cellspacing="0" cellpadding="0" border="0" style="margin-top:24px;border:1px solid #303030;border-radius:10px;background:#151515;"><tr><td colspan="2" style="padding:16px 18px 8px;color:#e62b1e;font-size:11px;letter-spacing:2px;font-weight:700;text-transform:uppercase;">Ticket details</td></tr><tr><td style="padding:7px 18px;color:#a6a6a6;font-size:13px;">TICKET CODE</td><td style="padding:7px 18px;color:#e62b1e;font-family:monospace;font-weight:700;font-size:14px;">{}</td></tr><tr><td style="padding:7px 18px;color:#a6a6a6;font-size:13px;">TIER</td><td style="padding:7px 18px;color:#fff;font-size:13px;">{}</td></tr><tr><td style="padding:7px 18px;color:#a6a6a6;font-size:13px;">AMOUNT PAID</td><td style="padding:7px 18px;color:#fff;font-size:13px;">{}</td></tr><tr><td colspan="2" style="padding:10px 18px;border-top:1px solid #303030;"></td></tr><tr><td style="padding:7px 18px;color:#a6a6a6;font-size:13px;">DATE</td><td style="padding:7px 18px;color:#fff;font-size:13px;">{}</td></tr><tr><td style="padding:7px 18px;color:#a6a6a6;font-size:13px;">TIME</td><td style="padding:7px 18px;color:#fff;font-size:13px;">{}</td></tr><tr><td style="padding:7px 18px 16px;color:#a6a6a6;font-size:13px;">VENUE</td><td style="padding:7px 18px 16px;color:#fff;font-size:13px;">{}</td></tr></table><div style="padding-top:28px;text-align:center;color:#a6a6a6;font-size:11px;letter-spacing:1.5px;font-weight:700;">SCAN AT THE ENTRANCE</div><div style="text-align:center;padding:14px 0;"><div style="display:inline-block;background:#fff;padding:12px;border-radius:12px;">{}</div></div><div style="text-align:center;color:#777;font-family:monospace;font-size:13px;letter-spacing:2px;">{}</div><div style="text-align:center;color:#a6a6a6;font-size:13px;line-height:21px;padding-top:24px;">Access your ticket anytime from your dashboard. This link expires in 7 days.</div>"#,
        escape_html(event_name),
        escape_html(name),
        escape_html(ticket_code),
        escape_html(tier),
        escape_html(amount),
        escape_html(event_date),
        escape_html(event_time),
        escape_html(event_venue),
        qr_html,
        escape_html(ticket_code)
    )
    .replace(
        "Your ticket has been confirmed. Present the QR code below at the entrance on event day. Save this email or screenshot the QR code.",
        "Your ticket is confirmed, and your printable PDF ticket is attached to this email. The QR code shown below is a convenient fallback for check-in. Keep the PDF or this email handy and present either QR code at the entrance on event day.",
    );
    email_shell(
        site_url,
        &body,
        Some(("Set Up Dashboard Access", set_password_url)),
    )
}
