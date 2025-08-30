//! CLI argument definitions (using structopt).

use structopt::StructOpt;

use crate::cli_types::Position;

#[derive(Debug, StructOpt)]
pub enum GetCmd {
    /// Fetch and optionally refresh cached league settings for a season + league
    LeagueData {
        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[structopt(long, short)]
        league_id: Option<u32>,

        /// Force refresh from ESPN, overwriting the cache.
        #[structopt(long)]
        refresh: bool,

        /// Season year (e.g. 2025).
        #[structopt(default_value = "2025", long, short)]
        season: u16,

        /// Print the cached path and a short summary when done.
        #[structopt(long)]
        verbose: bool,
    },

    /// Get players and their weekly fantasy points.
    ///
    /// Queries `/players?view=kona_player_info` and computes weekly totals
    /// using league settings (read from cache or fetched if missing).
    PlayerData {
        /// Print request URL and headers for debugging.
        #[structopt(long)]
        debug: bool,

        /// Output results as JSON instead of text lines.
        #[structopt(long)]
        json: bool,

        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[structopt(long, short)]
        league_id: Option<u32>,

        /// Limit the number of results
        #[structopt(long)]
        limit: Option<u32>,

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

        /// Single week.
        #[structopt(default_value = "1", long, short)]
        week: u16,
    },
}

#[derive(Debug, StructOpt)]
pub enum ESPN {
    /// Group for read operations
    Get(GetCmd),
}
