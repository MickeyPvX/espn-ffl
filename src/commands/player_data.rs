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
    cli::types::{InjuryStatusFilter, LeagueId, Position, RosterStatusFilter, Season, Week},
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::{get_player_data, update_player_points_with_roster_info, PlayerDataRequest},
        types::PlayerPoints,
    },
    storage::{Player, PlayerDatabase, PlayerWeeklyStats},
    Result,
};

use super::{player_filters::filter_and_convert_players, resolve_league_id};
use crate::espn::types::{CachedPlayerData, InjuryStatus};

/// Configuration parameters for player data retrieval.
///
/// Contains all options for filtering, formatting, and caching player data.
/// Used by the [`handle_player_data`] function to customize behavior.
///
/// # Examples
///
/// ```rust
/// use espn_ffl::{LeagueId, Season, Week, Position, commands::player_data::PlayerDataParams};
///
/// let params = PlayerDataParams {
///     league_id: Some(LeagueId::new(123456)),
///     season: Season::new(2025),
///     week: Week::new(1),
///     positions: Some(vec![Position::QB, Position::RB]),
///     projected: false, // Get actual stats, not projections
///     debug: false,
///     as_json: false,
///     // ... other fields
/// #   player_name: None,
/// #   refresh_positions: false,
/// #   clear_db: false,
/// #   refresh: false,
/// #   injury_status: None,
/// #   roster_status: None,
/// };
/// ```
#[derive(Debug)]
pub struct PlayerDataParams {
    pub debug: bool,
    pub as_json: bool,
    pub league_id: Option<LeagueId>,
    pub player_name: Option<Vec<String>>,
    pub positions: Option<Vec<Position>>,
    pub projected: bool,
    pub season: Season,
    pub week: Week,
    pub refresh_positions: bool,
    pub clear_db: bool,
    pub refresh: bool,
    pub injury_status: Option<InjuryStatusFilter>,
    pub roster_status: Option<RosterStatusFilter>,
}

/// Retrieve and process player fantasy data for a given week.
///
/// This is the main entry point for player data operations. It handles:
///
/// 1. **Database Management**: Connects to local SQLite database for caching
/// 2. **League Settings**: Loads scoring configuration from ESPN API
/// 3. **Data Retrieval**: Fetches player stats using efficient server-side filtering
/// 4. **Scoring Calculation**: Converts raw stats to fantasy points
/// 5. **Roster Status**: Determines which players are on fantasy rosters
/// 6. **Output Formatting**: Displays results in human-readable or JSON format
///
/// # Performance Optimization
///
/// - Uses database caching to avoid redundant API calls
/// - Applies server-side filtering where possible (injury status, positions)
/// - Only fetches fresh data when explicitly requested via `refresh` flag
///
/// # Examples
///
/// ```rust,no_run
/// use espn_ffl::{LeagueId, Season, Week, Position, commands::player_data::*};
///
/// # async fn example() -> espn_ffl::Result<()> {
/// let params = PlayerDataParams {
///     league_id: Some(LeagueId::new(123456)),
///     season: Season::new(2025),
///     week: Week::new(1),
///     positions: Some(vec![Position::QB]),
///     projected: false,
/// #   debug: false,
/// #   as_json: false,
/// #   player_name: None,
/// #   refresh_positions: false,
/// #   clear_db: false,
/// #   refresh: false,
/// #   injury_status: None,
/// #   roster_status: None,
///     // ... other fields
/// };
///
/// handle_player_data(params).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - League ID is missing and not set in environment
/// - Database connection fails
/// - ESPN API is unavailable or returns invalid data
/// - League settings cannot be loaded
pub async fn handle_player_data(params: PlayerDataParams) -> Result<()> {
    let league_id = resolve_league_id(params.league_id)?;
    println!("Connecting to database...");
    let mut db = PlayerDatabase::new()?;

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

        // Convert cached data to PlayerPoints format with status info
        for (
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
        ) in cached_data
        {
            player_points.push(PlayerPoints::from_cached_data(CachedPlayerData {
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
            }));
        }
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

        for filtered_player in
            filter_and_convert_players(players, params.player_name.clone(), positions_clone)
        {
            let player = filtered_player.original_player;
            let player_id = filtered_player.player_id;

            let position = if player.default_position_id < 0 {
                "UNKNOWN".to_string()
            } else {
                Position::try_from(player.default_position_id as u8)
                    .map(|p| p.to_string())
                    .unwrap_or_else(|_| "UNKNOWN".to_string())
            };

            // Store player info in database
            let db_player = Player {
                player_id,
                name: player
                    .full_name
                    .clone()
                    .unwrap_or_else(|| format!("Player {}", player.id)),
                position: position.clone(),
                team: None, // ESPN API doesn't provide team in this format
            };
            let _ = db.upsert_player(&db_player);

            if let Some(weekly_stats) = select_weekly_stats(
                &serde_json::to_value(&player)?,
                params.season.as_u16(),
                params.week.as_u16(),
                stat_source,
            ) {
                let position_id = if player.default_position_id < 0 {
                    0u8 // Default to QB position for scoring purposes
                } else {
                    player.default_position_id as u8
                };
                let points = compute_points_for_week(weekly_stats, position_id, &scoring_index);

                // Store weekly stats in database
                let weekly_db_stats = PlayerWeeklyStats {
                    player_id,
                    season: params.season,
                    week: params.week,
                    projected_points: if params.projected { Some(points) } else { None },
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
                let _ = db.merge_weekly_stats(&weekly_db_stats);

                if points > 0f64 {
                    player_points.push(PlayerPoints::from_espn_player(
                        player_id,
                        &player,
                        position.clone(),
                        points,
                        params.week,
                        params.projected,
                    ));
                }
            }
        }
    }

    println!(
        "✓ Found {} players with fantasy points",
        player_points.len()
    );

    // Check roster status for players
    update_player_points_with_roster_info(
        &mut player_points,
        league_id,
        params.season,
        params.week,
        true, // verbose
    )
    .await?;

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

    // Apply client-side filtering for specific injury statuses and roster status
    if params.injury_status.is_some() || params.roster_status.is_some() {
        player_points = apply_client_side_filters(player_points, &params);
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

/// Apply client-side filters for injury and roster status
fn apply_client_side_filters(
    mut player_points: Vec<PlayerPoints>,
    params: &PlayerDataParams,
) -> Vec<PlayerPoints> {
    // Apply injury status filter
    if let Some(injury_filter) = &params.injury_status {
        player_points.retain(|player| {
            match injury_filter {
                InjuryStatusFilter::Active => {
                    // For client-side filtering, we check if the player is active
                    matches!(player.injury_status, Some(InjuryStatus::Active)) ||
                    (player.injury_status.is_none() && player.injured != Some(true))
                }
                InjuryStatusFilter::Injured => {
                    // Already filtered server-side, but double-check
                    player.injured == Some(true) ||
                    matches!(&player.injury_status, Some(status) if *status != InjuryStatus::Active)
                }
                InjuryStatusFilter::Out => {
                    matches!(player.injury_status, Some(InjuryStatus::Out))
                }
                InjuryStatusFilter::Doubtful => {
                    matches!(player.injury_status, Some(InjuryStatus::Doubtful))
                }
                InjuryStatusFilter::Questionable => {
                    matches!(player.injury_status, Some(InjuryStatus::Questionable))
                }
                InjuryStatusFilter::Probable => {
                    matches!(player.injury_status, Some(InjuryStatus::Probable))
                }
                InjuryStatusFilter::DayToDay => {
                    matches!(player.injury_status, Some(InjuryStatus::DayToDay))
                }
                InjuryStatusFilter::IR => {
                    matches!(player.injury_status, Some(InjuryStatus::InjuryReserve))
                }
            }
        });
    }

    // Apply roster status filter (always client-side since ESPN doesn't support it)
    if let Some(roster_filter) = &params.roster_status {
        player_points.retain(|player| match roster_filter {
            RosterStatusFilter::Rostered => player.is_rostered == Some(true),
            RosterStatusFilter::FA => player.is_rostered == Some(false),
        });
    }

    player_points
}
