use std::sync::Arc;

use chrono::{Duration, Utc};
use mongodb::{bson::doc, Database};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    config::Config,
    errors::AppError,
    models::{
        refresh_token::RefreshToken,
        user::{User, UserRole},
    },
    utils::{
        email::send_email,
        hash::{hash_password, verify_password},
        jwt::{sign_access_token, sign_refresh_token, verify_refresh_token},
    },
};

use super::dto::{ForgotPasswordDto, LoginDto, RegisterDto, ResetPasswordDto, VerifyEmailDto};

const USERS: &str = "users";
const REFRESH_TOKENS: &str = "refresh_tokens";

fn database_error(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!(error.to_string()))
}

pub async fn register(
    db: &Database,
    config: &Arc<Config>,
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
    let verify_token = Uuid::new_v4().to_string();
    let now = Utc::now();
    let user = User {
        id: None,
        name: dto.name.trim().to_owned(),
        email: email.clone(),
        phone: dto.phone.trim().to_owned(),
        password: hash_password(&dto.password)?,
        role: UserRole::Attendee,
        is_verified: false,
        verify_token: Some(verify_token.clone()),
        verify_token_expiry: Some(now + Duration::hours(24)),
        reset_token: None,
        reset_token_expiry: None,
        created_at: Some(now),
        updated_at: Some(now),
    };
    users
        .insert_one(&user, None)
        .await
        .map_err(database_error)?;
    let mail_config = Arc::clone(config);
    let name = user.name.clone();
    let mail_email = email.clone();
    let link = format!(
        "{}/verify-email?token={}",
        config.frontend_url, verify_token
    );
    tokio::spawn(async move {
        if let Err(error) = send_email(
            &mail_email,
            &name,
            "Verify your email",
            &format!("<p>Verify your email: <a href=\"{link}\">Verify email</a></p>"),
            &mail_config,
        )
        .await
        {
            error!(%error, "Verification email could not be sent");
        }
    });
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
    let access_token = sign_access_token(&user_id_string, &user.email, &user.role, config)?;
    let refresh_token = sign_refresh_token(&user_id_string, config)?;
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
    sign_access_token(&user_id.to_hex(), &user.email, &user.role, config)
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
        .find_one(doc! { "verify_token": dto.token }, None)
        .await
        .map_err(database_error)?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired token".to_owned()))?;
    if user
        .verify_token_expiry
        .is_none_or(|expiry| expiry <= Utc::now())
    {
        return Err(AppError::BadRequest("Invalid or expired token".to_owned()));
    }
    users.update_one(doc! { "_id": user.id }, doc! { "$set": { "is_verified": true, "verify_token": mongodb::bson::Bson::Null, "verify_token_expiry": mongodb::bson::Bson::Null, "updated_at": mongodb::bson::DateTime::from_millis(Utc::now().timestamp_millis()) } }, None).await.map_err(database_error)?;
    Ok(())
}

pub async fn forgot_password(
    db: &Database,
    config: &Arc<Config>,
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
    let reset_token = Uuid::new_v4().to_string();
    let now = Utc::now();
    users.update_one(doc! { "_id": user.id }, doc! { "$set": { "reset_token": &reset_token, "reset_token_expiry": mongodb::bson::DateTime::from_millis((now + Duration::hours(1)).timestamp_millis()), "updated_at": mongodb::bson::DateTime::from_millis(now.timestamp_millis()) } }, None).await.map_err(database_error)?;
    let mail_config = Arc::clone(config);
    let name = user.name;
    let link = format!(
        "{}/reset-password?token={}",
        config.frontend_url, reset_token
    );
    tokio::spawn(async move {
        if let Err(error) = send_email(
            &email,
            &name,
            "Reset your password",
            &format!("<p>Reset your password: <a href=\"{link}\">Reset password</a></p>"),
            &mail_config,
        )
        .await
        {
            error!(%error, "Password reset email could not be sent");
        }
    });
    Ok(())
}

pub async fn reset_password(db: &Database, dto: ResetPasswordDto) -> Result<(), AppError> {
    let users = db.collection::<User>(USERS);
    let user = users
        .find_one(doc! { "reset_token": dto.token }, None)
        .await
        .map_err(database_error)?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired token".to_owned()))?;
    if user
        .reset_token_expiry
        .is_none_or(|expiry| expiry <= Utc::now())
    {
        return Err(AppError::BadRequest("Invalid or expired token".to_owned()));
    }
    let password = hash_password(&dto.new_password)?;
    users.update_one(doc! { "_id": user.id }, doc! { "$set": { "password": password, "reset_token": mongodb::bson::Bson::Null, "reset_token_expiry": mongodb::bson::Bson::Null, "updated_at": mongodb::bson::DateTime::from_millis(Utc::now().timestamp_millis()) } }, None).await.map_err(database_error)?;
    Ok(())
}
