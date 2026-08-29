use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::AppState;

const WINDOW: Duration = Duration::from_secs(60);
const MAX_REQUESTS_PER_WINDOW: usize = 120;

pub async fn request_rate_limit(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let key = request
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
        .unwrap_or("unknown")
        .to_owned();
    let now = Instant::now();
    let limited = {
        let mut timestamps = state.rate_limits.entry(key).or_default();
        while timestamps
            .front()
            .is_some_and(|timestamp| now.duration_since(*timestamp) >= WINDOW)
        {
            timestamps.pop_front();
        }
        if timestamps.len() >= MAX_REQUESTS_PER_WINDOW {
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
        response
            .headers_mut()
            .insert("retry-after", axum::http::HeaderValue::from_static("60"));
        return response;
    }
    next.run(request).await
}
