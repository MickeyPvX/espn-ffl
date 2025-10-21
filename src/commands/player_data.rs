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
    cli::types::position::Position,
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::{get_player_data, update_player_points_with_roster_data, PlayerDataRequest},
        types::PlayerPoints,
    },
    storage::{PlayerDatabase, PlayerWeeklyStats},
    Result, Season, Week,
};

use super::{
    common::{CommandParams, CommandParamsBuilder},
    league_data::resolve_league_id,
    player_filters::{apply_status_filters, filter_and_convert_players},
};
use crate::espn::types::CachedPlayerData;
use rayon::prelude::*;

/// Configuration for player data retrieval.
#[derive(Debug)]
pub struct PlayerDataParams {
    pub base: CommandParams,
    pub projected: bool,
    pub debug: bool,
    pub clear_db: bool,
    pub refresh_positions: bool,
}

impl PlayerDataParams {
    /// Create new parameters with required fields.
    pub fn new(season: Season, week: Week, projected: bool) -> Self {
        Self {
            base: CommandParams::new(season, week),
            projected,
            debug: false,
            clear_db: false,
            refresh_positions: false,
        }
    }

    /// Set debug output.
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }
}

impl CommandParamsBuilder for PlayerDataParams {
    fn base_mut(&mut self) -> &mut CommandParams {
        &mut self.base
    }

    fn base(&self) -> &CommandParams {
        &self.base
    }
}

/// Retrieve and process player fantasy data for a given week.
///
/// Fetches player stats from ESPN API, calculates fantasy points using league settings,
/// and caches results in local database for performance.
pub async fn handle_player_data(params: PlayerDataParams) -> Result<()> {
    let league_id = resolve_league_id(params.base.league_id)?;
    println!("Connecting to database...");
    let mut db = PlayerDatabase::new()?;

    // Fetch week-specific roster data to match the player data we're querying
    let roster_data = match crate::espn::http::get_league_roster_data(
        false,
        league_id,
        params.base.season,
        Some(params.base.week),
        params.base.refresh,
    )
    .await
    {
        Ok((data, cache_status)) => {
            match cache_status {
                crate::espn::http::CacheStatus::Hit => {
                    println!(
                        "✓ Week {} roster status loaded (from cache)",
                        params.base.week.as_u16()
                    );
                }
                crate::espn::http::CacheStatus::Miss => {
                    println!(
                        "✓ Week {} roster status fetched (cache miss)",
                        params.base.week.as_u16()
                    );
                }
                crate::espn::http::CacheStatus::Refreshed => {
                    println!(
                        "✓ Week {} roster status fetched (refreshed)",
                        params.base.week.as_u16()
                    );
                }
            }
            Some(data)
        }
        Err(e) => {
            println!(
                "⚠ Could not fetch week {} roster data: {}",
                params.base.week.as_u16(),
                e
            );
            None
        }
    };

    // If clear_db flag is set, clear all database data first
    if params.clear_db {
        println!("Clearing all database data..."); // tarpaulin::skip
        db.clear_all_data()?;
        println!("✓ Database cleared successfully!"); // tarpaulin::skip
    }

    // Load or fetch league settings to compute points; cached for future runs.
    println!("Loading league scoring settings...");
    let settings = load_or_fetch_league_settings(league_id, false, params.base.season).await?;
    let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

    let mut player_points: Vec<PlayerPoints> = Vec::new();
    let mut stats_to_save: Vec<(PlayerWeeklyStats, PlayerPoints)> = Vec::new();

    // Check if we should use cached data (only if not forcing refresh)
    let use_cached = !params.base.refresh
        && params.base.player_names.is_none()
        && params.base.positions.is_none()
        && db.has_data_for_week(
            params.base.season,
            params.base.week,
            params.base.player_names.as_ref(),
            None,
            Some(params.projected),
        )?;

    if use_cached {
        println!(
            "Using cached player data for Season {} Week {}...",
            params.base.season.as_u16(),
            params.base.week.as_u16()
        );

        // Get cached data directly from database
        let cached_data = db.get_cached_player_data(&params.base, params.projected)?;

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
                        week: params.base.week,
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
            params.base.season.as_u16(),
            params.base.week.as_u16()
        );

        // tarpaulin::skip - HTTP call, tested via integration tests
        let positions_clone = params.base.positions.clone();
        let players_val = get_player_data(PlayerDataRequest {
            debug: params.debug,
            refresh: params.base.refresh,
            league_id,
            player_names: params.base.player_names.clone(),
            positions: params.base.positions.clone(),
            season: params.base.season,
            week: params.base.week,
            injury_status_filter: params.base.injury_status.clone(),
            roster_status_filter: params.base.roster_status.clone(),
        })
        .await?;

        // Deserialize directly into Vec<Player>
        let players: Vec<crate::espn::types::Player> = serde_json::from_value(players_val)?;
        println!(
            "Processing {} players and calculating fantasy points...",
            players.len()
        );
        let stat_source = if params.projected { 1 } else { 0 };

        // Phase 1: Store ALL players and process stats separately
        let filtered_players =
            filter_and_convert_players(players, params.base.player_names.clone(), positions_clone);

        // First, store all players regardless of whether they have stats
        let espn_players: Vec<crate::espn::types::Player> = filtered_players
            .iter()
            .map(|fp| fp.original_player.clone())
            .collect();
        let _ = db.update_players_from_espn(&espn_players);

        // Phase 2: Process stats for players who have them
        let processed_data: Vec<(PlayerWeeklyStats, PlayerPoints)> = filtered_players
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

                // Compute weekly stats and fantasy points only if player has stats
                if let Ok(player_value) = serde_json::to_value(&player) {
                    if let Some(weekly_stats) = select_weekly_stats(
                        &player_value,
                        params.base.season.as_u16(),
                        params.base.week.as_u16(),
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
                            season: params.base.season,
                            week: params.base.week,
                            projected_points: if params.projected { Some(points) } else { None },
                            actual_points: if !params.projected {
                                Some(points)
                            } else {
                                None
                            },
                            active: player.active,
                            injured: player.injured,
                            injury_status: player.injury_status.clone(),
                            is_rostered: None, // Will be updated later when roster data is applied
                            fantasy_team_id: None, // Will be updated later when roster data is applied
                            fantasy_team_name: None, // Will be updated later when roster data is applied
                            created_at: 0,           // Will be set by database
                            updated_at: 0,           // Will be set by database
                        };

                        let player_point = PlayerPoints::from_espn_player(
                            player_id,
                            &player,
                            position.clone(),
                            points,
                            params.base.week,
                            params.projected,
                        );

                        Some((weekly_db_stats, player_point))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // Phase 3: Collect PlayerPoints first
        for (_weekly_db_stats, player_point) in &processed_data {
            player_points.push(player_point.clone());
        }

        // Store processed_data for use outside the else block
        stats_to_save = processed_data;
    }

    println!(
        "✓ Found {} players with fantasy points",
        player_points.len()
    );

    // Check roster status for players using pre-fetched data BEFORE saving to database
    update_player_points_with_roster_data(
        &mut player_points,
        roster_data.as_ref(),
        true, // verbose
    );

    // Now save to database with correct roster information
    if !use_cached {
        for (mut weekly_db_stats, _player_point) in stats_to_save {
            // Find the corresponding updated player_points to get roster info
            if let Some(updated_player) = player_points
                .iter()
                .find(|p| p.id == weekly_db_stats.player_id)
            {
                weekly_db_stats.is_rostered = updated_player.is_rostered;
                weekly_db_stats.fantasy_team_id = updated_player.team_id;
                weekly_db_stats.fantasy_team_name = updated_player.team_name.clone();
            }
            let _ = db.upsert_weekly_stats(&weekly_db_stats, true);
        }
    }

    // Update database with roster information for ALL players (not just those with points)
    if let Some(ref league_data) = roster_data {
        match db.update_all_players_roster_info(league_data, params.base.season, params.base.week) {
            Ok(count) => println!("✓ Updated roster info for {} players", count),
            Err(e) => println!("⚠ Warning: Could not update roster info: {}", e),
        }
    }

    // Apply client-side filtering for specific injury statuses, roster status, and fantasy team
    if params.base.injury_status.is_some()
        || params.base.roster_status.is_some()
        || params.base.fantasy_team_filter.is_some()
    {
        apply_status_filters(
            &mut player_points,
            params.base.injury_status.as_ref(),
            params.base.roster_status.as_ref(),
            params.base.fantasy_team_filter.as_ref(),
        );
    }

    // Sort descending by points
    player_points.sort_by(|a, b| {
        b.points
            .partial_cmp(&a.points)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if params.base.as_json {
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
                player.id.as_i64(),
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
