//! Player data retrieval and fantasy scoring calculation.
//!
//! This module handles fetching player statistics from ESPN's API, calculating
//! fantasy points based on league scoring settings, and managing local database
//! storage for performance.
//!
//! # Key Features
//!
//! - **Efficient Filtering**: Server-side filtering by injury status and position
//! - **Scoring Calculation**: Converts raw stats to fantasy points using league settings
//! - **Database Caching**: Stores results locally to avoid repeated API calls
//! - **Roster Status**: Checks which players are rostered in your league
//! - **Flexible Output**: Support for both human-readable and JSON output
//!
//! # Usage
//!
//! The main entry point is [`handle_player_data`] which accepts a [`PlayerDataParams`]
//! struct containing all configuration options.

use crate::{
    cli::types::{
        filters::{FantasyTeamFilter, InjuryStatusFilter, RosterStatusFilter},
        position::Position,
    },
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::{
            fetch_current_roster_data, get_player_data, update_player_points_with_roster_data,
            PlayerDataRequest,
        },
        types::PlayerPoints,
    },
    storage::{Player, PlayerDatabase, PlayerWeeklyStats},
    LeagueId, Result, Season, Week,
};

use super::{
    player_filters::{apply_status_filters, filter_and_convert_players},
    resolve_league_id,
};
use crate::espn::types::CachedPlayerData;
use rayon::prelude::*;

/// Configuration for player data retrieval.
#[derive(Debug)]
pub struct PlayerDataParams {
    pub league_id: Option<LeagueId>,
    pub season: Season,
    pub week: Week,
    pub projected: bool,
    pub debug: bool,
    pub as_json: bool,
    pub player_name: Option<Vec<String>>,
    pub positions: Option<Vec<Position>>,
    pub injury_status: Option<InjuryStatusFilter>,
    pub roster_status: Option<RosterStatusFilter>,
    pub fantasy_team_filter: Option<FantasyTeamFilter>,
    pub refresh: bool,
    pub clear_db: bool,
    pub refresh_positions: bool,
}

impl PlayerDataParams {
    /// Create new parameters with required fields.
    pub fn new(season: Season, week: Week, projected: bool) -> Self {
        Self {
            league_id: None,
            season,
            week,
            projected,
            debug: false,
            as_json: false,
            player_name: None,
            positions: None,
            injury_status: None,
            roster_status: None,
            fantasy_team_filter: None,
            refresh: false,
            clear_db: false,
            refresh_positions: false,
        }
    }

    /// Set league ID.
    pub fn with_league_id(mut self, league_id: LeagueId) -> Self {
        self.league_id = Some(league_id);
        self
    }

    /// Filter by specific player names.
    pub fn with_player_names(mut self, names: Vec<String>) -> Self {
        self.player_name = Some(names);
        self
    }

    /// Filter by positions.
    pub fn with_positions(mut self, positions: Vec<Position>) -> Self {
        self.positions = Some(positions);
        self
    }

    /// Enable debug output.
    pub fn with_debug(mut self) -> Self {
        self.debug = true;
        self
    }

    /// Output as JSON.
    pub fn with_json_output(mut self) -> Self {
        self.as_json = true;
        self
    }

    /// Force refresh from API.
    pub fn with_refresh(mut self) -> Self {
        self.refresh = true;
        self
    }
}

/// Retrieve and process player fantasy data for a given week.
///
/// Fetches player stats from ESPN API, calculates fantasy points using league settings,
/// and caches results in local database for performance.
pub async fn handle_player_data(params: PlayerDataParams) -> Result<()> {
    let league_id = resolve_league_id(params.league_id)?;
    println!("Connecting to database...");
    let mut db = PlayerDatabase::new()?;

    // Fetch current roster data once for efficient reuse
    let roster_data = fetch_current_roster_data(league_id, params.season, true).await?;

    // If clear_db flag is set, clear all database data first
    if params.clear_db {
        println!("Clearing all database data..."); // tarpaulin::skip
        db.clear_all_data()?;
        println!("✓ Database cleared successfully!"); // tarpaulin::skip
    }

    // Load or fetch league settings to compute points; cached for future runs.
    println!("Loading league scoring settings...");
    let settings = load_or_fetch_league_settings(league_id, false, params.season).await?;
    let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

    let mut player_points: Vec<PlayerPoints> = Vec::new();

    // Check if we should use cached data (only if not forcing refresh)
    let use_cached = !params.refresh
        && params.player_name.is_none()
        && params.positions.is_none()
        && db.has_data_for_week(
            params.season,
            params.week,
            params.player_name.as_ref(),
            None,
            Some(params.projected),
        )?;

    if use_cached {
        println!(
            "Using cached player data for Season {} Week {}...",
            params.season.as_u16(),
            params.week.as_u16()
        );

        // Get cached data directly from database
        let cached_data = db.get_cached_player_data(
            params.season,
            params.week,
            params.player_name.as_ref(),
            params.positions.as_ref(),
            params.projected,
        )?;

        // Convert cached data to PlayerPoints format with status info in parallel
        let cached_player_points: Vec<PlayerPoints> = cached_data
            .into_par_iter()
            .map(
                |(
                    player_id,
                    name,
                    position,
                    points,
                    active,
                    injured,
                    injury_status,
                    is_rostered,
                    team_id,
                    team_name,
                )| {
                    PlayerPoints::from_cached_data(CachedPlayerData {
                        player_id,
                        name,
                        position,
                        points,
                        week: params.week,
                        projected: params.projected,
                        active,
                        injured,
                        injury_status,
                        is_rostered,
                        team_id,
                        team_name,
                    })
                },
            )
            .collect();

        player_points.extend(cached_player_points);
    } else {
        println!(
            "Fetching fresh player data from ESPN for Season {} Week {}...",
            params.season.as_u16(),
            params.week.as_u16()
        );

        // tarpaulin::skip - HTTP call, tested via integration tests
        let positions_clone = params.positions.clone();
        let players_val = get_player_data(PlayerDataRequest {
            debug: params.debug,
            league_id,
            player_names: params.player_name.clone(),
            positions: params.positions.clone(),
            season: params.season,
            week: params.week,
            injury_status_filter: params.injury_status.clone(),
            roster_status_filter: params.roster_status.clone(),
        })
        .await?;

        // Debug: Print raw ESPN response structure for one player to understand data structure
        if params.debug {
            if let Some(first_player) = players_val.as_array().and_then(|arr| arr.first()) {
                eprintln!("DEBUG: Raw ESPN player data structure:");
                eprintln!("{}", serde_json::to_string_pretty(first_player)?);
                eprintln!("--- End raw data ---");
            }
        }

        // Deserialize directly into Vec<Player>
        let players: Vec<crate::espn::types::Player> = serde_json::from_value(players_val)?;
        println!(
            "Processing {} players and calculating fantasy points...",
            players.len()
        );
        let stat_source = if params.projected { 1 } else { 0 };

        // Phase 1: Process all players in parallel for CPU-intensive computations
        let processed_data: Vec<(Player, PlayerWeeklyStats, PlayerPoints)> =
            filter_and_convert_players(players, params.player_name.clone(), positions_clone)
                .into_par_iter()
                .filter_map(|filtered_player| {
                    let player = filtered_player.original_player;
                    let player_id = filtered_player.player_id;

                    let position = if player.default_position_id < 0 {
                        "UNKNOWN".to_string()
                    } else {
                        Position::try_from(player.default_position_id as u8)
                            .map(|p| p.to_string())
                            .unwrap_or_else(|_| "UNKNOWN".to_string())
                    };

                    // Prepare player info for database
                    let db_player = Player {
                        player_id,
                        name: player
                            .full_name
                            .clone()
                            .unwrap_or_else(|| format!("Player {}", player.id)),
                        position: position.clone(),
                        team: None, // ESPN API doesn't provide team in this format
                    };

                    // Compute weekly stats and fantasy points
                    if let Ok(player_value) = serde_json::to_value(&player) {
                        if let Some(weekly_stats) = select_weekly_stats(
                            &player_value,
                            params.season.as_u16(),
                            params.week.as_u16(),
                            stat_source,
                        ) {
                            let position_id = if player.default_position_id < 0 {
                                0u8 // Default to QB position for scoring purposes
                            } else {
                                player.default_position_id as u8
                            };
                            let points =
                                compute_points_for_week(weekly_stats, position_id, &scoring_index);

                            let weekly_db_stats = PlayerWeeklyStats {
                                player_id,
                                season: params.season,
                                week: params.week,
                                projected_points: if params.projected {
                                    Some(points)
                                } else {
                                    None
                                },
                                actual_points: if !params.projected {
                                    Some(points)
                                } else {
                                    None
                                },
                                active: player.active,
                                injured: player.injured,
                                injury_status: player.injury_status.clone(),
                                is_rostered: None, // Will be filled later after roster check
                                fantasy_team_id: None, // Will be filled later after roster check
                                fantasy_team_name: None, // Will be filled later after roster check
                                created_at: 0,     // Will be set by database
                                updated_at: 0,     // Will be set by database
                            };

                            let player_point = PlayerPoints::from_espn_player(
                                player_id,
                                &player,
                                position.clone(),
                                points,
                                params.week,
                                params.projected,
                            );

                            Some((db_player, weekly_db_stats, player_point))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();

        // Phase 2: Write results to database sequentially (SQLite requires sequential access)
        for (db_player, weekly_db_stats, player_point) in processed_data {
            let _ = db.upsert_player(&db_player);
            let _ = db.merge_weekly_stats(&weekly_db_stats);
            player_points.push(player_point);
        }
    }

    println!(
        "✓ Found {} players with fantasy points",
        player_points.len()
    );

    // Check roster status for players using pre-fetched data
    update_player_points_with_roster_data(
        &mut player_points,
        roster_data.as_ref(),
        true, // verbose
    );

    // Update database with roster information
    if !player_points.is_empty() {
        for player in &player_points {
            let updated_stats = PlayerWeeklyStats {
                player_id: player.id,
                season: params.season,
                week: params.week,
                projected_points: if params.projected {
                    Some(player.points)
                } else {
                    None
                },
                actual_points: if !params.projected {
                    Some(player.points)
                } else {
                    None
                },
                active: player.active,
                injured: player.injured,
                injury_status: player.injury_status.clone(),
                is_rostered: player.is_rostered,
                fantasy_team_id: player.team_id,
                fantasy_team_name: player.team_name.clone(),
                created_at: 0, // Will be set by database
                updated_at: 0, // Will be set by database
            };
            let _ = db.merge_weekly_stats(&updated_stats);
        }
    }

    // Apply client-side filtering for specific injury statuses, roster status, and fantasy team
    if params.injury_status.is_some()
        || params.roster_status.is_some()
        || params.fantasy_team_filter.is_some()
    {
        apply_status_filters(
            &mut player_points,
            params.injury_status.as_ref(),
            params.roster_status.as_ref(),
            params.fantasy_team_filter.as_ref(),
        );
    }

    // Sort descending by points
    player_points.sort_by(|a, b| {
        b.points
            .partial_cmp(&a.points)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if params.as_json {
        println!("{}", serde_json::to_string_pretty(&player_points)?); // tarpaulin::skip
    } else {
        for player in player_points {
            // tarpaulin::skip - console output
            let status_str = match (&player.injury_status, player.injured) {
                (Some(status), _) => format!("[{}]", status),
                (None, Some(true)) => "[Injured]".to_string(),
                (None, Some(false)) => "[Active]".to_string(),
                (None, None) => "[Active]".to_string(),
            };

            let roster_str = match (player.is_rostered, &player.team_name) {
                (Some(true), Some(team_name)) => format!(" ({})", team_name),
                (Some(true), None) => "(Rostered)".to_string(),
                (Some(false), _) => "(FA)".to_string(),
                (None, _) => "".to_string(),
            };

            println!(
                "{} {} ({}) [week {}] {} {} {:.2}",
                player.id.as_u64(),
                player.name,
                player.position,
                player.week.as_u16(),
                status_str,
                roster_str,
                player.points,
            );
        }
    }

    Ok(())
}
