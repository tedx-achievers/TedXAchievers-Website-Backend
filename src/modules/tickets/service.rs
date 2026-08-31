use super::dto::{InitiateTicketDto, SetPasswordDto, VerifyOtpDto};
use crate::{
    config::Config,
    errors::AppError,
    models::{
        ticket::{Ticket, TicketStatus, TicketTier},
        user::{User, UserRole},
    },
    utils::{
        hash::{hash_password, verify_password},
        squad,
    },
};
use chrono::{Duration, Utc};
use mongodb::{
    bson::{doc, oid::ObjectId},
    Database,
};
use rand::Rng;
use std::sync::Arc;
use uuid::Uuid;

const USERS: &str = "users";
const TICKETS: &str = "tickets";
const OTP_MINUTES: i64 = 10;
fn db_error(error: mongodb::error::Error) -> AppError {
    AppError::Internal(anyhow::anyhow!(error))
}
fn code() -> String {
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000u32))
}
async fn existing_paid(db: &Database, user_id: ObjectId) -> Result<bool, AppError> {
    Ok(db
        .collection::<Ticket>(TICKETS)
        .find_one(doc! {"user_id":user_id,"status":"paid"}, None)
        .await
        .map_err(db_error)?
        .is_some())
}

pub async fn initiate_ticket(
    db: &Database,
    config: &Arc<Config>,
    queue: &tokio::sync::mpsc::Sender<crate::utils::email::EmailJob>,
    dto: InitiateTicketDto,
) -> Result<serde_json::Value, AppError> {
    let email = dto.email.trim().to_lowercase();
    let users = db.collection::<User>(USERS);
    if let Some(user) = users
        .find_one(doc! {"email":&email}, None)
        .await
        .map_err(db_error)?
    {
        let id = user
            .id
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("User identifier missing")))?;
        if existing_paid(db, id).await? {
            return Err(AppError::Conflict(
                "A paid ticket already exists for this email".to_owned(),
            ));
        }
        if !user.is_verified {
            let otp = code();
            let now = Utc::now();
            users.update_one(doc! {"_id":id}, doc! {"$set":{"emailVerificationCodeHash":hash_password(&otp)?,"emailVerificationCodeExpiry":mongodb::bson::DateTime::from_chrono(now+Duration::minutes(OTP_MINUTES)),"emailVerificationAttempts":0}}, None).await.map_err(db_error)?;
            crate::utils::email::enqueue(
                queue,
                crate::utils::email::EmailJob {
                    to_email: email,
                    to_name: user.name.clone(),
                    subject: "Verify your TEDxAchievers ticket purchase".to_owned(),
                    html: crate::utils::email::ticket_otp_email_html(
                        &user.name,
                        &config.frontend_url,
                        &otp,
                    ),
                },
            );
            return Ok(
                serde_json::json!({"status":"verification_required","message":"A verification code was sent to your email"}),
            );
        }
        if let Some(ticket) = db
            .collection::<Ticket>(TICKETS)
            .find_one(doc! {"user_id":id,"status":"pending"}, None)
            .await
            .map_err(db_error)?
        {
            let checkout = squad::initiate_payment(
                &email,
                &user.name,
                &ticket.payment_ref,
                ticket.amount_kobo,
                config,
            )
            .await?;
            return Ok(
                serde_json::json!({"status":"payment_required","checkoutUrl":checkout,"paymentRef":ticket.payment_ref}),
            );
        }
        let (ticket, checkout) = create_pending(db, config, &user, dto.tier).await?;
        return Ok(
            serde_json::json!({"status":"payment_required","checkoutUrl":checkout,"paymentRef":ticket.payment_ref}),
        );
    }
    let now = Utc::now();
    let otp = code();
    let user = User {
        id: None,
        name: dto.name.trim().to_owned(),
        email: email.clone(),
        phone: dto.phone.trim().to_owned(),
        password: String::new(),
        role: UserRole::Attendee,
        is_verified: false,
        security_version: 0,
        email_verification_code_hash: Some(hash_password(&otp)?),
        email_verification_code_expiry: Some(now + Duration::minutes(OTP_MINUTES)),
        email_verification_attempts: 0,
        password_reset_code_hash: None,
        password_reset_code_expiry: None,
        password_reset_attempts: 0,
        set_password_token: None,
        set_password_token_expiry: None,
        created_at: Some(now),
        updated_at: Some(now),
    };
    users.insert_one(&user, None).await.map_err(|error| {
        if error.to_string().contains("E11000") {
            AppError::Conflict("Email already registered".to_owned())
        } else {
            db_error(error)
        }
    })?;
    crate::utils::email::enqueue(
        queue,
        crate::utils::email::EmailJob {
            to_email: email,
            to_name: user.name.clone(),
            subject: "Verify your TEDxAchievers ticket purchase".to_owned(),
            html: crate::utils::email::ticket_otp_email_html(
                &user.name,
                &config.frontend_url,
                &otp,
            ),
        },
    );
    Ok(
        serde_json::json!({"status":"verification_required","message":"A verification code was sent to your email"}),
    )
}

async fn create_pending(
    db: &Database,
    config: &Arc<Config>,
    user: &User,
    tier: TicketTier,
) -> Result<(Ticket, String), AppError> {
    let user_id = user
        .id
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("User identifier missing")))?;
    let payment_ref = format!("TEDX-{}", Uuid::new_v4().simple());
    let ticket = Ticket {
        id: None,
        user_id,
        event_id: ObjectId::new(),
        ticket_code: crate::utils::ticket_code::generate_ticket_code(),
        qr_code: None,
        payment_ref: payment_ref.clone(),
        status: TicketStatus::Pending,
        tier: tier.clone(),
        amount_kobo: tier.price_kobo(),
        checked_in: false,
        checked_in_at: None,
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    };
    db.collection::<Ticket>(TICKETS)
        .insert_one(&ticket, None)
        .await
        .map_err(db_error)?;
    let checkout = squad::initiate_payment(
        &user.email,
        &user.name,
        &payment_ref,
        ticket.amount_kobo,
        config,
    )
    .await?;
    Ok((ticket, checkout))
}

pub async fn verify_otp(
    db: &Database,
    config: &Arc<Config>,
    dto: VerifyOtpDto,
) -> Result<serde_json::Value, AppError> {
    let email = dto.email.trim().to_lowercase();
    let users = db.collection::<User>(USERS);
    let user = users
        .find_one(doc! {"email":&email}, None)
        .await
        .map_err(db_error)?
        .ok_or(AppError::Unauthorized)?;
    let id = user.id.ok_or(AppError::Unauthorized)?;
    let expiry = user
        .email_verification_code_expiry
        .ok_or(AppError::Unauthorized)?;
    let hash = user
        .email_verification_code_hash
        .as_ref()
        .ok_or(AppError::Unauthorized)?;
    if expiry < Utc::now() || user.email_verification_attempts >= 5 {
        return Err(AppError::Unauthorized);
    }
    users
        .update_one(
            doc! {"_id":id},
            doc! {"$inc":{"emailVerificationAttempts":1}},
            None,
        )
        .await
        .map_err(db_error)?;
    if !verify_password(&dto.code, &hash)? {
        return Err(AppError::Unauthorized);
    }
    users.update_one(doc!{"_id":id},doc!{"$set":{"isVerified":true},"$unset":{"emailVerificationCodeHash":"","emailVerificationCodeExpiry":""}},None).await.map_err(db_error)?;
    let mut verified = user;
    verified.id = Some(id);
    verified.is_verified = true;
    let (ticket, checkout) = create_pending(db, config, &verified, dto.tier).await?;
    Ok(
        serde_json::json!({"status":"payment_required","checkoutUrl":checkout,"paymentRef":ticket.payment_ref}),
    )
}
pub async fn my_ticket(db: &Database, user_id: &str) -> Result<Ticket, AppError> {
    let id = ObjectId::parse_str(user_id).map_err(|_| AppError::Unauthorized)?;
    db.collection::<Ticket>(TICKETS)
        .find_one(doc! {"user_id":id,"status":"paid"}, None)
        .await
        .map_err(db_error)?
        .ok_or_else(|| AppError::NotFound("No paid ticket found".to_owned()))
}
pub async fn verify_ticket(db: &Database, ticket_code: &str) -> Result<Ticket, AppError> {
    db.collection::<Ticket>(TICKETS)
        .find_one(doc! {"ticket_code":ticket_code}, None)
        .await
        .map_err(db_error)?
        .ok_or_else(|| AppError::NotFound("Ticket not found".to_owned()))
}
pub async fn checkin(db: &Database, ticket_code: &str) -> Result<Ticket, AppError> {
    let ticket = verify_ticket(db, ticket_code).await?;
    if ticket.status != TicketStatus::Paid {
        return Err(AppError::BadRequest("Ticket is not paid".to_owned()));
    }
    if ticket.checked_in {
        return Err(AppError::Conflict(
            "Ticket has already been checked in".to_owned(),
        ));
    }
    db.collection::<Ticket>(TICKETS).find_one_and_update(doc!{"_id":ticket.id,"checked_in":false},doc!{"$set":{"checked_in":true,"checked_in_at":mongodb::bson::DateTime::from_chrono(Utc::now()),"updated_at":mongodb::bson::DateTime::from_chrono(Utc::now())}},None).await.map_err(db_error)?.ok_or_else(||AppError::Conflict("Ticket has already been checked in".to_owned()))
}
pub async fn set_password(
    db: &Database,
    config: &Arc<Config>,
    dto: SetPasswordDto,
) -> Result<(String, String), AppError> {
    let users = db.collection::<User>(USERS);
    let user = users
        .find_one(doc! {"setPasswordToken":&dto.token}, None)
        .await
        .map_err(db_error)?
        .ok_or(AppError::Unauthorized)?;
    let id = user.id.ok_or(AppError::Unauthorized)?;
    if user
        .set_password_token_expiry
        .ok_or(AppError::Unauthorized)?
        < Utc::now()
    {
        return Err(AppError::Unauthorized);
    }
    let version = user.security_version + 1;
    let version_bson = i64::try_from(version)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Security version overflow")))?;
    users.update_one(doc!{"_id":id},doc!{"$set":{"password":hash_password(&dto.password)?,"securityVersion":version_bson},"$unset":{"setPasswordToken":"","setPasswordTokenExpiry":""}},None).await.map_err(db_error)?;
    Ok((
        crate::utils::jwt::sign_access_token(
            &id.to_hex(),
            &user.email,
            &user.role,
            version,
            config,
        )?,
        crate::utils::jwt::sign_refresh_token(&id.to_hex(), version, config)?,
    ))
}
pub async fn verify_ticket_payment(
    db: &Database,
    config: &Arc<Config>,
    reference: &str,
) -> Result<(), AppError> {
    if !squad::verify_transaction(reference, config).await? {
        return Err(AppError::BadRequest("Payment was not verified".to_owned()));
    }
    let tickets = db.collection::<Ticket>(TICKETS);
    let ticket = tickets
        .find_one(doc! {"payment_ref":reference}, None)
        .await
        .map_err(db_error)?
        .ok_or_else(|| AppError::NotFound("Payment reference not found".to_owned()))?;
    if ticket.status == TicketStatus::Paid {
        return Ok(());
    }
    let id = ticket.user_id;
    let qr = crate::utils::qr::generate_qr_base64(&ticket.ticket_code)?;
    tickets.update_one(doc!{"_id":ticket.id},doc!{"$set":{"status":"paid","qr_code":&qr,"updated_at":mongodb::bson::DateTime::from_chrono(Utc::now())}},None).await.map_err(db_error)?;
    let token = Uuid::new_v4().to_string();
    db.collection::<User>(USERS).update_one(doc!{"_id":id},doc!{"$set":{"setPasswordToken":token,"setPasswordTokenExpiry":mongodb::bson::DateTime::from_chrono(Utc::now()+Duration::days(7))}},None).await.map_err(db_error)?;
    Ok(())
}
