use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub mongodb_uri: String,
    pub jwt_access_secret: String,
    pub jwt_refresh_secret: String,
    pub jwt_access_expires_secs: u64,
    pub jwt_refresh_expires_secs: u64,
    pub frontend_url: String,
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
        Self {
            port: parsed("PORT"),
            mongodb_uri: required("MONGODB_URI"),
            jwt_access_secret: required("JWT_ACCESS_SECRET"),
            jwt_refresh_secret: required("JWT_REFRESH_SECRET"),
            jwt_access_expires_secs: parsed("JWT_ACCESS_EXPIRES_SECS"),
            jwt_refresh_expires_secs: parsed("JWT_REFRESH_EXPIRES_SECS"),
            frontend_url: required("FRONTEND_URL"),
            brevo_api_key: required("BREVO_API_KEY"),
            brevo_sender_email: required("BREVO_SENDER_EMAIL"),
            brevo_sender_name: required("BREVO_SENDER_NAME"),
            paystack_secret_key: required("PAYSTACK_SECRET_KEY"),
            paystack_webhook_secret: required("PAYSTACK_WEBHOOK_SECRET"),
        }
    }
}
