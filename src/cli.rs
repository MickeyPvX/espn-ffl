//! CLI argument definitions (using clap).

use clap::{Parser, Subcommand};

use crate::cli_types::{LeagueId, Position, Season, Week};

#[derive(Debug, Subcommand)]
pub enum GetCmd {
    /// Fetch and optionally refresh cached league settings for a season + league
    LeagueData {
        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[clap(long, short)]
        league_id: Option<LeagueId>,

        /// Force refresh from ESPN, overwriting the cache.
        #[clap(long)]
        refresh: bool,

        /// Season year (e.g. 2025).
        #[clap(long, short, default_value_t = Season::default())]
        season: Season,

        /// Print the cached path and a short summary when done.
        #[clap(long)]
        verbose: bool,
    },

    /// Get players and their weekly fantasy points.
    ///
    /// Queries `/players?view=kona_player_info` and computes weekly totals
    /// using league settings (read from cache or fetched if missing).
    PlayerData {
        /// Print request URL and headers for debugging.
        #[clap(long)]
        debug: bool,

        /// Output results as JSON instead of text lines.
        #[clap(long)]
        json: bool,

        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[clap(long, short)]
        league_id: Option<LeagueId>,

        /// Limit the number of results
        #[clap(long)]
        limit: Option<u32>,

        /// Filter by player last name (substring match).
        #[clap(long, short = 'n')]
        player_name: Option<String>,

        /// Filter by position (repeatable): `-p QB -p RB`.
        #[clap(short = 'p', long = "position")]
        positions: Option<Vec<Position>>,

        /// Use projected points instead of actual (statSourceId == 1)
        #[clap(long = "proj")]
        projected: bool,

        /// Season year (e.g. 2025).
        #[clap(long, short, default_value_t = Season::default())]
        season: Season,

        /// Single week.
        #[clap(long, short, default_value_t = Week::default())]
        week: Week,
    },
}

#[derive(Debug, Parser)]
#[clap(name = "espn-ffl", about = "ESPN Fantasy Football CLI")]
pub struct ESPN {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Get data from ESPN Fantasy Football
    Get {
        #[clap(subcommand)]
        cmd: GetCmd,
    },
}
