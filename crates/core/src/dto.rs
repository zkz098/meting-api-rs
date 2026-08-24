use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    pub name: String,
    pub artist: Vec<String>,
    pub album: String,
    pub pic_id: String,
    pub url_id: String,
    pub lyric_id: String,
    pub source: String,
    /// normalized fields for /v1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pic_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlInfo {
    pub url: String,
    pub br: u32,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PicInfo {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricInfo {
    pub lrc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tlyric: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yrc: Option<String>,
}

// generic envelope for /v1
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: u16,
    pub message: String,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self { code: 0, message: "ok".into(), data, meta: None }
    }
    pub fn ok_with_meta(data: T, meta: Meta) -> Self {
        Self { code: 0, message: "ok".into(), data, meta: Some(meta) }
    }
}

// RFC9457 problem+json
#[derive(Debug, Serialize)]
pub struct Problem {
    #[serde(rename = "type")]
    pub typ: String,
    pub title: String,
    pub status: u16,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}
