use worker::*;
use meting_core::{ApiResponse, Meta, Platform, parse_platform, Problem, mapping, provider::NeteaseProvider};
use serde::Deserialize;

fn cors_headers() -> Headers {
    let h = Headers::new();
    h.set("Access-Control-Allow-Origin", "*").ok();
    h.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS").ok();
    h.set("Access-Control-Allow-Headers", "Content-Type, Authorization").ok();
    h
}

fn json_resp<T: serde::Serialize>(status: u16, body: &T, cache: &str) -> Result<Response> {
    let headers = cors_headers();
    headers.set("Content-Type", "application/json; charset=utf-8").ok();
    headers.set("Cache-Control", cache).ok();
    headers.set("X-Meting-Api-Version", "v1").ok();
    Ok(Response::from_json(body)?.with_status(status).with_headers(headers))
}

fn problem(status: u16, code: &str, title: &str, detail: Option<String>) -> Result<Response> {
    let body = Problem {
        typ: format!("https://api.meting.rs/errors/{}", code.to_ascii_lowercase()),
        title: title.into(),
        status,
        code: code.into(),
        detail,
        instance: None,
    };
    json_resp(status, &body, "no-store")
}

async fn weapi_post(url: &str, body: &str) -> Result<serde_json::Value, String> {
    let headers = Headers::new();
    headers.set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36").ok();
    headers.set("Referer", "https://music.163.com/").ok();
    headers.set("Content-Type", "application/x-www-form-urlencoded").ok();
    headers.set("Cookie", "os=pc; appver=8.9.75; osver=Microsoft-Windows-10-Professional-build-19045-64bit;").ok();
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));
    let req = Request::new_with_init(url, &init).map_err(|e| e.to_string())?;
    let mut resp = Fetch::Request(req).send().await.map_err(|e| e.to_string())?;
    let status = resp.status_code();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("upstream {status}: {}", &text[..text.len().min(600)]));
    }
    serde_json::from_str(&text).map_err(|e| format!("invalid json {e}: {}", &text[..text.len().min(600)]))
}
async fn eapi_post(url: &str, body: &str) -> Result<serde_json::Value, String> { weapi_post(url, body).await }

/// eapi + Android 网易云客户端头（meting.js 同款姿势：随机 deviceId/requestId + NeteaseMusic UA）
async fn eapi_post_android(url: &str, body: &str) -> Result<serde_json::Value, String> {
    let mut dev = [0u8; 16];
    let _ = getrandom::getrandom(&mut dev);
    let device_id: String = dev.iter().map(|x| format!("{x:02X}")).collect();
    let mut rid = [0u8; 8];
    let _ = getrandom::getrandom(&mut rid);
    let ts = u64::from_le_bytes(rid);
    let request_id = format!("{ts}_{:04}", (ts >> 32) % 1000);
    let headers = Headers::new();
    headers.set("User-Agent", "Mozilla/5.0 (Linux; Android 11; M2007J3SC Build/RKQ1.200826.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/77.0.3865.120 MQQBrowser/6.2 TBS/045714 Mobile Safari/537.36 NeteaseMusic/8.7.01").ok();
    headers.set("Referer", "music.163.com").ok();
    headers.set("Content-Type", "application/x-www-form-urlencoded").ok();
    headers.set("Cookie", &format!("osver=android; appver=8.7.01; os=android; deviceId={device_id}; channel=netease; requestId={request_id}; __remember_me=true")).ok();
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));
    let req = Request::new_with_init(url, &init).map_err(|e| e.to_string())?;
    let mut resp = Fetch::Request(req).send().await.map_err(|e| e.to_string())?;
    let status = resp.status_code();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("upstream {status}: {}", &text[..text.len().min(600)]));
    }
    serde_json::from_str(&text).map_err(|e| format!("invalid json {e}: {}", &text[..text.len().min(600)]))
}

/// 三级 url fallback（weapi PC → eapi PC → eapi Android）
/// 任一拿到非空 url 即返回；全空返回 Err(最后一级错误详情)
async fn fetch_url_with_fallback(provider: &NeteaseProvider, id: &str, br: u32) -> Result<meting_core::UrlInfo, String> {
    let mut errors: Vec<String> = Vec::new();

    // 1) weapi PC（现状；住宅/国内 IP 下通常直接可用）
    let body = provider.url_body(&[id.to_string()], br);
    let endpoint = provider.endpoint("/weapi/song/enhance/player/url");
    match weapi_post(&endpoint, &body).await {
        Ok(v) => {
            let info = mapping::map_url(&v, id);
            if !info.url.is_empty() {
                return Ok(info);
            }
            errors.push(format!("weapi empty: {}", &v.to_string()[..v.to_string().len().min(300)]));
        }
        Err(e) => errors.push(e),
    }

    // 2) eapi PC（meting.js 端点族；数据中心 IP 下可能放行）
    let (ebody, eurl) = provider.url_eapi_body(&[id.to_string()], br);
    match eapi_post(&eurl, &ebody).await {
        Ok(v) => {
            let info = mapping::map_url(&v, id);
            if !info.url.is_empty() {
                return Ok(info);
            }
            errors.push(format!("eapi(PC) empty: {}", &v.to_string()[..v.to_string().len().min(300)]));
        }
        Err(e) => errors.push(e),
    }

    // 3) eapi + Android 客户端头（meting.js 完整姿势）
    match eapi_post_android(&eurl, &ebody).await {
        Ok(v) => {
            let info = mapping::map_url(&v, id);
            if !info.url.is_empty() {
                return Ok(info);
            }
            errors.push(format!("eapi(android) empty: {}", &v.to_string()[..v.to_string().len().min(300)]));
        }
        Err(e) => errors.push(e),
    }

    Err(errors.join(" | "))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    platform: Option<String>,
    server: Option<String>,
    q: Option<String>,
    keyword: Option<String>,
    id: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
    page: Option<u32>,
}

fn require_netease(platform: Platform) -> Option<Result<Response>> {
    if platform != Platform::Netease {
        return Some(problem(400, "UNSUPPORTED_PLATFORM", "only netease wired", Some(format!("platform={} stub", platform.as_str()))));
    }
    None
}

#[event(fetch)]
pub async fn main(req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    if req.method() == Method::Options {
        return Ok(Response::empty()?.with_status(204).with_headers(cors_headers()));
    }

    let url = req.url()?;
    let path = url.path().to_string();
    let headers = req.headers();

    if let Ok(token) = _env.secret("METING_TOKEN") {
        let expected = token.to_string();
        if !expected.is_empty() && (path.contains("/url") || path.contains("/pic") || path.contains("/lyric") || path.contains("/batch")) {
            let auth = headers.get("Authorization").ok().flatten().unwrap_or_default();
            if auth != format!("Bearer {expected}") {
                let has_q_token = url.query_pairs().any(|(_, v)| v == expected);
                if !has_q_token {
                    return problem(401, "UNAUTHORIZED", "Unauthorized", Some("missing Bearer token".into()));
                }
            }
        }
    }

    match (req.method(), path.as_str()) {
        (Method::Get, "/v1/health") => {
            json_resp(200, &ApiResponse::ok(serde_json::json!({"status":"ok","runtime":"workers-wasm-weapi"})), "public, s-maxage=60")
        }
        (Method::Get, p) if p.starts_with("/v1/search") => {
            let mut q = SearchQuery{platform:None,server:None,q:None,keyword:None,id:None,limit:None,cursor:None,page:None};
            for (k,v) in url.query_pairs() {
                match k.as_ref() {
                    "platform" => q.platform = Some(v.into_owned()),
                    "server" => q.server = Some(v.into_owned()),
                    "q" | "keyword" => q.q = Some(v.into_owned()),
                    "id" => q.id = Some(v.into_owned()),
                    "limit" => q.limit = v.parse().ok(),
                    "cursor" => q.cursor = Some(v.into_owned()),
                    "page" => q.page = v.parse().ok(),
                    _ => {}
                }
            }
            let platform = parse_platform(q.platform.or(q.server).as_deref());
            if let Some(r) = require_netease(platform) { return r; }
            let keyword = q.q.or(q.keyword).or(q.id).unwrap_or_default();
            if keyword.is_empty() { return problem(400, "BAD_REQUEST", "missing q", None); }
            let limit = q.limit.unwrap_or(20).min(50);
            let offset = if let Some(cur) = q.cursor { cur.parse::<u32>().unwrap_or(0) } else if let Some(page) = q.page { page.saturating_sub(1)*limit } else { 0 };
            let provider = NeteaseProvider::default();
            let mut last_err = String::new();
            let mut result: Option<(Vec<meting_core::Song>, usize)> = None;
            for (body, url) in [provider.search_eapi_body(&keyword, limit, offset), provider.search_eapi_keywords_body(&keyword, limit, offset)] {
                match eapi_post(&url, &body).await {
                    Ok(v) => {
                        if v.get("code").and_then(|x| x.as_i64()) == Some(200) || v.get("result").is_some() {
                            let (songs, total) = mapping::map_search_result(&v);
                            if !songs.is_empty() || total>0 || v.get("code").and_then(|x| x.as_i64())==Some(200) {
                                result = Some((songs, total));
                                break;
                            }
                        }
                        last_err = v.to_string();
                    }
                    Err(e) => last_err = e,
                }
            }
            if let Some((songs, total)) = result {
                let has_more = (offset as usize + songs.len()) < total;
                let next_cursor = if has_more { Some((offset+limit).to_string()) } else { None };
                let meta = Meta{ total: Some(total), cursor: next_cursor, has_more: Some(has_more) };
                return json_resp(200, &ApiResponse::ok_with_meta(songs, meta), "public, s-maxage=3600, stale-while-revalidate=600");
            }
            // fallback weapi
            let body = provider.search_body(&keyword, limit, offset);
            let endpoint = provider.endpoint("/weapi/cloudsearch/get/web");
            match weapi_post(&endpoint, &body).await {
                Ok(v) => {
                    let (songs, total) = mapping::map_search_result(&v);
                    let has_more = (offset as usize + songs.len()) < total;
                    let next_cursor = if has_more { Some((offset+limit).to_string()) } else { None };
                    let meta = Meta{ total: Some(total), cursor: next_cursor, has_more: Some(has_more) };
                    json_resp(200, &ApiResponse::ok_with_meta(songs, meta), "public, s-maxage=3600, stale-while-revalidate=600")
                }
                Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(format!("eapi:{last_err} weapi:{e}"))),
            }
        }
        (Method::Get, p) if p.starts_with("/v1/songs/") && p.ends_with("/url") => {
            let id = p.trim_start_matches("/v1/songs/").trim_end_matches("/url").to_string();
            let platform = parse_platform(url.query_pairs().find(|(k,_)| k=="platform"||k=="server").map(|(_,v)| v.into_owned()).as_deref());
            if let Some(r) = require_netease(platform) { return r; }
            let br: u32 = url.query_pairs().find(|(k,_)| k=="br").and_then(|(_,v)| v.parse().ok()).unwrap_or(320000);
            let redirect = url.query_pairs().any(|(k,v)| k=="redirect" && v=="1");
            let provider = NeteaseProvider::default();
            match fetch_url_with_fallback(&provider, &id, br).await {
                Ok(info) => {
                    if info.url.is_empty() {
                        // 上游未下发播放地址（网易云对数据中心 IP 风控 / 无版权 / 需 VIP 常见；code 仍为 200 但 url 为空）
                        // 明确报错而非静默 200 空 url（避免前端把 JSON 当音频源加载失败且无从诊断）
                        return problem(502, "UPSTREAM_EMPTY_URL", "upstream returned no playable url", Some(format!("id={id} br={br}")));
                    }
                    if redirect && !info.url.is_empty() {
                        let h = cors_headers();
                        h.set("Location", &info.url).ok();
                        return Ok(Response::empty()?.with_status(302).with_headers(h));
                    }
                    json_resp(200, &ApiResponse::ok(info), "private, max-age=600")
                }
                Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
            }
        }
        (Method::Get, p) if p.starts_with("/v1/songs/") && p.ends_with("/lyric") => {
            let id = p.trim_start_matches("/v1/songs/").trim_end_matches("/lyric").to_string();
            let platform = parse_platform(url.query_pairs().find(|(k,_)| k=="platform"||k=="server").map(|(_,v)| v.into_owned()).as_deref());
            if let Some(r) = require_netease(platform) { return r; }
            let provider = NeteaseProvider::default();
            let body = provider.lyric_body(&id);
            let endpoint = provider.endpoint("/weapi/song/lyric");
            match weapi_post(&endpoint, &body).await {
                Ok(v) => {
                    let info = mapping::map_lyric(&v);
                    json_resp(200, &ApiResponse::ok(info), "public, s-maxage=3600, stale-while-revalidate=600")
                }
                Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
            }
        }
        (Method::Get, p) if p.starts_with("/v1/songs/") && p.ends_with("/pic") => {
            let id = p.trim_start_matches("/v1/songs/").trim_end_matches("/pic").to_string();
            let platform = parse_platform(url.query_pairs().find(|(k,_)| k=="platform"||k=="server").map(|(_,v)| v.into_owned()).as_deref());
            if let Some(r) = require_netease(platform) { return r; }
            let size: u32 = url.query_pairs().find(|(k,_)| k=="size").and_then(|(_,v)| v.parse().ok()).unwrap_or(500);
            let redirect = url.query_pairs().any(|(k,v)| k=="redirect" && v=="1");
            let provider = NeteaseProvider::default();
            let body = provider.song_detail_body(&[id.clone()]);
            let endpoint = provider.endpoint("/weapi/v3/song/detail");
            match weapi_post(&endpoint, &body).await {
                Ok(v) => {
                    let info = mapping::map_pic(&v, size.clamp(100,1000));
                    if redirect && !info.url.is_empty() {
                        let h = cors_headers();
                        h.set("Location", &info.url).ok();
                        return Ok(Response::empty()?.with_status(302).with_headers(h));
                    }
                    json_resp(200, &ApiResponse::ok(info), "public, s-maxage=3600, stale-while-revalidate=600")
                }
                Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
            }
        }
        (Method::Get, p) if p.starts_with("/v1/songs/") && !p.contains("/batch") => {
            // /v1/songs/:id
            let id = p.trim_start_matches("/v1/songs/").to_string();
            if id.contains('/') { return problem(404, "NOT_FOUND", "not found", None); }
            let platform = parse_platform(url.query_pairs().find(|(k,_)| k=="platform"||k=="server").map(|(_,v)| v.into_owned()).as_deref());
            if let Some(r) = require_netease(platform) { return r; }
            let provider = NeteaseProvider::default();
            let body = provider.song_detail_body(&[id.clone()]);
            let endpoint = provider.endpoint("/weapi/v3/song/detail");
            match weapi_post(&endpoint, &body).await {
                Ok(v) => {
                    let mut songs = mapping::map_song_detail(&v);
                    if let Some(s) = songs.pop() {
                        json_resp(200, &ApiResponse::ok(s), "public, s-maxage=3600, stale-while-revalidate=600")
                    } else {
                        problem(404, "NOT_FOUND", "song not found", Some(format!("id={id}")))
                    }
                }
                Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
            }
        }
        (Method::Get, p) if p.starts_with("/v1/playlists/") => {
            let id = p.trim_start_matches("/v1/playlists/").to_string();
            let platform = parse_platform(url.query_pairs().find(|(k,_)| k=="platform"||k=="server").map(|(_,v)| v.into_owned()).as_deref());
            if let Some(r) = require_netease(platform) { return r; }
            let provider = NeteaseProvider::default();
            let body = provider.playlist_body(&id);
            let endpoint = provider.endpoint("/weapi/v3/playlist/detail");
            match weapi_post(&endpoint, &body).await {
                Ok(v) => {
                    let songs = mapping::map_playlist_tracks(&v);
                    json_resp(200, &ApiResponse::ok(serde_json::json!({"id": id, "platform": platform.as_str(), "songs": songs})), "public, s-maxage=3600, stale-while-revalidate=600")
                }
                Err(e) => problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
            }
        }
        (Method::Get, p) if p == "/api" || p == "/meting" || p == "/" => {
            // legacy: delegate to modern with deprecation headers
            let typ = url.query_pairs().find(|(k,_)| k=="type").map(|(_,v)| v.into_owned()).unwrap_or_else(|| "song".into());
            let id = url.query_pairs().find(|(k,_)| k=="id").map(|(_,v)| v.into_owned()).unwrap_or_default();
            let platform = parse_platform(url.query_pairs().find(|(k,_)| k=="server"||k=="platform").map(|(_,v)| v.into_owned()).as_deref());
            if platform != Platform::Netease {
                return problem(400, "UNSUPPORTED_PLATFORM", "only netease wired", None);
            }
            let provider = NeteaseProvider::default();
            let (json_val, cache) = match typ.to_ascii_lowercase().as_str() {
                "search" => {
                    let body = provider.search_body(&id, 20, 0);
                    let endpoint = provider.endpoint("/weapi/cloudsearch/get/web");
                    match weapi_post(&endpoint, &body).await {
                        Ok(v) => {
                            let (songs, _) = mapping::map_search_result(&v);
                            (serde_json::to_value(songs).unwrap(), "public, s-maxage=3600, stale-while-revalidate=600")
                        }
                        Err(e) => return problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
                    }
                }
                "song" => {
                    let body = provider.song_detail_body(&[id.clone()]);
                    let endpoint = provider.endpoint("/weapi/v3/song/detail");
                    match weapi_post(&endpoint, &body).await {
                        Ok(v) => (serde_json::to_value(mapping::map_song_detail(&v)).unwrap(), "public, s-maxage=3600, stale-while-revalidate=600"),
                        Err(e) => return problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
                    }
                }
                "playlist" => {
                    let body = provider.playlist_body(&id);
                    let endpoint = provider.endpoint("/weapi/v3/playlist/detail");
                    match weapi_post(&endpoint, &body).await {
                        Ok(v) => (serde_json::to_value(mapping::map_playlist_tracks(&v)).unwrap(), "public, s-maxage=3600, stale-while-revalidate=600"),
                        Err(e) => return problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
                    }
                }
                "url" => {
                    let body = provider.url_body(&[id.clone()], 320000);
                    let endpoint = provider.endpoint("/weapi/song/enhance/player/url");
                    match weapi_post(&endpoint, &body).await {
                        Ok(v) => {
                            let info = mapping::map_url(&v, &id);
                            (serde_json::json!({"url": info.url, "br": info.br}), "private, max-age=600")
                        }
                        Err(e) => return problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
                    }
                }
                "pic" => {
                    let body = provider.song_detail_body(&[id.clone()]);
                    let endpoint = provider.endpoint("/weapi/v3/song/detail");
                    match weapi_post(&endpoint, &body).await {
                        Ok(v) => {
                            let info = mapping::map_pic(&v, 500);
                            (serde_json::json!({"url": info.url}), "public, s-maxage=3600, stale-while-revalidate=600")
                        }
                        Err(e) => return problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
                    }
                }
                "lrc" | "lyric" => {
                    let body = provider.lyric_body(&id);
                    let endpoint = provider.endpoint("/weapi/song/lyric");
                    match weapi_post(&endpoint, &body).await {
                        Ok(v) => {
                            let info = mapping::map_lyric(&v);
                            let h = cors_headers();
                            h.set("Content-Type", "text/plain; charset=utf-8").ok();
                            h.set("Cache-Control", "public, s-maxage=3600").ok();
                            h.set("Deprecation", "true").ok();
                            h.set("Sunset", "Wed, 01 Jan 2027 00:00:00 GMT").ok();
                            return Ok(Response::from_bytes(info.lrc.into_bytes())?.with_status(200).with_headers(h));
                        }
                        Err(e) => return problem(502, "UPSTREAM_ERROR", "upstream error", Some(e)),
                    }
                }
                _ => return problem(400, "BAD_REQUEST", "unsupported type", Some(format!("type={typ}"))),
            };
            let h = cors_headers();
            h.set("Content-Type", "application/json; charset=utf-8").ok();
            h.set("Cache-Control", cache).ok();
            h.set("Deprecation", "true").ok();
            h.set("Sunset", "Wed, 01 Jan 2027 00:00:00 GMT").ok();
            h.set("Link", r#"<https://api.meting.rs/v1>; rel="successor-version""#).ok();
            Ok(Response::from_json(&json_val)?.with_status(200).with_headers(h))
        }
        _ => problem(404, "NOT_FOUND", "not found", Some(path)),
    }
}
