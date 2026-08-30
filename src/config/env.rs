use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub mongodb_uri: String,
    pub jwt_access_secret: String,
    pub jwt_access_secret_previous: Option<String>,
    pub jwt_refresh_secret: String,
    pub jwt_refresh_secret_previous: Option<String>,
    pub jwt_access_expires_secs: u64,
    pub jwt_refresh_expires_secs: u64,
    pub frontend_url: String,
    pub frontend_urls: Vec<String>,
    pub brevo_api_key: String,
    pub brevo_sender_email: String,
    pub brevo_sender_name: String,
    pub paystack_secret_key: String,
    pub paystack_webhook_secret: String,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        fn required(name: &str) -> String {
            env::var(name)
                .unwrap_or_else(|_| panic!("Required environment variable is missing: {name}"))
        }
        fn parsed<T: std::str::FromStr>(name: &str) -> T
        where
            T::Err: std::fmt::Display,
        {
            required(name)
                .parse()
                .unwrap_or_else(|error| panic!("Invalid {name}: {error}"))
        }
        fn optional(name: &str) -> Option<String> {
            env::var(name).ok().filter(|value| !value.trim().is_empty())
        }
        let frontend_urls = required("FRONTEND_URL")
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if frontend_urls.is_empty() {
            panic!("FRONTEND_URL must contain at least one origin");
        }
        for origin in &frontend_urls {
            let is_local =
                origin.starts_with("http://localhost") || origin.starts_with("http://127.0.0.1");
            if (!origin.starts_with("https://") && !is_local) || origin.ends_with('/') {
                panic!("Every FRONTEND_URL origin must be HTTPS without a trailing slash");
            }
        }
        let jwt_access_expires_secs = parsed("JWT_ACCESS_EXPIRES_SECS");
        if jwt_access_expires_secs == 0 || jwt_access_expires_secs > 900 {
            panic!("JWT_ACCESS_EXPIRES_SECS must be between 1 and 900 seconds");
        }
        Self {
            port: parsed("PORT"),
            mongodb_uri: required("MONGODB_URI"),
            jwt_access_secret: required("JWT_ACCESS_SECRET"),
            jwt_access_secret_previous: optional("JWT_ACCESS_SECRET_PREVIOUS"),
            jwt_refresh_secret: required("JWT_REFRESH_SECRET"),
            jwt_refresh_secret_previous: optional("JWT_REFRESH_SECRET_PREVIOUS"),
            jwt_access_expires_secs,
            jwt_refresh_expires_secs: parsed("JWT_REFRESH_EXPIRES_SECS"),
            frontend_url: frontend_urls[0].clone(),
            frontend_urls,
            brevo_api_key: required("BREVO_API_KEY"),
            brevo_sender_email: required("BREVO_SENDER_EMAIL"),
            brevo_sender_name: required("BREVO_SENDER_NAME"),
            paystack_secret_key: required("PAYSTACK_SECRET_KEY"),
            paystack_webhook_secret: required("PAYSTACK_WEBHOOK_SECRET"),
        }
    }
}
