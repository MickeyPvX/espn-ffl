//! CLI argument definitions and parsing structures.

use super::types::{
    filters::{FantasyTeamFilter, InjuryStatusFilter, RosterStatusFilter},
    ids::LeagueId,
    position::Position,
    time::{Season, Week},
};
use clap::{Args, Parser, Subcommand};

/// Common filtering arguments shared between commands
#[derive(Debug, Args)]
pub struct CommonFilters {
    /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
    #[clap(long, short)]
    pub league_id: Option<LeagueId>,

    /// Filter by player last name (substring match) - repeatable: `-n Smith -n Johnson`.
    #[clap(long, short = 'n')]
    pub player_name: Option<Vec<String>>,

    /// Filter by position (repeatable): `-p QB -p RB`.
    #[clap(short = 'p', long = "position", value_parser = clap::value_parser!(Position))]
    pub positions: Option<Vec<Position>>,

    /// Season year (e.g. 2025).
    #[clap(long, short, default_value_t = Season::default())]
    pub season: Season,

    /// Single week.
    #[clap(long, short, default_value_t = Week::default())]
    pub week: Week,

    /// Filter by injury status.
    #[clap(long)]
    pub injury_status: Option<InjuryStatusFilter>,

    /// Filter by roster status.
    #[clap(long)]
    pub roster_status: Option<RosterStatusFilter>,

    /// Filter by fantasy team name (partial matching).
    #[clap(long)]
    pub team: Option<String>,

    /// Filter by exact fantasy team ID.
    #[clap(long)]
    pub team_id: Option<u32>,
}

impl CommonFilters {
    /// Get the fantasy team filter if specified
    pub fn get_fantasy_team_filter(&self) -> Option<FantasyTeamFilter> {
        self.team
            .as_ref()
            .map(|team_name| FantasyTeamFilter::Name(team_name.clone()))
            .or_else(|| self.team_id.map(FantasyTeamFilter::Id))
            .or(None)
    }
}

#[derive(Debug, Parser)]
#[clap(name = "espn-ffl", about = "ESPN Fantasy Football CLI")]
pub struct ESPN {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
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
        #[clap(flatten)]
        filters: CommonFilters,

        /// Print request URL and headers for debugging.
        #[clap(long)]
        debug: bool,

        /// Output results as JSON instead of text lines.
        #[clap(long)]
        json: bool,

        /// Use projected points instead of actual (statSourceId == 1)
        #[clap(long = "proj")]
        projected: bool,

        /// Force refresh player positions in database (useful after position mapping updates)
        #[clap(long)]
        refresh_positions: bool,

        /// Clear all data from the database before fetching (useful for starting fresh)
        #[clap(long)]
        clear_db: bool,

        /// Force refresh from ESPN API even if cached data exists
        #[clap(long)]
        refresh: bool,
    },

    /// Analyze projection accuracy and generate predictions for players.
    ///
    /// Uses historical projection vs actual data to adjust ESPN projections.
    ProjectionAnalysis {
        #[clap(flatten)]
        filters: CommonFilters,

        /// Output results as JSON instead of text lines.
        #[clap(long)]
        json: bool,

        /// Force refresh from ESPN API even if cached data exists
        #[clap(long)]
        refresh: bool,

        /// Bias adjustment strength (0.0 = no adjustment, 1.0 = full bias correction, >1.0 = amplified correction)
        #[clap(long)]
        bias_strength: Option<f64>,
    },

    /// Update all player data (actual and projected) for multiple weeks.
    ///
    /// Efficiently populates the database with complete historical data needed
    /// for accurate projection analysis by fetching both actual and projected
    /// points for all players from week 1 through the specified week.
    UpdateAllData {
        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[clap(long, short)]
        league_id: Option<LeagueId>,

        /// Season year (e.g. 2025).
        #[clap(long, short, default_value_t = Season::default())]
        season: Season,

        /// Update data through this week (inclusive) - e.g., 4 means weeks 1,2,3,4.
        #[clap(long)]
        through_week: Week,

        /// Show detailed progress information.
        #[clap(long)]
        verbose: bool,
    },
}
