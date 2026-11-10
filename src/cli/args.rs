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

    /// Season year (defaults to the season in progress).
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

        /// Season year (defaults to the season in progress).
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

        /// Bias adjustment strength (0.0 = none, the default; 1.0 = full correction).
        ///
        /// Backtests on 2025 and 2026 found that correcting for a player's past misses made
        /// projections less accurate at every strength above zero: a player's miss in one
        /// part of the season does not predict the next. Kept for experimentation.
        #[clap(long)]
        bias_strength: Option<f64>,
    },

    /// Win probability for this week's matchup and the lineup most likely to win it.
    ///
    /// Takes ESPN's projections as each player's expected score, models how widely each
    /// player's outcome spreads around it, and compares your current lineup, your highest
    /// projected lineup, and the lineup with the best chance of beating your opponent's.
    /// Projected underdogs are steered toward upside, favourites toward a steady floor.
    ///
    /// Spread estimates come from the local database, so run `update-all-data` first.
    Matchup {
        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[clap(long, short)]
        league_id: Option<LeagueId>,

        /// Season year (defaults to the season in progress).
        #[clap(long, short, default_value_t = Season::default())]
        season: Season,

        /// Week to plan for.
        #[clap(long, short)]
        week: Week,

        /// Your fantasy team name (partial match). Defaults to the team owned by ESPN_SWID.
        #[clap(long)]
        team: Option<String>,

        /// Your fantasy team ID.
        #[clap(long)]
        team_id: Option<u32>,

        /// Opponent team name (partial match). Defaults to this week's scheduled opponent.
        #[clap(long)]
        opponent: Option<String>,

        /// Refetch rosters and projections instead of using the cache.
        #[clap(long)]
        refresh: bool,

        /// Output results as JSON instead of a table.
        #[clap(long)]
        json: bool,
    },

    /// Rank the draft pool by value over replacement in this league's scoring.
    ///
    /// Recomputes each player's season projection using your league's scoring settings,
    /// measures it against the replacement level implied by your starting lineup, and
    /// compares the result to ESPN's average draft position to surface value picks.
    DraftBoard {
        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[clap(long, short)]
        league_id: Option<LeagueId>,

        /// Season year (defaults to the season in progress).
        #[clap(long, short, default_value_t = Season::default())]
        season: Season,

        /// Filter by position (repeatable): `-p RB -p WR`.
        #[clap(short = 'p', long = "position", value_parser = clap::value_parser!(Position))]
        positions: Option<Vec<Position>>,

        /// Number of players to display.
        #[clap(long, default_value_t = 40)]
        top: usize,

        /// How many players to pull from ESPN before ranking.
        #[clap(long, default_value_t = 700)]
        pool_size: u32,

        /// ESPN ranking flavour used to select the pool: PPR, STANDARD, SUPERFLEX.
        #[clap(long, default_value = "PPR")]
        rank_type: String,

        /// Read live draft state: hide drafted players and show your remaining needs.
        #[clap(long)]
        live: bool,

        /// Refetch projections and ADP from ESPN instead of using the cached pool.
        #[clap(long)]
        refresh: bool,

        /// Re-read the draft every N seconds until it completes (implies --live).
        #[clap(long, value_name = "SECONDS")]
        watch: Option<u64>,

        /// Follow the draft on ESPN's live feed, drafting from this tool.
        ///
        /// The only mode that sees picks as they happen: ESPN's REST API reports an
        /// untouched league until the draft completes. Picks remove themselves from the
        /// board, and `draft <name>` sends your pick.
        ///
        /// WARNING: ESPN allows one draft connection per team, so this evicts your browser
        /// draft room. Nothing but autodraft will pick for you, so pick from here.
        #[clap(long)]
        live_draft: bool,

        /// Track the draft interactively: type each pick at a prompt and the board redraws.
        ///
        /// Use this during a live draft, when ESPN's API reports the draft in progress but
        /// publishes no picks. Combine with --taken-file to save picks as you go and resume
        /// after a restart.
        #[clap(long, short = 'i')]
        interactive: bool,

        /// Track picks from a local file instead of ESPN's draft feed (implies --live).
        ///
        /// One player name per line, in pick order; blank lines and `#` comments are
        /// ignored. Prefix a line with `*` to mark it as your own pick. Re-read on every
        /// refresh, so with --watch you can append picks as they happen. Use this when
        /// ESPN's API reports the draft in progress but publishes no picks.
        #[clap(long, value_name = "PATH")]
        taken_file: Option<std::path::PathBuf>,

        /// Your fantasy team name (partial match). Defaults to the team owned by ESPN_SWID.
        #[clap(long)]
        team: Option<String>,

        /// Your fantasy team ID.
        #[clap(long)]
        team_id: Option<u32>,

        /// Output results as JSON instead of a table.
        #[clap(long)]
        json: bool,

        /// Print request URL and filter for debugging.
        #[clap(long)]
        debug: bool,
    },

    /// Rank free agents by how much they would improve your team.
    ///
    /// Each free agent is scored by adding them and dropping whichever of your players costs
    /// the least: the projected points your starting lineup gains over the rest of the
    /// regular season (bench depth and bye weeks included), and the change in your chance
    /// to win this week's matchup.
    Waivers {
        /// League ID (or set `ESPN_FFL_LEAGUE_ID` env var).
        #[clap(long, short)]
        league_id: Option<LeagueId>,

        /// Season year (defaults to the season in progress).
        #[clap(long, short, default_value_t = Season::default())]
        season: Season,

        /// First week to count (normally the upcoming week).
        #[clap(long, short)]
        week: Week,

        /// Last week to count. Defaults to the end of the regular season.
        #[clap(long)]
        through_week: Option<Week>,

        /// Only consider free agents at these positions (repeatable): `-p RB -p WR`.
        #[clap(short = 'p', long = "position", value_parser = clap::value_parser!(Position))]
        positions: Option<Vec<Position>>,

        /// Number of recommendations to show.
        #[clap(long, default_value_t = 15)]
        top: usize,

        /// Your fantasy team name (partial match). Defaults to the team owned by ESPN_SWID.
        #[clap(long)]
        team: Option<String>,

        /// Your fantasy team ID.
        #[clap(long)]
        team_id: Option<u32>,

        /// Refetch rosters and projections instead of using the cache.
        #[clap(long)]
        refresh: bool,

        /// Output results as JSON instead of a table.
        #[clap(long)]
        json: bool,
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

        /// Season year (defaults to the season in progress).
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
