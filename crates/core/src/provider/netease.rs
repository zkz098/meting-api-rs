use serde_json::{json, Value};
use crate::crypto::{weapi::weapi_encrypt, eapi::eapi_encrypt};

/// Endpoints (weapi)
const BASE: &str = "https://music.163.com";

#[derive(Debug, Clone)]
pub struct NeteaseProvider {
    pub base: String,
}

impl Default for NeteaseProvider {
    fn default() -> Self {
        Self { base: BASE.into() }
    }
}

impl NeteaseProvider {
    pub fn new(base: Option<String>) -> Self {
        Self { base: base.unwrap_or_else(|| BASE.into()) }
    }

    /// Build form body for weapi: params, encSecKey
    pub fn build_weapi_body(&self, payload: &Value) -> String {
        let text = serde_json::to_string(payload).unwrap();
        let (params, enc_sec_key) = weapi_encrypt(&text);
        format!(
            "params={}&encSecKey={}",
            urlencoding::encode(&params),
            urlencoding::encode(&enc_sec_key)
        )
    }

    pub fn search_body(&self, keyword: &str, limit: u32, offset: u32) -> String {
        self.build_weapi_body(&json!({
            "s": keyword,
            "type": 1,
            "limit": limit,
            "offset": offset,
            "csrf_token": ""
        }))
    }

    /// eapi variant for cloudsearch (preferred since 2023)
    pub fn search_eapi_body(&self, keyword: &str, limit: u32, offset: u32) -> (String, String) {
        let url = "/api/cloudsearch/pc";
        let payload = json!({
            "s": keyword,
            "type": 1,
            "limit": limit,
            "offset": offset,
            "csrf_token": ""
        });
        let params = eapi_encrypt(url, &payload);
        let body = format!("params={}", urlencoding::encode(&params));
        (body, self.endpoint("/eapi/cloudsearch/pc"))
    }

    pub fn search_eapi_keywords_body(&self, keyword: &str, limit: u32, offset: u32) -> (String, String) {
        let url = "/api/cloudsearch/pc";
        let payload = json!({
            "keywords": keyword,
            "type": 1,
            "limit": limit,
            "offset": offset,
            "csrf_token": ""
        });
        let params = eapi_encrypt(url, &payload);
        let body = format!("params={}", urlencoding::encode(&params));
        (body, self.endpoint("/eapi/cloudsearch/pc"))
    }

    pub fn playlist_body(&self, id: &str) -> String {
        self.build_weapi_body(&json!({
            "id": id,
            "n": 100000,
            "s": 8,
            "csrf_token": ""
        }))
    }

    pub fn song_detail_body(&self, ids: &[String]) -> String {
        let c = serde_json::to_string(
            &ids.iter().map(|id| json!({"id": id})).collect::<Vec<_>>()
        ).unwrap();
        self.build_weapi_body(&json!({
            "c": c,
            "csrf_token": ""
        }))
    }

    pub fn url_body(&self, ids: &[String], br: u32) -> String {
        self.build_weapi_body(&json!({
            "ids": ids,
            "br": br,
            "csrf_token": ""
        }))
    }

    pub fn lyric_body(&self, id: &str) -> String {
        self.build_weapi_body(&json!({
            "id": id,
            "lv": -1,
            "kv": -1,
            "tv": -1,
            "csrf_token": ""
        }))
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
}

// urlencoding helper without extra crate
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut out = String::new();
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
                _ => out.push_str(&format!("%{:02X}", b)),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn body_not_empty() {
        let p = NeteaseProvider::default();
        let b = p.search_body("hello", 20, 0);
        assert!(b.contains("params="));
        assert!(b.contains("encSecKey="));
    }
}
