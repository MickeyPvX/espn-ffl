//! Error types for the ESPN Fantasy Football CLI

use thiserror::Error;

pub type Result<T> = std::result::Result<T, EspnError>;

#[derive(Error, Debug)]
pub enum EspnError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid header value: {0}")]
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),

    #[error("League ID not provided and {env_var} environment variable not set")]
    MissingLeagueId { env_var: String },

    #[error("Failed to parse league ID: {0}")]
    InvalidLeagueId(#[from] std::num::ParseIntError),

    #[error("Cache error: {message}")]
    Cache { message: String },

    #[error("ESPN API returned no data")]
    NoData,

    #[error("Invalid position: {position}")]
    InvalidPosition { position: String },

    #[error("Player not found: {name}")]
    PlayerNotFound { name: String },

    #[error("Invalid scoring configuration")]
    InvalidScoring,
}

impl From<Box<dyn std::error::Error + Send + Sync>> for EspnError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        EspnError::Cache {
            message: err.to_string(),
        }
    }
}