use thiserror::Error;

#[derive(Debug, Error)]
pub enum MetingError {
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("rate limited")]
    RateLimited,
}

impl MetingError {
    pub fn status(&self) -> u16 {
        match self {
            Self::UnsupportedPlatform(_) | Self::BadRequest(_) => 400,
            Self::Unauthorized => 401,
            Self::NotFound(_) => 404,
            Self::RateLimited => 429,
            Self::Upstream(_) => 502,
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedPlatform(_) => "UNSUPPORTED_PLATFORM",
            Self::BadRequest(_) => "BAD_REQUEST",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::NotFound(_) => "NOT_FOUND",
            Self::RateLimited => "RATE_LIMITED",
            Self::Upstream(_) => "UPSTREAM_ERROR",
        }
    }
}
