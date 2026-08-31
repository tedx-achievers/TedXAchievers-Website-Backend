use std::sync::Arc;

use chrono::{Duration, Utc};
use mongodb::{bson::doc, Database};
use tracing::info;

use crate::{
    config::Config,
    errors::AppError,
    middleware::auth::{invalidate_user_cache, CachedAuthUser},
    models::{
        refresh_token::RefreshToken,
        user::{User, UserRole},
    },
    utils::{
        hash::{hash_password, verify_password},
        jwt::{sign_access_token, sign_refresh_token, verify_refresh_token},
    },
};
use dashmap::DashMap;
use rand::Rng;
use tokio::sync::mpsc::Sender;

use super::dto::{ForgotPasswordDto, LoginDto, RegisterDto, ResetPasswordDto, VerifyEmailDto};

const USERS: &str = "users";
const REFRESH_TOKENS: &str = "refresh_tokens";
const CODE_TTL_MINUTES: i64 = 15;
const MAX_CODE_ATTEMPTS: u32 = 5;

fn is_six_digit_code(code: &str) -> bool {
    code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit())
}

fn database_error(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!(error.to_string()))
}

fn generate_code() -> String {
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000))
}

pub async fn register(
    db: &Database,
    config: &Arc<Config>,
    email_queue: &Sender<crate::utils::email::EmailJob>,
    dto: RegisterDto,
) -> Result<(), AppError> {
    let email = dto.email.trim().to_lowercase();
    let users = db.collection::<User>(USERS);
    if users
        .find_one(doc! { "email": &email }, None)
        .await
        .map_err(database_error)?
        .is_some()
    {
        return Err(AppError::Conflict("Email already registered".to_owned()));
    }
    let verification_code = generate_code();
    let verification_code_hash = hash_password(&verification_code)?;
    let now = Utc::now();
    let user = User {
        id: None,
        name: dto.name.trim().to_owned(),
        email: email.clone(),
        phone: dto.phone.trim().to_owned(),
        password: hash_password(&dto.password)?,
        role: UserRole::Attendee,
        is_verified: false,
        security_version: 0,
        email_verification_code_hash: Some(verification_code_hash),
        email_verification_code_expiry: Some(now + Duration::minutes(CODE_TTL_MINUTES)),
        email_verification_attempts: 0,
        password_reset_code_hash: None,
        password_reset_code_expiry: None,
        password_reset_attempts: 0,
        set_password_token: None,
        set_password_token_expiry: None,
        created_at: Some(now),
        updated_at: Some(now),
    };
    if let Err(error) = users.insert_one(&user, None).await {
        if error.to_string().contains("E11000") {
            return Err(AppError::Conflict("Email already registered".to_owned()));
        }
        return Err(database_error(error));
    }
    crate::utils::email::enqueue(
        email_queue,
        crate::utils::email::EmailJob {
            to_email: email.clone(),
            to_name: user.name.clone(),
            subject: "Verify your TEDxAchievers account".to_owned(),
            html: crate::utils::email::auth_code_email_html(
                &user.name,
                &config.frontend_url,
                "Verify your email",
                "Use the secure code below to verify your TEDxAchievers account.",
                &verification_code,
                "Verify your email",
                &format!("{}/verify-email", config.frontend_url.trim_end_matches('/')),
                CODE_TTL_MINUTES as u32,
            ),
        },
    );
    info!(email = %email, "User registered");
    Ok(())
}

pub async fn login(
    db: &Database,
    config: &Arc<Config>,
    dto: LoginDto,
) -> Result<(String, String), AppError> {
    let email = dto.email.trim().to_lowercase();
    let user = db
        .collection::<User>(USERS)
        .find_one(doc! { "email": &email }, None)
        .await
        .map_err(database_error)?
        .ok_or(AppError::Unauthorized)?;
    if !verify_password(&dto.password, &user.password)? {
        return Err(AppError::Unauthorized);
    }
    if !user.is_verified {
        return Err(AppError::Forbidden);
    }
    let user_id = user
        .id
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("User is missing an id")))?;
    let user_id_string = user_id.to_hex();
    let access_token = sign_access_token(
        &user_id_string,
        &user.email,
        &user.role,
        user.security_version,
        config,
    )?;
    let refresh_token = sign_refresh_token(&user_id_string, user.security_version, config)?;
    let now = Utc::now();
    db.collection::<RefreshToken>(REFRESH_TOKENS)
        .insert_one(
            &RefreshToken {
                id: None,
                token: refresh_token.clone(),
                user_id,
                expires_at: now + Duration::seconds(config.jwt_refresh_expires_secs as i64),
                created_at: Some(now),
            },
            None,
        )
        .await
        .map_err(database_error)?;
    info!(user_id = %user_id_string, "User logged in");
    Ok((access_token, refresh_token))
}

pub async fn refresh(
    db: &Database,
    config: &Arc<Config>,
    refresh_token_str: &str,
) -> Result<String, AppError> {
    let claims = verify_refresh_token(refresh_token_str, config)?;
    let token = db
        .collection::<RefreshToken>(REFRESH_TOKENS)
        .find_one(doc! { "token": refresh_token_str }, None)
        .await
        .map_err(database_error)?
        .ok_or(AppError::Unauthorized)?;
    if token.expires_at <= Utc::now() {
        return Err(AppError::Unauthorized);
    }
    let user = db
        .collection::<User>(USERS)
        .find_one(doc! { "_id": &token.user_id }, None)
        .await
        .map_err(database_error)?
        .ok_or(AppError::Unauthorized)?;
    let user_id = user
        .id
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("User is missing an id")))?;
    if user_id.to_hex() != claims.sub {
        return Err(AppError::Unauthorized);
    }
    if user.security_version != claims.security_version {
        return Err(AppError::Unauthorized);
    }
    sign_access_token(
        &user_id.to_hex(),
        &user.email,
        &user.role,
        user.security_version,
        config,
    )
}

pub async fn logout(db: &Database, refresh_token_str: &str) -> Result<(), AppError> {
    db.collection::<RefreshToken>(REFRESH_TOKENS)
        .delete_one(doc! { "token": refresh_token_str }, None)
        .await
        .map_err(database_error)?;
    Ok(())
}

pub async fn verify_email(db: &Database, dto: VerifyEmailDto) -> Result<(), AppError> {
    let users = db.collection::<User>(USERS);
    let user = users
        .find_one(doc! { "email": dto.email.trim().to_lowercase() }, None)
        .await
        .map_err(database_error)?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired verification code".to_owned()))?;
    if !is_six_digit_code(&dto.code)
        || user.email_verification_attempts >= MAX_CODE_ATTEMPTS
        || user.email_verification_code_hash.is_none()
        || user
            .email_verification_code_expiry
            .is_none_or(|expiry| expiry <= Utc::now())
    {
        return Err(AppError::BadRequest(
            "Invalid or expired verification code".to_owned(),
        ));
    }
    let valid = verify_password(
        &dto.code,
        user.email_verification_code_hash
            .as_deref()
            .unwrap_or_default(),
    )?;
    if !valid {
        users
            .update_one(
                doc! { "_id": user.id },
                doc! { "$inc": { "emailVerificationAttempts": 1_i64 } },
                None,
            )
            .await
            .map_err(database_error)?;
        return Err(AppError::BadRequest(
            "Invalid or expired verification code".to_owned(),
        ));
    }
    users.update_one(doc! { "_id": user.id }, doc! { "$set": { "isVerified": true, "emailVerificationCodeHash": mongodb::bson::Bson::Null, "emailVerificationCodeExpiry": mongodb::bson::Bson::Null, "emailVerificationAttempts": 0_i64, "updatedAt": mongodb::bson::DateTime::from_millis(Utc::now().timestamp_millis()) } }, None).await.map_err(database_error)?;
    Ok(())
}

pub async fn forgot_password(
    db: &Database,
    config: &Arc<Config>,
    email_queue: &Sender<crate::utils::email::EmailJob>,
    dto: ForgotPasswordDto,
) -> Result<(), AppError> {
    let email = dto.email.trim().to_lowercase();
    let users = db.collection::<User>(USERS);
    let Some(user) = users
        .find_one(doc! { "email": &email }, None)
        .await
        .map_err(database_error)?
    else {
        return Ok(());
    };
    let reset_code = generate_code();
    let reset_code_hash = hash_password(&reset_code)?;
    let now = Utc::now();
    users.update_one(doc! { "_id": user.id }, doc! { "$set": { "passwordResetCodeHash": reset_code_hash, "passwordResetCodeExpiry": mongodb::bson::DateTime::from_millis((now + Duration::minutes(CODE_TTL_MINUTES)).timestamp_millis()), "passwordResetAttempts": 0_i64, "updatedAt": mongodb::bson::DateTime::from_millis(now.timestamp_millis()) } }, None).await.map_err(database_error)?;
    crate::utils::email::enqueue(
        email_queue,
        crate::utils::email::EmailJob {
            to_email: email,
            to_name: user.name.clone(),
            subject: "Reset your TEDxAchievers password".to_owned(),
            html: crate::utils::email::auth_code_email_html(
                &user.name,
                &config.frontend_url,
                "Reset your password",
                "Use the secure code below to reset your TEDxAchievers password.",
                &reset_code,
                "Reset your password",
                &format!(
                    "{}/reset-password",
                    config.frontend_url.trim_end_matches('/')
                ),
                CODE_TTL_MINUTES as u32,
            ),
        },
    );
    Ok(())
}

pub async fn reset_password(
    db: &Database,
    cache: &DashMap<String, CachedAuthUser>,
    dto: ResetPasswordDto,
) -> Result<(), AppError> {
    let users = db.collection::<User>(USERS);
    let user = users
        .find_one(doc! { "email": dto.email.trim().to_lowercase() }, None)
        .await
        .map_err(database_error)?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired reset code".to_owned()))?;
    if !is_six_digit_code(&dto.code)
        || user.password_reset_attempts >= MAX_CODE_ATTEMPTS
        || user.password_reset_code_hash.is_none()
        || user
            .password_reset_code_expiry
            .is_none_or(|expiry| expiry <= Utc::now())
    {
        return Err(AppError::BadRequest(
            "Invalid or expired reset code".to_owned(),
        ));
    }
    let valid = verify_password(
        &dto.code,
        user.password_reset_code_hash.as_deref().unwrap_or_default(),
    )?;
    if !valid {
        users
            .update_one(
                doc! { "_id": user.id },
                doc! { "$inc": { "passwordResetAttempts": 1_i64 } },
                None,
            )
            .await
            .map_err(database_error)?;
        return Err(AppError::BadRequest(
            "Invalid or expired reset code".to_owned(),
        ));
    }
    let password = hash_password(&dto.new_password)?;
    let user_id = user
        .id
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("User is missing an id")))?;
    users.update_one(doc! { "_id": user_id }, doc! { "$set": { "password": password, "passwordResetCodeHash": mongodb::bson::Bson::Null, "passwordResetCodeExpiry": mongodb::bson::Bson::Null, "passwordResetAttempts": 0_i64, "updatedAt": mongodb::bson::DateTime::from_millis(Utc::now().timestamp_millis()) }, "$inc": { "securityVersion": 1_i64 } }, None).await.map_err(database_error)?;
    invalidate_user_cache(cache, &user_id);
    Ok(())
}
