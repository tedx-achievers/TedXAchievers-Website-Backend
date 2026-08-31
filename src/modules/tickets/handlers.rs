use super::{
    dto::{InitiateTicketDto, VerifyOtpDto},
    service,
};
use crate::{
    errors::AppError,
    middleware::role::{RequireAttendee, RequireVolunteer},
    AppState,
};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use std::sync::Arc;

pub async fn initiate_ticket(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<InitiateTicketDto>,
) -> Result<impl IntoResponse, AppError> {
    validator::Validate::validate(&dto).map_err(|e| AppError::BadRequest(e.to_string()))?;
    Ok((
        StatusCode::OK,
        Json(service::initiate_ticket(&state.db, &state.config, &state.email_queue, dto).await?),
    ))
}
pub async fn verify_otp(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<VerifyOtpDto>,
) -> Result<impl IntoResponse, AppError> {
    if dto.code.len() != 6 {
        return Err(AppError::BadRequest(
            "Verification code must be 6 characters".to_owned(),
        ));
    }
    Ok((
        StatusCode::OK,
        Json(service::verify_otp(&state.db, &state.config, dto).await?),
    ))
}
pub async fn my_ticket(
    State(state): State<Arc<AppState>>,
    RequireAttendee(user): RequireAttendee,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(
        serde_json::json!({"success":true,"data":service::my_ticket(&state.db,&user.id).await?}),
    ))
}
pub async fn verify_ticket(
    State(state): State<Arc<AppState>>,
    RequireVolunteer(_): RequireVolunteer,
    Path(code): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(
        serde_json::json!({"success":true,"data":service::verify_ticket(&state.db,&code).await?}),
    ))
}
pub async fn checkin_ticket(
    State(state): State<Arc<AppState>>,
    RequireVolunteer(_): RequireVolunteer,
    Path(code): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(
        serde_json::json!({"success":true,"data":service::checkin(&state.db,&code).await?}),
    ))
}
pub async fn webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let signature = headers
        .get("x-squad-encrypted-body")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    use hmac::{Hmac, Mac};
    use sha2::Sha512;
    let mut mac = Hmac::<Sha512>::new_from_slice(state.config.squad_webhook_secret.as_bytes())
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Webhook configuration error")))?;
    mac.update(&body);
    let expected = hex::encode(mac.finalize().into_bytes());
    if !expected.eq_ignore_ascii_case(signature) {
        return Err(AppError::Unauthorized);
    }
    let payload: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("Invalid webhook payload".to_owned()))?;
    let event_name = payload.get("Event").and_then(|v| v.as_str());
    if event_name != Some("charge_successful") && event_name != Some("charge.success") {
        return Ok(Json(serde_json::json!({"success":true,"ignored":true})));
    }
    let event = payload
        .get("Body")
        .ok_or_else(|| AppError::BadRequest("Webhook body missing".to_owned()))?;
    let reference = event
        .get("transaction_ref")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Transaction reference missing".to_owned()))?;
    service::verify_ticket_payment(&state.db, &state.config, reference).await?;
    if let Some(ticket) = state
        .db
        .collection::<crate::models::ticket::Ticket>("tickets")
        .find_one(mongodb::bson::doc! {"payment_ref":reference}, None)
        .await
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Ticket lookup failed")))?
    {
        if let Some(user) = state
            .db
            .collection::<crate::models::user::User>("users")
            .find_one(mongodb::bson::doc! {"_id":ticket.user_id}, None)
            .await
            .map_err(|_| AppError::Internal(anyhow::anyhow!("User lookup failed")))?
        {
            if let (Some(qr), Some(token)) = (ticket.qr_code, user.set_password_token) {
                let config = Arc::clone(&state.config);
                let name = user.name;
                let email = user.email;
                let code = ticket.ticket_code;
                let tier = ticket.tier.display_name().to_owned();
                let amount = format!("NGN {:.2}", ticket.amount_kobo as f64 / 100.0);
                let event_name = config.event_name.clone();
                let event_theme = config.event_theme.clone();
                let event_date = config.event_date.clone();
                let event_time = config.event_time.clone();
                let event_venue = config.event_venue.clone();
                tokio::spawn(async move {
                    let link = format!(
                        "{}/set-password?token={}",
                        config.frontend_url.trim_end_matches('/'),
                        token
                    );
                    let html = crate::utils::email::ticket_confirmation_email_html(
                        &name,
                        &config.frontend_url,
                        &code,
                        &tier,
                        &amount,
                        &event_name,
                        &event_date,
                        &event_time,
                        &event_venue,
                        &qr,
                        &link,
                    );
                    let pdf = crate::utils::pdf::generate_ticket_pdf(
                        &name,
                        &email,
                        &code,
                        &qr,
                        &tier,
                        &amount,
                        &event_name,
                        &event_theme,
                        &event_date,
                        &event_time,
                        &event_venue,
                        &chrono::Utc::now().to_rfc3339(),
                    );
                    if let Ok(pdf) = pdf {
                        use base64::{engine::general_purpose::STANDARD, Engine};
                        if let Err(error) = crate::utils::email::send_email_with_attachment(
                            &email,
                            &name,
                            "Your TEDxAchievers Ticket 🎟",
                            &html,
                            "TEDxAchievers-Ticket.pdf",
                            &STANDARD.encode(pdf),
                            &config,
                        )
                        .await
                        {
                            tracing::error!(%error, recipient = %email, "Ticket confirmation email failed");
                        }
                    }
                });
            }
        }
    }
    Ok(Json(serde_json::json!({"success":true})))
}
