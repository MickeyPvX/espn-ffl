//! ESPN Fantasy Football CLI Library
//!
//! A Rust library for interacting with ESPN Fantasy Football APIs.

pub mod cache;
pub mod cli;
pub mod cli_types;
pub mod commands;
pub mod database;
pub mod error;
pub mod espn;
pub mod filters;
pub mod util;

// Re-export commonly used types
pub use cli_types::{LeagueId, PlayerId, Position, Season, Week};
pub use error::{EspnError, Result};
pub use espn::types::{LeagueSettings, ScoringItem, ScoringSettings};

pub const LEAGUE_ID_ENV_VAR: &str = "ESPN_FFL_LEAGUE_ID";
