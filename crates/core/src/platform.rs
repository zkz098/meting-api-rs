use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Netease,
    Tencent,
    Kugou,
    Kuwo,
    Baidu,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Netease => "netease",
            Self::Tencent => "tencent",
            Self::Kugou => "kugou",
            Self::Kuwo => "kuwo",
            Self::Baidu => "baidu",
        }
    }
}

impl std::str::FromStr for Platform {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "netease" | "163" => Ok(Self::Netease),
            "tencent" | "qq" => Ok(Self::Tencent),
            "kugou" => Ok(Self::Kugou),
            "kuwo" => Ok(Self::Kuwo),
            "baidu" => Ok(Self::Baidu),
            _ => Err(format!("unsupported platform: {s}")),
        }
    }
}

/// alias `server` -> `platform` for legacy compat (?server=netease)
pub fn parse_platform(raw: Option<&str>) -> Platform {
    raw.and_then(|s| s.parse().ok()).unwrap_or(Platform::Netease)
}
