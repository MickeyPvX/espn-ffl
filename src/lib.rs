//! ESPN Fantasy Football CLI Library
//!
//! A comprehensive Rust library for interacting with ESPN Fantasy Football APIs,
//! providing player data, scoring calculations, projection analysis, and database storage.
//!
//! ## Features
//!
//! - **Player Data Retrieval**: Fetch player statistics and fantasy points from ESPN API
//! - **Server-side Filtering**: Efficient filtering by injury status, position, and activity
//! - **Projection Analysis**: Historical bias analysis and adjusted predictions
//! - **Database Storage**: Local caching of player data and statistics
//! - **Roster Management**: Track player roster status across fantasy teams
//! - **Flexible Scoring**: Support for custom league scoring settings
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use espn_ffl::{LeagueId, Season, Week, commands::player_data::*};
//!
//! # async fn example() -> espn_ffl::Result<()> {
//! // Get player data for current week
//! let params = PlayerDataParams {
//!     league_id: Some(LeagueId::new(123456)),
//!     season: Season::default(),
//!     week: Week::new(1),
//!     // ... other parameters
//! #   debug: false,
//! #   as_json: false,
//! #   player_name: None,
//! #   positions: None,
//! #   projected: false,
//! #   refresh_positions: false,
//! #   clear_db: false,
//! #   refresh: false,
//! #   injury_status: None,
//! #   roster_status: None,
//! };
//!
//! handle_player_data(params).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Environment Configuration
//!
//! Set your ESPN league ID to avoid passing it in every command:
//! ```bash
//! export ESPN_FFL_LEAGUE_ID=123456
//! ```

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
