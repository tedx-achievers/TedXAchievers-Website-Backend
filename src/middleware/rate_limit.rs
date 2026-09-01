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
use std::net::SocketAddr;

use crate::AppState;

const GLOBAL_WINDOW: Duration = Duration::from_secs(15 * 60);
const GLOBAL_MAX_REQUESTS: usize = 120;
const TICKET_WINDOW: Duration = Duration::from_secs(60);
const TICKET_MAX_REQUESTS: usize = 62;
const SENSITIVE_WINDOW: Duration = Duration::from_secs(12 * 60 * 60);
const SENSITIVE_MAX_REQUESTS: usize = 30;

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
            | "/api/auth/verify-email"
            | "/api/auth/resend-verification"
            | "/api/auth/forgot-password"
            | "/api/auth/reset-password"
    )
}

pub async fn request_rate_limit(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
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
    let (bucket, window, max_requests) = if ticket_scan_path(path) {
        (
            format!("ticket-scan:{path}"),
            TICKET_WINDOW,
            TICKET_MAX_REQUESTS,
        )
    } else if sensitive_auth_path(path) {
        (
            format!("sensitive-auth:{path}"),
            SENSITIVE_WINDOW,
            SENSITIVE_MAX_REQUESTS,
        )
    } else {
        ("global".to_owned(), GLOBAL_WINDOW, GLOBAL_MAX_REQUESTS)
    };
    let key = format!("{bucket}:{ip}");
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
