use super::{
    dto::{
        ForgotPasswordDto, LoginDto, RegisterDto, ResendVerificationDto, ResetPasswordDto,
        VerifyEmailDto,
    },
    service,
};
use crate::modules::tickets::dto::SetPasswordDto;
use crate::{errors::AppError, AppState};
use axum::{
    extract::{Json, State},
    http::{header::SET_COOKIE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::CookieJar;
use serde_json::json;
use std::sync::Arc;
use validator::Validate;

fn validation_response(error: validator::ValidationErrors) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        axum::Json(json!({ "success": false, "message": error.to_string() })),
    )
        .into_response()
}
fn cookie_header(
    name: &str,
    value: &str,
    path: &str,
    max_age: u64,
) -> Result<HeaderValue, AppError> {
    HeaderValue::from_str(&format!(
        "{name}={value}; HttpOnly; Secure; SameSite=None; Path={path}; Max-Age={max_age}"
    ))
    .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}
fn response_with_cookies(message: &str, cookies: Vec<HeaderValue>) -> Response {
    let mut response = (StatusCode::OK, axum::Json(json!({ "message": message }))).into_response();
    for cookie in cookies {
        response.headers_mut().append(SET_COOKIE, cookie);
    }
    response
}
pub async fn register_handler(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<RegisterDto>,
) -> Result<impl IntoResponse, AppError> {
    body.name = body.name.trim().to_owned();
    body.email = body.email.trim().to_lowercase();
    body.phone = body.phone.trim().to_owned();
    if let Err(error) = body.validate() {
        return Ok(validation_response(error));
    }
    service::register(&state.db, &state.config, &state.email_queue, body).await?;
    Ok((
        StatusCode::CREATED,
        axum::Json(json!({ "message": "Registration successful. Check your email to verify." })),
    )
        .into_response())
}
pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<LoginDto>,
) -> Result<impl IntoResponse, AppError> {
    body.email = body.email.trim().to_lowercase();
    if let Err(error) = body.validate() {
        return Ok(validation_response(error));
    }
    let (access_token, refresh_token) = service::login(&state.db, &state.config, body).await?;
    let cookies = vec![
        cookie_header(
            "access_token",
            &access_token,
            "/",
            state.config.jwt_access_expires_secs,
        )?,
        cookie_header(
            "refresh_token",
            &refresh_token,
            "/api/auth/refresh",
            state.config.jwt_refresh_expires_secs,
        )?,
    ];
    Ok(response_with_cookies("Login successful", cookies))
}
pub async fn refresh_handler(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<impl IntoResponse, AppError> {
    let token = jar
        .get("refresh_token")
        .map(|cookie| cookie.value().to_owned())
        .ok_or(AppError::Unauthorized)?;
    let access_token = service::refresh(&state.db, &state.config, &token).await?;
    let cookie = cookie_header(
        "access_token",
        &access_token,
        "/",
        state.config.jwt_access_expires_secs,
    )?;
    let mut response = (
        StatusCode::OK,
        axum::Json(json!({ "message": "Token refreshed" })),
    )
        .into_response();
    response.headers_mut().append(SET_COOKIE, cookie);
    Ok(response)
}
pub async fn logout_handler(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<impl IntoResponse, AppError> {
    if let Some(cookie) = jar.get("refresh_token") {
        service::logout(&state.db, cookie.value()).await?;
    }
    let cookies = vec![
        cookie_header("access_token", "", "/", 0)?,
        cookie_header("refresh_token", "", "/api/auth/refresh", 0)?,
    ];
    Ok(response_with_cookies("Logged out", cookies))
}
pub async fn verify_email_handler(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<VerifyEmailDto>,
) -> Result<impl IntoResponse, AppError> {
    body.email = body.email.trim().to_lowercase();
    if let Err(error) = body.validate() {
        return Ok(validation_response(error));
    }
    service::verify_email(&state.db, body).await?;
    Ok((
        StatusCode::OK,
        axum::Json(json!({ "message": "Email verified successfully" })),
    )
        .into_response())
}

pub async fn resend_verification_handler(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<ResendVerificationDto>,
) -> Result<impl IntoResponse, AppError> {
    body.email = body.email.trim().to_lowercase();
    if let Err(error) = body.validate() {
        return Ok(validation_response(error));
    }
    service::resend_verification(
        &state.db,
        &state.config,
        &state.email_queue,
        &state.verification_resends,
        body,
    )
    .await?;
    Ok(axum::Json(json!({
        "message": "If that email requires verification, a new verification code has been sent"
    }))
    .into_response())
}
pub async fn forgot_password_handler(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<ForgotPasswordDto>,
) -> Result<impl IntoResponse, AppError> {
    body.email = body.email.trim().to_lowercase();
    if let Err(error) = body.validate() {
        return Ok(validation_response(error));
    }
    service::forgot_password(&state.db, &state.config, &state.email_queue, body).await?;
    Ok((
        StatusCode::OK,
        axum::Json(json!({ "message": "If that email exists, a reset code has been sent" })),
    )
        .into_response())
}
pub async fn reset_password_handler(
    State(state): State<Arc<AppState>>,
    Json(mut body): Json<ResetPasswordDto>,
) -> Result<impl IntoResponse, AppError> {
    body.email = body.email.trim().to_lowercase();
    if let Err(error) = body.validate() {
        return Ok(validation_response(error));
    }
    service::reset_password(&state.db, &state.cache, body).await?;
    Ok((
        StatusCode::OK,
        axum::Json(json!({ "message": "Password reset successful" })),
    )
        .into_response())
}

pub async fn set_password_handler(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<SetPasswordDto>,
) -> Result<impl IntoResponse, AppError> {
    validator::Validate::validate(&dto).map_err(|error| AppError::BadRequest(error.to_string()))?;
    let (access_token, refresh_token) =
        service::set_password(&state.db, &state.config, dto).await?;
    let cookies = vec![
        cookie_header(
            "access_token",
            &access_token,
            "/",
            state.config.jwt_access_expires_secs,
        )?,
        cookie_header(
            "refresh_token",
            &refresh_token,
            "/api/auth/refresh",
            state.config.jwt_refresh_expires_secs,
        )?,
    ];
    state.cache.clear();
    Ok(response_with_cookies(
        "Password set successfully. Welcome!",
        cookies,
    ))
}
