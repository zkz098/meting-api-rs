// 手动实验用：pnpm 无关联；`cargo test -p meting-server --test url_eapi -- --ignored --nocapture`（依赖住宅/国内网络）
//! 集成测试：直接验证网易云 eapi url 端点（meting.js 姿势）在本机网络下是否下发播放地址。
//! 目的：分离「eapi 实现是否正确」与「CF 出口 IP 是否被风控」两个变量。
//! 本地全部拿不到 url ≠ 实现错误；本地拿到 + CF 拿不到 = IP 风控（需部署后实测 CF 侧）。
use meting_core::{mapping, provider::NeteaseProvider};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, REFERER, USER_AGENT};

const SONG_ID: &str = "3411999848";

fn pc_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"));
    h.insert(REFERER, HeaderValue::from_static("https://music.163.com/"));
    h.insert("Content-Type", HeaderValue::from_static("application/x-www-form-urlencoded"));
    h.insert(COOKIE, HeaderValue::from_static("os=pc; appver=8.9.75; osver=Microsoft-Windows-10-Professional-build-19045-64bit; channel=netease;"));
    h
}

fn android_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Linux; Android 11; M2007J3SC Build/RKQ1.200826.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/77.0.3865.120 MQQBrowser/6.2 TBS/045714 Mobile Safari/537.36 NeteaseMusic/8.7.01"));
    h.insert(REFERER, HeaderValue::from_static("music.163.com"));
    h.insert("Content-Type", HeaderValue::from_static("application/x-www-form-urlencoded"));
    h.insert(COOKIE, HeaderValue::from_static("osver=android; appver=8.7.01; os=android; deviceId=TESTDEVICE1234567890ABCDEF; channel=netease; requestId=1700000000000_0420; __remember_me=true"));
    h
}

async fn post_eapi(url: &str, body: &str, headers: &HeaderMap) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let resp = client.post(url).headers(headers.clone()).body(body.to_string()).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("upstream {status}: {}", &text[..text.len().min(400)]));
    }
    serde_json::from_str(&text).map_err(|e| format!("invalid json {e}: {}", &text[..text.len().min(400)]))
}

/// eapi PC 头（web 风格）→ url 端点
#[tokio::test]
#[ignore]
async fn eapi_url_pc_headers() {
    let p = NeteaseProvider::default();
    let (body, url) = p.url_eapi_body(&[SONG_ID.to_string()], 320000);
    println!("POST {url}");
    let v = post_eapi(&url, &body, &pc_headers()).await.expect("eapi POST failed");
    println!("upstream json: {}", &v.to_string()[..v.to_string().len().min(500)]);
    let info = mapping::map_url(&v, SONG_ID);
    println!("mapped url empty? {}", info.url.is_empty());
    if info.url.is_empty() {
        panic!("eapi(PC) returned no url on local network — 若 CF 同样为空则纯 IP 风控，若仅本地为空则实现需排查");
    }
    println!("url: {}", &info.url[..info.url.len().min(120)]);
}

/// eapi + Android 客户端头（meting.js 完整姿势）→ url 端点
#[tokio::test]
#[ignore]
async fn eapi_url_android_headers() {
    let p = NeteaseProvider::default();
    let (body, url) = p.url_eapi_body(&[SONG_ID.to_string()], 320000);
    let v = post_eapi(&url, &body, &android_headers()).await.expect("eapi android POST failed");
    println!("upstream json: {}", &v.to_string()[..v.to_string().len().min(500)]);
    let info = mapping::map_url(&v, SONG_ID);
    println!("mapped url empty? {}", info.url.is_empty());
    if info.url.is_empty() {
        panic!("eapi(android) returned no url on local network");
    }
    println!("url: {}", &info.url[..info.url.len().min(120)]);
}