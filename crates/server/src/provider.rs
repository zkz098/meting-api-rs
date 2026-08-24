use meting_core::{mapping, provider::NeteaseProvider};
use serde_json::Value;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, REFERER, USER_AGENT};

async fn weapi_post(url: &str, body: &str) -> Result<Value, String> {
    weapi_post_with_headers(url, body, None).await
}

async fn eapi_post(url: &str, body: &str) -> Result<Value, String> {
    weapi_post_with_headers(url, body, Some("eapi")).await
}

/// 简易 xorshift PRNG（避免为 deviceId 引入 rand 依赖）
struct XorShift64(u64);
impl XorShift64 {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Android 网易云客户端 header（meting.js 同款姿势：随机 deviceId/requestId）
fn android_client_headers() -> HeaderMap {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x9e3779b97f4a7c15);
    let mut rng = XorShift64(seed | 1);
    let device_id: String = (0..16).map(|_| format!("{:02X}", (rng.next() & 0xff) as u8)).collect();
    let request_id = format!("{}_{:04}", seed / 1_000_000, rng.next() % 1000);
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Linux; Android 11; M2007J3SC Build/RKQ1.200826.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/77.0.3865.120 MQQBrowser/6.2 TBS/045714 Mobile Safari/537.36 NeteaseMusic/8.7.01"));
    headers.insert(REFERER, HeaderValue::from_static("music.163.com"));
    headers.insert("Content-Type", HeaderValue::from_static("application/x-www-form-urlencoded"));
    headers.insert(COOKIE, HeaderValue::from_str(&format!("osver=android; appver=8.7.01; os=android; deviceId={device_id}; channel=netease; requestId={request_id}; __remember_me=true")).unwrap_or_else(|_| HeaderValue::from_static("os=android; appver=8.7.01")));
    headers
}

async fn eapi_post_android(url: &str, body: &str) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let resp = match client.post(url).headers(android_client_headers()).body(body.to_string()).send().await {
        Ok(r) => r,
        Err(e) => return Err(e.to_string()),
    };
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("upstream {status}: {}", &text[..text.len().min(800)]));
    }
    serde_json::from_str(&text).map_err(|e| format!("invalid json {e}: {}", &text[..text.len().min(1200)]))
}

fn slice600(s: &str) -> &str {
    &s[..s.len().min(600)]
}

/// /v1/songs/:id/url 上游获取：三级 fallback（weapi PC → eapi PC → eapi Android），
/// 任一拿到非空 url 立即返回。全空时返回最后一级的错误/空信息，由 handler 报 UPSTREAM_EMPTY_URL。
pub async fn url_info_fallback(id: &str, br: u32) -> Result<meting_core::UrlInfo, String> {
    let p = NeteaseProvider::default();
    let mut errors: Vec<String> = Vec::new();

    // 1) weapi PC（现状；住宅/国内 IP 下通常直接可用）
    let body = p.url_body(&[id.to_string()], br);
    match weapi_post(&p.endpoint("/weapi/song/enhance/player/url"), &body).await {
        Ok(v) => {
            let info = mapping::map_url(&v, id);
            if !info.url.is_empty() {
                return Ok(info);
            }
            errors.push(format!("weapi empty: {}", slice600(&v.to_string())));
        }
        Err(e) => errors.push(e),
    }

    // 2) eapi PC（meting.js 端点族；数据中心 IP 下可能放行）
    let (ebody, eurl) = p.url_eapi_body(&[id.to_string()], br);
    match eapi_post(&eurl, &ebody).await {
        Ok(v) => {
            let info = mapping::map_url(&v, id);
            if !info.url.is_empty() {
                return Ok(info);
            }
            errors.push(format!("eapi(PC) empty: {}", slice600(&v.to_string())));
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
            errors.push(format!("eapi(android) empty: {}", slice600(&v.to_string())));
        }
        Err(e) => errors.push(e),
    }

    Err(errors.join(" | "))
}

async fn weapi_post_with_headers(url: &str, body: &str, _kind: Option<&str>) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"));
    headers.insert(REFERER, HeaderValue::from_static("https://music.163.com/"));
    headers.insert("Content-Type", HeaderValue::from_static("application/x-www-form-urlencoded"));
    headers.insert(COOKIE, HeaderValue::from_static("os=pc; appver=8.9.75; osver=Microsoft-Windows-10-Professional-build-19045-64bit; channel=netease;"));

    let resp = client.post(url).headers(headers).body(body.to_string()).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("upstream {status}: {}", &text[..text.len().min(800)]));
    }
    // eapi response is JSON directly; weapi also JSON
    serde_json::from_str(&text).map_err(|e| format!("invalid json {e}: {}", &text[..text.len().min(1200)]))
}

pub async fn search(keyword: &str, limit: u32, offset: u32) -> Result<(Vec<meting_core::Song>, usize), String> {
    let p = NeteaseProvider::default();
    // Try eapi first (2023+), fallback to weapi
    let mut last_err = String::new();
    for (body, url) in [
        p.search_eapi_body(keyword, limit, offset),
        p.search_eapi_keywords_body(keyword, limit, offset),
    ] {
        match eapi_post(&url, &body).await {
            Ok(v) => {
                // eapi returns code 200 on success
                if v.get("code").and_then(|x| x.as_i64()) == Some(200) || v.get("result").is_some() {
                    let (songs, total) = mapping::map_search_result(&v);
                    if !songs.is_empty() || total > 0 {
                        return Ok((songs, total));
                    }
                    // if empty but code 200, treat as success (keyword no result)
                    if v.get("code").and_then(|x| x.as_i64()) == Some(200) {
                        return Ok((songs, total));
                    }
                }
                last_err = format!("eapi empty: {}", &v.to_string()[..v.to_string().len().min(600)]);
            }
            Err(e) => last_err = e,
        }
    }
    // fallback weapi
    let body = p.search_body(keyword, limit, offset);
    let url = p.endpoint("/weapi/cloudsearch/get/web");
    match weapi_post(&url, &body).await {
        Ok(v) => {
            if v.get("code").and_then(|x| x.as_i64()) == Some(400) {
                return Err(format!("upstream code 400: {}", v));
            }
            if v.to_string().contains("50000005") {
                return Err(format!("weapi 50000005 (deprecated): {} last_eapi_err={}", v, last_err));
            }
            Ok(mapping::map_search_result(&v))
        }
        Err(e) => Err(format!("all search failed eapi:{last_err} weapi:{e}")),
    }
}

pub async fn playlist(id: &str) -> Result<Vec<meting_core::Song>, String> {
    let p = NeteaseProvider::default();
    let body = p.playlist_body(id);
    let url = p.endpoint("/weapi/v3/playlist/detail");
    let v = weapi_post(&url, &body).await?;
    Ok(mapping::map_playlist_tracks(&v))
}

pub async fn song_detail(ids: &[String]) -> Result<Vec<meting_core::Song>, String> {
    let p = NeteaseProvider::default();
    let body = p.song_detail_body(ids);
    let url = p.endpoint("/weapi/v3/song/detail");
    let v = weapi_post(&url, &body).await?;
    Ok(mapping::map_song_detail(&v))
}

pub async fn url_info(id: &str, br: u32) -> Result<meting_core::UrlInfo, String> {
    let p = NeteaseProvider::default();
    let body = p.url_body(&[id.to_string()], br);
    let url = p.endpoint("/weapi/song/enhance/player/url");
    let v = weapi_post(&url, &body).await?;
    Ok(mapping::map_url(&v, id))
}

pub async fn lyric_info(id: &str) -> Result<meting_core::LyricInfo, String> {
    let p = NeteaseProvider::default();
    let body = p.lyric_body(id);
    let url = p.endpoint("/weapi/song/lyric");
    let v = weapi_post(&url, &body).await?;
    Ok(mapping::map_lyric(&v))
}

pub async fn pic_info(id: &str, size: u32) -> Result<meting_core::PicInfo, String> {
    // reuse song_detail to get picUrl
    let songs = song_detail(&[id.to_string()]).await?;
    if let Some(s) = songs.first() {
        if let Some(pic) = &s.pic_url {
            let url = format!("{pic}?param={size}y{size}");
            return Ok(meting_core::PicInfo { url, size: Some(size) });
        }
    }
    // fallback: direct via api
    Ok(meting_core::PicInfo { url: format!("https://p2.music.126.net/placeholder?param={size}y{size}"), size: Some(size) })
}
