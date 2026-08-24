use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use meting_core::Problem;

/// Optional Bearer auth: if METING_TOKEN is set, require `Authorization: Bearer <token>` for sensitive types.
/// Sensitive = url / pic / lrc / batch ; public = search / song / playlist / album / artist / health
fn is_sensitive_path(path: &str) -> bool {
    path.contains("/url") || path.contains("/pic") || path.contains("/lyric") || path.contains("/batch")
}

pub async fn auth_middleware(req: Request, next: Next) -> Response {
    let token = std::env::var("METING_TOKEN").ok();
    if let Some(expected) = token {
        if expected.is_empty() {
            return next.run(req).await;
        }
        let path = req.uri().path().to_string();
        if !is_sensitive_path(&path) {
            return next.run(req).await;
        }
        let headers: &HeaderMap = req.headers();
        let auth = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let expected_header = format!("Bearer {expected}");
        if auth != expected_header {
            // also accept ?token= query for legacy? we check query via uri
            let query = req.uri().query().unwrap_or("");
            let has_token_query = query.contains(&format!("token={expected}"))
                || query.contains(&format!("auth={expected}"));
            if !has_token_query {
                let body = Problem {
                    typ: "https://api.meting.rs/errors/unauthorized".into(),
                    title: "Unauthorized".into(),
                    status: 401,
                    code: "UNAUTHORIZED".into(),
                    detail: Some("missing or invalid Bearer token".into()),
                    instance: Some(path),
                };
                return (StatusCode::UNAUTHORIZED, Json(body)).into_response();
            }
        }
    }
    next.run(req).await
}
