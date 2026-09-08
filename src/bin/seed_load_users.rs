use std::{env, fs, path::Path};

use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use mongodb::{
    bson::{doc, Document},
    options::ClientOptions,
    Client,
};
use serde::Serialize;
use tedxachievers::{
    models::user::UserRole,
    utils::jwt::Claims,
};

const COLLECTION: &str = "users";
const DEFAULT_COUNT: usize = 50;
const DEFAULT_TOKEN_FILE: &str = "k6-tests/auth-tokens.json";

#[derive(Serialize)]
struct TokenFile {
    emails: Vec<String>,
    tokens: Vec<String>,
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("Required environment variable is missing: {name}"))
}

fn role_from_document(document: &Document) -> Result<UserRole, Box<dyn std::error::Error>> {
    Ok(match document.get_str("role")? {
        "attendee" => UserRole::Attendee,
        "volunteer" => UserRole::Volunteer,
        "admin" => UserRole::Admin,
        role => return Err(format!("Unsupported user role: {role}").into()),
    })
}

fn security_version(document: &Document) -> u64 {
    document
        .get_i64("securityVersion")
        .ok()
        .and_then(|value| u64::try_from(value).ok())
        .or_else(|| document.get_i32("securityVersion").ok().map(|value| value as u64))
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let mongodb_uri = required("MONGODB_URI");
    let jwt_secret = required("JWT_ACCESS_SECRET");
    let password = required("LOAD_TEST_PASSWORD");
    let count = env::var("LOAD_TEST_COUNT")
        .unwrap_or_else(|_| DEFAULT_COUNT.to_string())
        .parse::<usize>()?;
    let email_domain = env::var("LOAD_TEST_EMAIL_DOMAIN")
        .unwrap_or_else(|_| "loadtest.invalid".to_owned());
    let email_prefix = env::var("LOAD_TEST_EMAIL_PREFIX")
        .unwrap_or_else(|_| "tedx-loadtest".to_owned());
    let token_file = env::var("LOAD_TEST_TOKEN_FILE")
        .unwrap_or_else(|_| DEFAULT_TOKEN_FILE.to_owned());
    let expires_secs = env::var("JWT_ACCESS_EXPIRES_SECS")
        .unwrap_or_else(|_| "900".to_owned())
        .parse::<u64>()?;

    if count < 50 {
        return Err("LOAD_TEST_COUNT must be at least 50".into());
    }
    if password.len() < 8 {
        return Err("LOAD_TEST_PASSWORD must be at least 8 characters".into());
    }

    let options = ClientOptions::parse(&mongodb_uri).await?;
    let database_name = options
        .default_database
        .clone()
        .unwrap_or_else(|| "tedxachievers".to_owned());
    let client = Client::with_options(options)?;
    let users = client
        .database(&database_name)
        .collection::<Document>(COLLECTION);
    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
    let now = Utc::now();
    let mut emails = Vec::with_capacity(count);
    let mut tokens = Vec::with_capacity(count);

    for index in 1..=count {
        let email = format!("{email_prefix}-{index:03}@{email_domain}");
        let existing = users.find_one(doc! { "email": &email }, None).await?;
        let (user_id, user_email, role, user_security_version) = if let Some(existing) = existing {
            if !existing.get_bool("isVerified").unwrap_or(false) {
                return Err(format!("Synthetic user is not verified: {email}").into());
            }
            (
                existing.get_object_id("_id")?.to_owned(),
                existing.get_str("email")?.to_owned(),
                role_from_document(&existing)?,
                security_version(&existing),
            )
        } else {
            let user_id = mongodb::bson::oid::ObjectId::new();
            users
                .insert_one(
                    doc! {
                        "_id": user_id,
                        "name": format!("TEDx Load Test User {index:03}"),
                        "email": &email,
                        "phone": format!("0900000{index:04}"),
                        "password": &password_hash,
                        "role": "attendee",
                        "isVerified": true,
                        "securityVersion": 0_i64,
                        "emailVerificationAttempts": 0_i64,
                        "passwordResetAttempts": 0_i64,
                        "createdAt": mongodb::bson::DateTime::from_chrono(now),
                        "updatedAt": mongodb::bson::DateTime::from_chrono(now),
                    },
                    None,
                )
                .await?;
            (user_id, email.clone(), UserRole::Attendee, 0)
        };
        let claims = Claims {
            sub: user_id.to_hex(),
            email: user_email.clone(),
            role,
            is_verified: true,
            security_version: user_security_version,
            exp: (Utc::now().timestamp() as u64 + expires_secs) as usize,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(jwt_secret.as_bytes()),
        )?;
        emails.push(email);
        tokens.push(token);
    }

    if let Some(parent) = Path::new(&token_file).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &token_file,
        serde_json::to_vec_pretty(&TokenFile { emails, tokens })?,
    )?;
    println!("Seeded {count} verified load-test users.");
    println!("Access tokens written to {token_file}.");
    println!("Remove these users after testing using the email prefix: {email_prefix}-");
    Ok(())
}
