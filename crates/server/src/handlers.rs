use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use meting_core::{ApiResponse, Meta, Platform, Problem, parse_platform};
use serde::Deserialize;

use crate::provider;

// ---------- helpers ----------

fn problem(status: u16, code: &str, title: &str, detail: Option<String>, instance: Option<String>) -> Response {
    let body = Problem {
        typ: format!("https://api.meting.rs/errors/{}", code.to_ascii_lowercase()),
        title: title.into(),
        status,
        code: code.into(),
        detail,
        instance,
    };
    (StatusCode::from_u16(status).unwrap(), Json(body)).into_response()
}

fn cache_headers(headers: &mut HeaderMap, kind: &str) {
    match kind {
        "url" => { headers.insert("cache-control", "private, max-age=600".parse().unwrap()); }
        "meta" => { headers.insert("cache-control", "public, s-maxage=3600, stale-while-revalidate=600".parse().unwrap()); }
        _ => {}
    }
    headers.insert("x-meting-api-version", "v1".parse().unwrap());
}

fn require_netease(platform: Platform) -> Option<Response> {
    if platform != Platform::Netease {
        return Some(problem(400, "UNSUPPORTED_PLATFORM", "platform not yet implemented", Some(format!("platform={} only netease wired (tencent/kugou stubs)", platform.as_str())), None));
    }
    None
}

// ---------- health / docs ----------

pub async fn health() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    cache_headers(&mut headers, "meta");
    let body = ApiResponse::ok(serde_json::json!({"status":"ok","version": env!("CARGO_PKG_VERSION")}));
    (headers, Json(body))
}

pub async fn openapi() -> impl IntoResponse {
    let data = std::fs::read_to_string("openapi.json").unwrap_or_else(|_| r#"{"openapi":"3.1.0","info":{"title":"meting-api-rs","version":"0.1.0"}}"#.into());
    let v: serde_json::Value = serde_json::from_str(&data).unwrap_or(serde_json::json!({}));
    Json(v)
}

pub async fn docs() -> impl IntoResponse {
    Redirect::temporary("https://scalar.com")
}

// ---------- query structs ----------

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub platform: Option<String>,
    pub server: Option<String>,
    pub q: Option<String>,
    pub keyword: Option<String>,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub typ: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    pub page: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CommonQuery {
    pub platform: Option<String>,
    pub server: Option<String>,
    pub br: Option<u32>,
    pub size: Option<u32>,
    pub redirect: Option<String>,
}
fn is_redirect(q: &Option<String>) -> bool {
    matches!(q.as_deref(), Some("1") | Some("true") | Some("yes"))
}

#[derive(Debug, Deserialize)]
pub struct BatchBody {
    pub platform: Option<String>,
    pub server: Option<String>,
    pub ids: Vec<String>,
}

// ---------- /v1/search ----------

pub async fn search(Query(q): Query<SearchQuery>) -> Response {
    let platform_raw = q.platform.or(q.server);
    let platform = parse_platform(platform_raw.as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    let keyword = q.q.or(q.keyword).or(q.id).unwrap_or_default();
    if keyword.is_empty() {
        return problem(400, "BAD_REQUEST", "missing query", Some("q is required".into()), Some("/v1/search".into()));
    }
    let limit = q.limit.unwrap_or(20).min(50);
    let offset = if let Some(cur) = q.cursor { cur.parse::<u32>().unwrap_or(0) } else if let Some(page) = q.page { page.saturating_sub(1) * limit } else { 0 };

    match provider::search(&keyword, limit, offset).await {
        Ok((songs, total)) => {
            let mut headers = HeaderMap::new();
            cache_headers(&mut headers, "meta");
            let has_more = (offset as usize + songs.len()) < total;
            let next_cursor = if has_more { Some((offset + limit).to_string()) } else { None };
            let meta = Meta { total: Some(total), cursor: next_cursor, has_more: Some(has_more) };
            (headers, Json(ApiResponse::ok_with_meta(songs, meta))).into_response()
        }
        Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), Some("/v1/search".into())),
    }
}

// ---------- /v1/songs/:id ----------

pub async fn song(Path(id): Path<String>, Query(q): Query<CommonQuery>) -> Response {
    let platform = parse_platform(q.platform.or(q.server).as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    if id.is_empty() { return problem(400, "BAD_REQUEST", "missing id", None, Some(format!("/v1/songs/{id}"))); }
    match provider::song_detail(&[id.clone()]).await {
        Ok(mut songs) => {
            let mut headers = HeaderMap::new();
            cache_headers(&mut headers, "meta");
            if let Some(s) = songs.pop() { (headers, Json(ApiResponse::ok(s))).into_response() }
            else { problem(404, "NOT_FOUND", "song not found", Some(format!("id={id}")), Some("/v1/songs/:id".into())) }
        }
        Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None),
    }
}

pub async fn song_url(Path(id): Path<String>, Query(q): Query<CommonQuery>) -> Response {
    let platform = parse_platform(q.platform.or(q.server).as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    let br = q.br.unwrap_or(320000);
    let redirect = is_redirect(&q.redirect);
    match provider::url_info_fallback(&id, br).await {
        Ok(info) => {
            if info.url.is_empty() {
                // 上游未下发播放地址（网易云对数据中心 IP 风控 / 无版权 / 需 VIP 常见；code 仍为 200 但 url 为空）
                // 明确报错而非静默 200 空 url，避免前端把 JSON 当音频源加载且无从诊断
                return problem(
                    502,
                    "UPSTREAM_EMPTY_URL",
                    "upstream returned no playable url",
                    Some(format!("id={id} br={br} (netease 风控/无版权/无 VIP 时常见)")),
                    Some("/v1/songs/:id/url".into()),
                );
            }
            if redirect && !info.url.is_empty() {
                return Redirect::temporary(&info.url).into_response();
            }
            let mut headers = HeaderMap::new();
            cache_headers(&mut headers, "url");
            (headers, Json(ApiResponse::ok(info))).into_response()
        }
        Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None),
    }
}

pub async fn song_lyric(Path(id): Path<String>, Query(q): Query<CommonQuery>) -> Response {
    let platform = parse_platform(q.platform.or(q.server).as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    let _ = q;
    match provider::lyric_info(&id).await {
        Ok(info) => {
            let mut headers = HeaderMap::new();
            cache_headers(&mut headers, "meta");
            (headers, Json(ApiResponse::ok(info))).into_response()
        }
        Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None),
    }
}

pub async fn song_pic(Path(id): Path<String>, Query(q): Query<CommonQuery>) -> Response {
    let platform = parse_platform(q.platform.or(q.server).as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    let size = q.size.unwrap_or(500).clamp(100, 1000);
    let redirect = is_redirect(&q.redirect);
    match provider::pic_info(&id, size).await {
        Ok(info) => {
            if redirect && !info.url.is_empty() {
                return Redirect::temporary(&info.url).into_response();
            }
            let mut headers = HeaderMap::new();
            cache_headers(&mut headers, "meta");
            (headers, Json(ApiResponse::ok(info))).into_response()
        }
        Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None),
    }
}

pub async fn playlist(Path(id): Path<String>, Query(q): Query<CommonQuery>) -> Response {
    let platform = parse_platform(q.platform.or(q.server).as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    let _ = q;
    match provider::playlist(&id).await {
        Ok(songs) => {
            let mut headers = HeaderMap::new();
            cache_headers(&mut headers, "meta");
            (headers, Json(ApiResponse::ok(serde_json::json!({"id": id, "platform": platform.as_str(), "songs": songs})))).into_response()
        }
        Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None),
    }
}

pub async fn album(Path(id): Path<String>, Query(q): Query<CommonQuery>) -> Response {
    let platform = parse_platform(q.platform.or(q.server).as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    // album not yet wired, reuse song_detail as placeholder
    let _ = id;
    let _ = q;
    problem(501, "NOT_IMPLEMENTED", "album not yet implemented", None, Some("/v1/albums/:id".into()))
}

pub async fn artist(Path(id): Path<String>, Query(q): Query<CommonQuery>) -> Response {
    let platform = parse_platform(q.platform.or(q.server).as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    let _ = id; let _ = q;
    problem(501, "NOT_IMPLEMENTED", "artist not yet implemented", None, Some("/v1/artists/:id".into()))
}

pub async fn batch(Json(body): Json<BatchBody>) -> Response {
    let platform = parse_platform(body.platform.or(body.server).as_deref());
    if let Some(r) = require_netease(platform) { return r; }
    if body.ids.is_empty() { return problem(400, "BAD_REQUEST", "ids required", None, Some("/v1/songs/batch".into())); }
    let ids = body.ids.iter().take(100).cloned().collect::<Vec<_>>();
    match provider::song_detail(&ids).await {
        Ok(songs) => {
            let mut headers = HeaderMap::new();
            cache_headers(&mut headers, "meta");
            (headers, Json(ApiResponse::ok(songs))).into_response()
        }
        Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None),
    }
}

// ---------- legacy /api ----------

#[derive(Debug, Deserialize)]
pub struct LegacyQuery {
    pub server: Option<String>,
    pub platform: Option<String>,
    #[serde(rename = "type")]
    pub typ: Option<String>,
    pub id: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub br: Option<u32>,
    pub size: Option<u32>,
}

fn legacy_song_json(s: &meting_core::Song) -> serde_json::Value {
    serde_json::json!({
        "name": s.name,
        "artist": s.artist.join(" / "),
        "url": format!("/api?server=netease&type=url&id={}", s.id),
        "pic": format!("/api?server=netease&type=pic&id={}", s.id),
        "lrc": format!("/api?server=netease&type=lrc&id={}", s.id),
        "id": s.id,
        "album": s.album,
    })
}

pub async fn legacy_api(Query(q): Query<LegacyQuery>, headers: HeaderMap) -> Response {
    let platform = parse_platform(q.server.or(q.platform).as_deref());
    let typ = q.typ.unwrap_or_else(|| "song".into()).to_ascii_lowercase();
    let id = q.id.clone().unwrap_or_default();
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert("deprecation", "true".parse().unwrap());
    resp_headers.insert("sunset", "Wed, 01 Jan 2027 00:00:00 GMT".parse().unwrap());
    resp_headers.insert("link", r#"<https://api.meting.rs/v1>; rel="successor-version""#.parse().unwrap());
    resp_headers.insert("x-meting-legacy", "true".parse().unwrap());

    let _ = headers; // legacy url/pic always 302, no need to inspect Accept

    // Only netease wired for legacy too
    if platform != Platform::Netease {
        return (resp_headers, problem(400, "UNSUPPORTED_PLATFORM", "only netease wired", None, Some("/api".into()))).into_response();
    }

    let result: Response = match typ.as_str() {
        "search" => {
            let keyword = id.clone();
            if keyword.is_empty() { return (resp_headers, problem(400, "BAD_REQUEST", "missing id for search", None, Some("/api".into()))).into_response(); }
            let limit = q.limit.unwrap_or(20).min(50);
            let offset = q.page.map(|p| p.saturating_sub(1)*limit).unwrap_or(0);
            match provider::search(&keyword, limit, offset).await {
                Ok((songs, _)) => {
                    let legacy: Vec<serde_json::Value> = songs.iter().map(legacy_song_json).collect();
                    let mut h = resp_headers.clone();
                    cache_headers(&mut h, "meta");
                    (h, Json(serde_json::to_value(legacy).unwrap())).into_response()
                }
                Err(e) => (resp_headers.clone(), problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), Some("/api".into()))).into_response(),
            }
        }
        "song" => {
            match provider::song_detail(&[id.clone()]).await {
                Ok(songs) => {
                    let legacy: Vec<serde_json::Value> = songs.iter().map(legacy_song_json).collect();
                    let mut h = resp_headers.clone();
                    cache_headers(&mut h, "meta");
                    (h, Json(serde_json::to_value(legacy).unwrap())).into_response()
                }
                Err(e) => (resp_headers.clone(), problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None)).into_response(),
            }
        }
        "playlist" => {
            match provider::playlist(&id).await {
                Ok(songs) => {
                    let legacy: Vec<serde_json::Value> = songs.iter().map(legacy_song_json).collect();
                    let mut h = resp_headers.clone();
                    cache_headers(&mut h, "meta");
                    (h, Json(serde_json::to_value(legacy).unwrap())).into_response()
                }
                Err(e) => (resp_headers.clone(), problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None)).into_response(),
            }
        }
        "url" => {
            let br = q.br.unwrap_or(320000);
            match provider::url_info(&id, br).await {
                Ok(info) => {
                    if info.url.is_empty() {
                        let mut h = resp_headers.clone();
                        cache_headers(&mut h, "url");
                        (h, Json(serde_json::json!({"url": info.url, "br": info.br, "size": info.size}))).into_response()
                    } else {
                        // legacy always 302 for audio src
                        return (resp_headers.clone(), Redirect::temporary(&info.url)).into_response();
                    }
                }
                Err(e) => (resp_headers.clone(), problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None)).into_response(),
            }
        }
        "pic" => {
            let size = q.size.unwrap_or(500);
            match provider::pic_info(&id, size).await {
                Ok(info) => {
                    if info.url.is_empty() {
                        let mut h = resp_headers.clone();
                        cache_headers(&mut h, "meta");
                        (h, Json(serde_json::json!({"url": info.url}))).into_response()
                    } else {
                        return (resp_headers.clone(), Redirect::temporary(&info.url)).into_response();
                    }
                }
                Err(e) => (resp_headers.clone(), problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None)).into_response(),
            }
        }
        "lrc" | "lyric" => {
            match provider::lyric_info(&id).await {
                Ok(info) => {
                    // legacy returns plain text lrc
                    let mut h = resp_headers.clone();
                    h.insert("content-type", "text/plain; charset=utf-8".parse().unwrap());
                    cache_headers(&mut h, "meta");
                    (h, info.lrc).into_response()
                }
                Err(e) => (resp_headers.clone(), problem(502, "UPSTREAM_ERROR", "upstream error", Some(e), None)).into_response(),
            }
        }
        _ => return (resp_headers, problem(400, "BAD_REQUEST", "unsupported type", Some(format!("type={typ}")), Some("/api".into()))).into_response(),
    };
    result
}
