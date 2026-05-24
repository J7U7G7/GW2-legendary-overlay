use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("gw2 api returned {status}: {body}")]
    Api { status: u16, body: String },

    #[error("invalid or revoked GW2 API key")]
    Unauthorized,

    #[error("GW2 API rate limit exceeded after retries")]
    RateLimited,

    #[error("GW2 API unavailable after retries (last status: {0})")]
    Unavailable(u16),

    #[error("invalid GW2 API key format")]
    BadKeyFormat,

    #[error("GW2 API key is missing required permissions: {0}")]
    MissingPermissions(String),

    #[error("no GW2 API key configured")]
    NoApiKey,

    #[error("windows crypto error: {0}")]
    WinCrypto(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
