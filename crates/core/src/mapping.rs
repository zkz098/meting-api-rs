use serde_json::Value;
use crate::dto::{LyricInfo, PicInfo, Song, UrlInfo};

fn str_or_empty(v: Option<&Value>) -> String {
    v.and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn force_https(s: String) -> String {
    if s.starts_with("http://") {
        format!("https://{}", &s[7..])
    } else {
        s
    }
}

fn artists_from_ar(ar: Option<&Value>) -> Vec<String> {
    match ar {
        Some(Value::Array(arr)) => arr.iter().map(|x| str_or_empty(x.get("name"))).filter(|s| !s.is_empty()).collect(),
        _ => vec![],
    }
}

pub fn map_song(v: &Value) -> Song {
    let id = v.get("id").and_then(|x| x.as_u64()).map(|n| n.to_string())
        .or_else(|| v.get("id").and_then(|x| x.as_str()).map(|s| s.to_string()))
        .unwrap_or_default();
    let name = str_or_empty(v.get("name"));
    let mut artist = artists_from_ar(v.get("ar"));
    if artist.is_empty() {
        if let Some(Value::Array(arr)) = v.get("artists") {
            artist = arr.iter().map(|a| str_or_empty(a.get("name").or(Some(a)))).filter(|s| !s.is_empty()).collect();
        }
    }
    if artist.is_empty() {
        if let Some(s) = v.get("artist").and_then(|x| x.as_str()) {
            artist = vec![s.to_string()];
        }
    }

    let album = v.get("al").and_then(|al| al.get("name")).and_then(|x| x.as_str())
        .or_else(|| v.get("album").and_then(|x| x.as_str()).map(|s| s))
        .or_else(|| v.get("album").and_then(|al| al.get("name")).and_then(|x| x.as_str()))
        .unwrap_or("").to_string();

    let pic_url = v.get("al").and_then(|al| al.get("picUrl")).and_then(|x| x.as_str())
        .or_else(|| v.get("album").and_then(|al| al.get("picUrl")).and_then(|x| x.as_str()))
        .or_else(|| v.get("picUrl").and_then(|x| x.as_str()))
        .map(|s| force_https(s.to_string()));

    let duration = v.get("dt").and_then(|x| x.as_u64())
        .or_else(|| v.get("duration").and_then(|x| x.as_u64()));

    Song {
        id: id.clone(),
        name,
        artist,
        album,
        pic_id: pic_url.clone().unwrap_or_default(),
        url_id: id.clone(),
        lyric_id: id.clone(),
        source: "netease".into(),
        pic_url,
        duration,
    }
}

pub fn map_search_result(v: &Value) -> (Vec<Song>, usize) {
    let songs: Vec<Song> = v.get("result").and_then(|r| r.get("songs")).and_then(|x| x.as_array())
        .or_else(|| v.get("songs").and_then(|x| x.as_array()))
        .map(|arr| arr.iter().map(map_song).collect())
        .unwrap_or_default();
    let total = v.get("result").and_then(|r| r.get("songCount")).and_then(|x| x.as_u64()).unwrap_or(songs.len() as u64) as usize;
    (songs, total)
}

pub fn map_playlist_tracks(v: &Value) -> Vec<Song> {
    if let Some(Value::Array(arr)) = v.get("playlist").and_then(|p| p.get("tracks")) {
        return arr.iter().map(map_song).collect();
    }
    if let Some(Value::Array(arr)) = v.get("songs") {
        return arr.iter().map(map_song).collect();
    }
    vec![]
}

pub fn map_song_detail(v: &Value) -> Vec<Song> {
    v.get("songs").and_then(|x| x.as_array())
        .map(|arr| arr.iter().map(map_song).collect())
        .unwrap_or_default()
}

pub fn map_url(v: &Value, fallback_id: &str) -> UrlInfo {
    let data = v.get("data").and_then(|x| x.as_array()).and_then(|arr| arr.first());
    let url = data.and_then(|d| d.get("url")).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let br = data.and_then(|d| d.get("br")).and_then(|x| x.as_u64()).unwrap_or(128000) as u32;
    let size = data.and_then(|d| d.get("size")).and_then(|x| x.as_u64()).unwrap_or(0);
    let _ = fallback_id;
    UrlInfo { url, br, size, expire_at: None }
}

pub fn map_lyric(v: &Value) -> LyricInfo {
    let lrc = v.get("lrc").and_then(|x| x.get("lyric")).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let tlyric = v.get("tlyric").and_then(|x| x.get("lyric")).and_then(|x| x.as_str()).map(|s| s.to_string()).filter(|s| !s.is_empty());
    let yrc = v.get("yrc").and_then(|x| x.get("lyric")).and_then(|x| x.as_str()).map(|s| s.to_string()).filter(|s| !s.is_empty());
    LyricInfo { lrc, tlyric, yrc }
}

pub fn map_pic(v: &Value, size: u32) -> PicInfo {
    let base = v.get("songs").and_then(|arr| arr.as_array()).and_then(|arr| arr.first())
        .and_then(|s| s.get("al")).and_then(|al| al.get("picUrl")).and_then(|x| x.as_str())
        .unwrap_or("");
    let base = force_https(base.to_string());
    let url = if base.is_empty() { "".into() } else { format!("{base}?param={size}y{size}") };
    PicInfo { url, size: Some(size) }
}
