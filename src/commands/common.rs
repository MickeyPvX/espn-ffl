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
        http::get_league_roster_data,
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

/// Map a player's `defaultPositionId` to the lineup slot id used to look up scoring overrides.
///
/// `pointsOverrides` in the league settings is keyed by *lineup slot* id, not by
/// `defaultPositionId`, so the two must be translated rather than passed through. Players whose
/// position does not resolve fall back to the QB slot, which carries no overrides in practice.
pub fn scoring_slot_id(default_position_id: i32) -> u8 {
    const FALLBACK_SLOT: u8 = 0;

    if default_position_id < 0 {
        return FALLBACK_SLOT;
    }

    Position::from_default_position_id(default_position_id as u8)
        .ok()
        .and_then(|p| p.lineup_slot_ids().first().copied())
        .unwrap_or(FALLBACK_SLOT)
}

/// Convert player's default_position_id to a position string
pub fn position_id_to_string(default_position_id: i32) -> String {
    if default_position_id < 0 {
        "UNKNOWN".to_string()
    } else {
        Position::from_default_position_id(default_position_id as u8)
            .map(|p| p.to_string())
            .unwrap_or_else(|_| "UNKNOWN".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scoring_slot_id_translates_position_to_slot() {
        // D/ST is 16 in both spaces, which is where this league's overrides live.
        assert_eq!(scoring_slot_id(16), 16);
        // QB is position 1 but slot 0 — the translation that a passthrough would miss.
        assert_eq!(scoring_slot_id(1), 0);
        assert_eq!(scoring_slot_id(3), 4); // WR: position 3 -> slot 4
        assert_eq!(scoring_slot_id(4), 6); // TE: position 4 -> slot 6
        assert_eq!(scoring_slot_id(5), 17); // K: position 5 -> slot 17
                                            // Unresolvable positions fall back to the override-free QB slot.
        assert_eq!(scoring_slot_id(-1), 0);
        assert_eq!(scoring_slot_id(999), 0);
    }

    #[test]
    fn test_position_id_to_string() {
        assert_eq!(position_id_to_string(-1), "UNKNOWN");
        assert_eq!(position_id_to_string(1), "QB");
        assert_eq!(position_id_to_string(2), "RB");
        assert_eq!(position_id_to_string(16), "D/ST");
        assert_eq!(position_id_to_string(999), "UNKNOWN");
    }
}
