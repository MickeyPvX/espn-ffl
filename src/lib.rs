//! ESPN Fantasy Football CLI Library
//!
//! A Rust library for interacting with ESPN Fantasy Football APIs.

pub mod cli;
pub mod commands;
pub mod core;
pub mod error;
pub mod espn;
pub mod storage;

// Re-export commonly used types
pub use cli::types::{LeagueId, PlayerId, Position, Season, Week};
pub use error::{EspnError, Result};
pub use espn::types::{LeagueSettings, ScoringItem, ScoringSettings};

pub const LEAGUE_ID_ENV_VAR: &str = "ESPN_FFL_LEAGUE_ID";
