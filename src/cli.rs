//! CLI argument definitions (using structopt).

use structopt::StructOpt;
use crate::positions::Position;

/// Top-level CLI for ESPN Fantasy Football utilities.
#[derive(Debug, StructOpt)]
pub enum ESPN {
    /// Get players and their weekly fantasy points.
    ///
    /// This subcommand queries ESPN's `/players` endpoint with `kona_player_info`
    /// and filters results client-side for the requested weeks.
    Get {
        /// Print request URL and headers for debugging.
        #[structopt(long)]
        debug: bool,

        /// Output results as JSON instead of text lines.
        #[structopt(long)]
        json: bool,

        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[structopt(long)]
        league_id: Option<u64>,

        /// Filter by player last name (substring match).
        #[structopt(long)]
        player_name: Option<String>,

        /// Filter by position (repeatable): `-p QB -p RB`.
        #[structopt(short = "p", long = "position")]
        positions: Option<Vec<Position>>,

        /// Season year (e.g. 2025).
        #[structopt(long, default_value = "2025")]
        season: u16,

        /// Single week (mutually exclusive with `--weeks`).
        #[structopt(long)]
        week: Option<u16>,

        /// Week spec: e.g. `1`, `1,3,5`, `2-6`, `1-4,6,8-10`.
        /// Mutually exclusive with `--week`.
        #[structopt(long)]
        weeks: Option<String>,
    },
}
