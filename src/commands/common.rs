//! Common utilities and helper functions shared across commands.
//!
//! This module contains shared functionality that would otherwise be duplicated
//! across different command implementations.

use std::collections::BTreeMap;

use crate::{
    cli::types::{
        filters::{InjuryStatusFilter, RosterStatusFilter},
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
