use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use base64::{engine::general_purpose, Engine as _};
use std::net::SocketAddr;

use crate::AppState;

// Temporary production-load-test switch. Set to true to restore rate limiting.
const RATE_LIMITING_ENABLED: bool = true;

const PUBLIC_WINDOW: Duration = Duration::from_secs(15 * 60);
const PUBLIC_MAX_REQUESTS: usize = 2_000;
const USER_WINDOW: Duration = Duration::from_secs(15 * 60);
const USER_MAX_REQUESTS: usize = 200;
const TICKET_WINDOW: Duration = Duration::from_secs(60);
const TICKET_MAX_REQUESTS: usize = 300;
const ADMIN_MAX_REQUESTS: usize = 300;
const PROFILE_MAX_REQUESTS: usize = 20;
const TICKET_INITIATE_MAX_REQUESTS: usize = 10;
const TICKET_VERIFY_OTP_MAX_REQUESTS: usize = 10;
const VOLUNTEER_APPLY_MAX_REQUESTS: usize = 5;
const VOLUNTEER_ME_MAX_REQUESTS: usize = 100;
const VOLUNTEER_CHANGE_ROLE_MAX_REQUESTS: usize = 3;
const AUTH_REFRESH_MAX_REQUESTS: usize = 50;
const SENSITIVE_WINDOW: Duration = Duration::from_secs(12 * 60 * 60);
const SENSITIVE_MAX_REQUESTS: usize = 15;

fn ticket_scan_path(path: &str) -> bool {
    let mut segments = path.split('/');
    segments.next() == Some("")
        && segments.next() == Some("api")
        && segments.next() == Some("tickets")
        && segments.next().is_some_and(|code| !code.is_empty())
        && segments
            .next()
            .is_some_and(|action| action == "verify" || action == "checkin")
        && segments.next().is_none()
}

fn sensitive_auth_path(path: &str) -> bool {
    matches!(
        path,
        "/api/auth/login"
            | "/api/auth/register"
            | "/api/auth/verify-email"
            | "/api/auth/resend-verification"
            | "/api/auth/forgot-password"
            | "/api/auth/reset-password"
    )
}

fn volunteer_me_path(path: &str) -> bool {
    path == "/api/volunteers/me"
}

fn dashboard_path(path: &str) -> bool {
    path == "/api/dashboard" || path.starts_with("/api/dashboard/")
}

fn admin_path(path: &str) -> bool {
    path == "/api/admin" || path.starts_with("/api/admin/")
}

fn webhook_path(path: &str) -> bool {
    path == "/api/tickets/webhook"
}

fn health_path(path: &str) -> bool {
    path == "/api/health"
}

fn authenticated_user_id(request: &Request<Body>) -> Option<String> {
    let jar = CookieJar::from_headers(request.headers());
    let token = jar.get("access_token").map(|cookie| cookie.value())?;
    let payload = token.split('.').nth(1)?;
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    serde_json::from_slice::<serde_json::Value>(&decoded)
        .ok()?
        .get("sub")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub async fn request_rate_limit(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !RATE_LIMITING_ENABLED {
        return next.run(request).await;
    }

    if request.method() == Method::OPTIONS {
        return next.run(request).await;
    }

    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
        })
        .map(str::to_owned)
        .or_else(|| {
            request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|connect_info| connect_info.0.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let path = request.uri().path();
    let method = request.method();
    let (bucket, window, max_requests, user_based) = if health_path(path) || webhook_path(path) {
        return next.run(request).await;
    } else if sensitive_auth_path(path) {
        (
            format!("sensitive-auth:{path}"),
            SENSITIVE_WINDOW,
            SENSITIVE_MAX_REQUESTS,
            false,
        )
    } else if method == Method::POST && path == "/api/auth/refresh" {
        (
            "auth-refresh".to_owned(),
            PUBLIC_WINDOW,
            AUTH_REFRESH_MAX_REQUESTS,
            false,
        )
    } else if method == Method::POST && path == "/api/tickets/initiate" {
        (
            "ticket-initiate".to_owned(),
            PUBLIC_WINDOW,
            TICKET_INITIATE_MAX_REQUESTS,
            false,
        )
    } else if method == Method::POST && path == "/api/tickets/verify-otp" {
        (
            "ticket-verify-otp".to_owned(),
            PUBLIC_WINDOW,
            TICKET_VERIFY_OTP_MAX_REQUESTS,
            false,
        )
    } else if method == Method::POST && path == "/api/volunteers/apply" {
        (
            "volunteer-apply".to_owned(),
            PUBLIC_WINDOW,
            VOLUNTEER_APPLY_MAX_REQUESTS,
            false,
        )
    } else if method == Method::GET && volunteer_me_path(path) {
        (
            "volunteer-me".to_owned(),
            PUBLIC_WINDOW,
            VOLUNTEER_ME_MAX_REQUESTS,
            false,
        )
    } else if method == Method::PATCH && path == "/api/volunteers/change-role" {
        (
            "volunteer-change-role".to_owned(),
            PUBLIC_WINDOW,
            VOLUNTEER_CHANGE_ROLE_MAX_REQUESTS,
            false,
        )
    } else if method == Method::PATCH && path == "/api/dashboard/profile" {
        (
            "dashboard-profile".to_owned(),
            USER_WINDOW,
            PROFILE_MAX_REQUESTS,
            true,
        )
    } else if ticket_scan_path(path) {
        (
            "ticket-scan".to_owned(),
            TICKET_WINDOW,
            TICKET_MAX_REQUESTS,
            true,
        )
    } else if path == "/api/tickets/mine" {
        (
            "ticket-mine".to_owned(),
            USER_WINDOW,
            USER_MAX_REQUESTS,
            true,
        )
    } else if admin_path(path) {
        ("admin".to_owned(), USER_WINDOW, ADMIN_MAX_REQUESTS, true)
    } else if dashboard_path(path) {
        ("dashboard".to_owned(), USER_WINDOW, USER_MAX_REQUESTS, true)
    } else {
        (
            "public".to_owned(),
            PUBLIC_WINDOW,
            PUBLIC_MAX_REQUESTS,
            false,
        )
    };
    let key = if user_based {
        authenticated_user_id(&request)
            .map(|user_id| format!("user:{bucket}:{user_id}"))
            .unwrap_or_else(|| format!("ip:{bucket}:{ip}"))
    } else {
        format!("ip:{bucket}:{ip}")
    };
    let now = Instant::now();
    let limited = {
        let mut timestamps = state.rate_limits.entry(key).or_default();
        while timestamps
            .front()
            .is_some_and(|timestamp| now.duration_since(*timestamp) >= window)
        {
            timestamps.pop_front();
        }
        if timestamps.len() >= max_requests {
            true
        } else {
            timestamps.push_back(now);
            false
        }
    };
    if limited {
        let mut response = Response::new(axum::body::Body::from(
            r#"{"success":false,"message":"Too many requests"}"#,
        ));
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        response.headers_mut().insert(
            "content-type",
            axum::http::HeaderValue::from_static("application/json"),
        );
        response.headers_mut().insert(
            "retry-after",
            axum::http::HeaderValue::from_str(&window.as_secs().to_string())
                .expect("duration is a valid header value"),
        );
        return response;
    }
    next.run(request).await
}
