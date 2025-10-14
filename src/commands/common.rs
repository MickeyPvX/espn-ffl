//! Common utilities and helper functions shared across commands.
//!
//! This module contains shared functionality that would otherwise be duplicated
//! across different command implementations.

use std::collections::BTreeMap;

use crate::{
    cli::types::{
        filters::{FantasyTeamFilter, InjuryStatusFilter, RosterStatusFilter},
        position::Position,
    },
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::build_scoring_index,
        http::{get_league_roster_data, PlayerDataRequest},
        types::{LeagueData, LeagueSettings},
    },
    storage::PlayerDatabase,
    LeagueId, Result, Season, Week,
};

/// Type alias for scoring index
pub type ScoringIndex = BTreeMap<u16, (f64, BTreeMap<u8, f64>)>;

/// Shared command parameters that are common across multiple commands
#[derive(Debug, Clone)]
pub struct CommandParams {
    pub league_id: Option<LeagueId>,
    pub season: Season,
    pub week: Week,
    pub as_json: bool,
    pub refresh: bool,
    pub player_names: Option<Vec<String>>,
    pub positions: Option<Vec<Position>>,
    pub injury_status: Option<InjuryStatusFilter>,
    pub roster_status: Option<RosterStatusFilter>,
    pub fantasy_team_filter: Option<FantasyTeamFilter>,
}

impl CommandParams {
    /// Create new parameters with required fields
    pub fn new(season: Season, week: Week) -> Self {
        Self {
            league_id: None,
            season,
            week,
            as_json: false,
            refresh: false,
            player_names: None,
            positions: None,
            injury_status: None,
            roster_status: None,
            fantasy_team_filter: None,
        }
    }

    /// Set league ID
    pub fn with_league_id(mut self, league_id: LeagueId) -> Self {
        self.league_id = Some(league_id);
        self
    }

    /// Filter by specific player names
    pub fn with_player_names(mut self, names: Vec<String>) -> Self {
        self.player_names = Some(names);
        self
    }

    /// Filter by positions
    pub fn with_positions(mut self, positions: Vec<Position>) -> Self {
        self.positions = Some(positions);
        self
    }

    /// Output as JSON
    pub fn with_json_output(mut self) -> Self {
        self.as_json = true;
        self
    }

    /// Force refresh from API
    pub fn with_refresh(mut self) -> Self {
        self.refresh = true;
        self
    }

    /// Filter by injury status
    pub fn with_injury_filter(mut self, filter: InjuryStatusFilter) -> Self {
        self.injury_status = Some(filter);
        self
    }

    /// Filter by roster status
    pub fn with_roster_filter(mut self, filter: RosterStatusFilter) -> Self {
        self.roster_status = Some(filter);
        self
    }

    /// Filter by fantasy team
    pub fn with_fantasy_team_filter(mut self, filter: FantasyTeamFilter) -> Self {
        self.fantasy_team_filter = Some(filter);
        self
    }
}

/// Trait for common command parameter building patterns
pub trait CommandParamsBuilder {
    /// Get mutable access to the base CommandParams
    fn base_mut(&mut self) -> &mut CommandParams;

    /// Get access to the base CommandParams
    fn base(&self) -> &CommandParams;

    /// Set league ID
    fn with_league_id(mut self, league_id: LeagueId) -> Self
    where
        Self: Sized,
    {
        self.base_mut().league_id = Some(league_id);
        self
    }

    /// Filter by specific player names
    fn with_player_names(mut self, names: Vec<String>) -> Self
    where
        Self: Sized,
    {
        self.base_mut().player_names = Some(names);
        self
    }

    /// Filter by positions
    fn with_positions(mut self, positions: Vec<Position>) -> Self
    where
        Self: Sized,
    {
        self.base_mut().positions = Some(positions);
        self
    }

    /// Output as JSON
    fn with_json_output(mut self) -> Self
    where
        Self: Sized,
    {
        self.base_mut().as_json = true;
        self
    }

    /// Force refresh from API
    fn with_refresh(mut self) -> Self
    where
        Self: Sized,
    {
        self.base_mut().refresh = true;
        self
    }

    /// Filter by injury status
    fn with_injury_filter(mut self, filter: InjuryStatusFilter) -> Self
    where
        Self: Sized,
    {
        self.base_mut().injury_status = Some(filter);
        self
    }

    /// Filter by roster status
    fn with_roster_filter(mut self, filter: RosterStatusFilter) -> Self
    where
        Self: Sized,
    {
        self.base_mut().roster_status = Some(filter);
        self
    }

    /// Filter by fantasy team
    fn with_fantasy_team_filter(mut self, filter: FantasyTeamFilter) -> Self
    where
        Self: Sized,
    {
        self.base_mut().fantasy_team_filter = Some(filter);
        self
    }

    /// Set league ID if provided
    fn with_optional_league_id(mut self, league_id: Option<LeagueId>) -> Self
    where
        Self: Sized,
    {
        if let Some(id) = league_id {
            self.base_mut().league_id = Some(id);
        }
        self
    }

    /// Filter by player names if provided
    fn with_optional_player_names(mut self, names: Option<Vec<String>>) -> Self
    where
        Self: Sized,
    {
        if let Some(names) = names {
            self.base_mut().player_names = Some(names);
        }
        self
    }

    /// Filter by positions if provided
    fn with_optional_positions(mut self, positions: Option<Vec<Position>>) -> Self
    where
        Self: Sized,
    {
        if let Some(positions) = positions {
            self.base_mut().positions = Some(positions);
        }
        self
    }

    /// Filter by injury status if provided
    fn with_optional_injury_filter(mut self, filter: Option<InjuryStatusFilter>) -> Self
    where
        Self: Sized,
    {
        if let Some(filter) = filter {
            self.base_mut().injury_status = Some(filter);
        }
        self
    }

    /// Filter by roster status if provided
    fn with_optional_roster_filter(mut self, filter: Option<RosterStatusFilter>) -> Self
    where
        Self: Sized,
    {
        if let Some(filter) = filter {
            self.base_mut().roster_status = Some(filter);
        }
        self
    }

    /// Filter by fantasy team if provided
    fn with_optional_fantasy_team_filter(mut self, filter: Option<FantasyTeamFilter>) -> Self
    where
        Self: Sized,
    {
        if let Some(filter) = filter {
            self.base_mut().fantasy_team_filter = Some(filter);
        }
        self
    }

    /// Set JSON output conditionally
    fn with_json_output_if(mut self, json: bool) -> Self
    where
        Self: Sized,
    {
        if json {
            self.base_mut().as_json = true;
        }
        self
    }

    /// Set refresh conditionally
    fn with_refresh_if(mut self, refresh: bool) -> Self
    where
        Self: Sized,
    {
        if refresh {
            self.base_mut().refresh = true;
        }
        self
    }
}

/// Context containing common resources needed by most commands
pub struct CommandContext {
    pub league_id: LeagueId,
    pub db: PlayerDatabase,
    pub settings: LeagueSettings,
    pub scoring_index: ScoringIndex,
}

impl CommandContext {
    /// Initialize common command context with database and league settings
    pub async fn new(league_id: LeagueId, season: Season, verbose: bool) -> Result<Self> {
        if verbose {
            println!("Connecting to database...");
        }
        let db = PlayerDatabase::new()?;

        if verbose {
            println!("Loading league scoring settings...");
        }
        let settings = load_or_fetch_league_settings(league_id, false, season).await?;
        let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

        Ok(Self {
            league_id,
            db,
            settings,
            scoring_index,
        })
    }
}

/// Fetch week-specific roster data and display appropriate message
pub async fn fetch_roster_data_with_message(
    league_id: LeagueId,
    season: Season,
    week: Option<Week>,
    refresh: bool,
    verbose: bool,
) -> Result<Option<LeagueData>> {
    match get_league_roster_data(false, league_id, season, week, refresh).await {
        Ok((data, cache_status)) => {
            if verbose {
                match cache_status {
                    crate::espn::http::CacheStatus::Hit => {
                        if let Some(w) = week {
                            println!("✓ Week {} roster status loaded (from cache)", w.as_u16());
                        } else {
                            println!("✓ Current roster status loaded (from cache)");
                        }
                    }
                    crate::espn::http::CacheStatus::Miss => {
                        if let Some(w) = week {
                            println!("✓ Week {} roster status fetched (cache miss)", w.as_u16());
                        } else {
                            println!("✓ Current roster status fetched (cache miss)");
                        }
                    }
                    crate::espn::http::CacheStatus::Refreshed => {
                        if let Some(w) = week {
                            println!("✓ Week {} roster status fetched (refreshed)", w.as_u16());
                        } else {
                            println!("✓ Current roster status fetched (refreshed)");
                        }
                    }
                }
            }
            Ok(Some(data))
        }
        Err(e) => {
            if verbose {
                if let Some(w) = week {
                    println!("⚠ Could not fetch week {} roster data: {}", w.as_u16(), e);
                } else {
                    println!("⚠ Could not fetch current roster data: {}", e);
                }
            }
            Ok(None)
        }
    }
}

/// Convert player's default_position_id to a safe position_id for scoring calculations
pub fn normalize_position_id(default_position_id: i32) -> u8 {
    if default_position_id < 0 {
        0u8 // Default to QB position for scoring purposes
    } else {
        default_position_id as u8
    }
}

/// Convert player's default_position_id to a position string
pub fn position_id_to_string(default_position_id: i32) -> String {
    use crate::cli::types::position::Position;

    if default_position_id < 0 {
        "UNKNOWN".to_string()
    } else {
        Position::try_from(default_position_id as u8)
            .map(|p| p.to_string())
            .unwrap_or_else(|_| "UNKNOWN".to_string())
    }
}

/// Extension trait for PlayerDataRequest to improve builder pattern
pub trait PlayerDataRequestExt {
    /// Create a request for projected data (common pattern)
    fn for_projections(league_id: LeagueId, season: Season, week: Week) -> Self;

    /// Create a request for actual data (common pattern)
    fn for_actual_data(league_id: LeagueId, season: Season, week: Week) -> Self;

    /// Add filters in a fluent interface style
    fn with_filters(
        self,
        player_names: Option<Vec<String>>,
        positions: Option<Vec<Position>>,
        injury_filter: Option<InjuryStatusFilter>,
        roster_filter: Option<RosterStatusFilter>,
    ) -> Self;
}

impl PlayerDataRequestExt for PlayerDataRequest {
    fn for_projections(league_id: LeagueId, season: Season, week: Week) -> Self {
        Self::new(league_id, season, week)
    }

    fn for_actual_data(league_id: LeagueId, season: Season, week: Week) -> Self {
        Self::new(league_id, season, week)
    }

    fn with_filters(
        mut self,
        player_names: Option<Vec<String>>,
        positions: Option<Vec<Position>>,
        injury_filter: Option<InjuryStatusFilter>,
        roster_filter: Option<RosterStatusFilter>,
    ) -> Self {
        self.player_names = player_names;
        self.positions = positions;
        self.injury_status_filter = injury_filter;
        self.roster_status_filter = roster_filter;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_position_id() {
        assert_eq!(normalize_position_id(-1), 0);
        assert_eq!(normalize_position_id(0), 0);
        assert_eq!(normalize_position_id(1), 1);
        assert_eq!(normalize_position_id(2), 2);
    }

    #[test]
    fn test_position_id_to_string() {
        assert_eq!(position_id_to_string(-1), "UNKNOWN");
        assert_eq!(position_id_to_string(0), "QB");
        assert_eq!(position_id_to_string(2), "RB");
        assert_eq!(position_id_to_string(999), "UNKNOWN");
    }

    #[test]
    fn test_player_data_request_ext() {
        use crate::{LeagueId, Season, Week};

        let league_id = LeagueId::new(12345);
        let season = Season::new(2025);
        let week = Week::new(6);

        // Test fluent interface
        let request = PlayerDataRequest::for_projections(league_id, season, week)
            .with_filters(None, None, None, None);

        assert_eq!(request.league_id, league_id);
        assert_eq!(request.season, season);
        assert_eq!(request.week, week);

        // Test with some filters
        let player_names = Some(vec!["Josh Allen".to_string()]);
        let request = PlayerDataRequest::for_actual_data(league_id, season, week).with_filters(
            player_names.clone(),
            None,
            None,
            None,
        );

        assert_eq!(request.player_names, player_names);
    }
}
