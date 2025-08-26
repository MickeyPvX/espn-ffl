//! CLI argument definitions (using structopt).

use crate::cli_types::{Availability, Position};
use structopt::StructOpt;

/// Top-level CLI for ESPN Fantasy Football utilities.
#[derive(Debug, StructOpt)]
pub enum ESPN {
    /// Get players and their weekly fantasy points.
    ///
    /// This subcommand queries ESPN's `/players` endpoint with `kona_player_info`
    /// and filters results client-side for the requested weeks.
    Get {
        /// Availability filter: all | free | onteam
        #[structopt(default_value = "all", long, short)]
        availability: Availability,

        /// Print request URL and headers for debugging.
        #[structopt(long)]
        debug: bool,

        /// Output results as JSON instead of text lines.
        #[structopt(long)]
        json: bool,

        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[structopt(long, short)]
        league_id: Option<u64>,

        /// Filter by player last name (substring match).
        #[structopt(long, short = "n")]
        player_name: Option<String>,

        /// Filter by position (repeatable): `-p QB -p RB`.
        #[structopt(short = "p", long = "position")]
        positions: Option<Vec<Position>>,

        /// Use projected points instead of actual (statSourceId == 1)
        #[structopt(long = "proj")]
        projected: bool,

        /// Season year (e.g. 2025).
        #[structopt(default_value = "2025", long, short)]
        season: u16,

        /// Single week (mutually exclusive with `--weeks`).
        #[structopt(long, short)]
        week: Option<u16>,

        /// Week spec: e.g. `1`, `1,3,5`, `2-6`, `1-4,6,8-10`.
        /// Mutually exclusive with `--week`.
        #[structopt(long)]
        weeks: Option<String>,
    },
}
