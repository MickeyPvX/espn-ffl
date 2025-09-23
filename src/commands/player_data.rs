//! Player data command implementation

use crate::{
    cli::types::{LeagueId, PlayerId, Position, Season, Week},
    storage::{Player, PlayerDatabase, PlayerWeeklyStats},
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::get_player_data,
        types::PlayerPoints,
    },
    Result,
};

use super::resolve_league_id;

/// Parameters for the player data command
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
}

/// Handle the player data command (simplified version)
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
        && db.has_data_for_week(params.season, params.week, params.player_name.as_ref(), None, Some(params.projected))?;

    if use_cached {
        println!("Using cached player data for Season {} Week {}...", params.season.as_u16(), params.week.as_u16());

        // Get cached data directly from database
        let cached_data = db.get_cached_player_data(
            params.season,
            params.week,
            params.player_name.as_ref(),
            params.positions.as_ref(),
            params.projected,
        )?;

        // Convert cached data to PlayerPoints format
        for (player_id, name, position, points) in cached_data {
            player_points.push(PlayerPoints {
                id: player_id,
                name,
                position,
                points,
                week: params.week,
                projected: params.projected,
            });
        }
    } else {
        println!("Fetching fresh player data from ESPN for Season {} Week {}...", params.season.as_u16(), params.week.as_u16());

        // tarpaulin::skip - HTTP call, tested via integration tests
        let players_val = get_player_data(
            params.debug,
            league_id,
            None, // Don't pass limit to ESPN API
            params.player_name.clone(),
            params.positions,
            params.season,
            params.week,
        )
        .await?;

        // Deserialize directly into Vec<Player>
        let players: Vec<crate::espn::types::Player> = serde_json::from_value(players_val)?;
        println!("Processing {} players and calculating fantasy points...", players.len());
        let stat_source = if params.projected { 1 } else { 0 };

        for player in players {
            // Skip invalid player IDs, team placeholder entries, and individual defensive players
            if player.id < 0
                || player.default_position_id == 15
                || (player.default_position_id >= 8 && player.default_position_id <= 15) {
                continue;
            }

            // Apply local player name filtering for multiple names
            if let Some(names) = &params.player_name {
                if names.len() > 1 {
                    let player_name = player.full_name.as_deref().unwrap_or("");
                    let matches = names.iter().any(|name|
                        player_name.to_lowercase().contains(&name.to_lowercase())
                    );
                    if !matches {
                        continue;
                    }
                }
            }

            let player_id = PlayerId::new(player.id as u64);

            let position = if player.default_position_id < 0 {
                "UNKNOWN".to_string()
            } else {
                Position::try_from(player.default_position_id as u8)
                    .map(|p| p.to_string())
                    .unwrap_or_else(|_| "UNKNOWN".to_string())
            };

            // Store player info in database
            let db_player = Player {
                player_id: player_id,
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
                    player_id: player_id,
                    season: params.season,
                    week: params.week,
                    projected_points: if params.projected { Some(points) } else { None },
                    actual_points: if !params.projected {
                        Some(points)
                    } else {
                        None
                    },
                    created_at: 0, // Will be set by database
                    updated_at: 0, // Will be set by database
                };
                let _ = db.merge_weekly_stats(&weekly_db_stats);

                if points > 0f64 {
                    player_points.push(PlayerPoints {
                        id: player_id,
                        name: player
                            .full_name
                            .unwrap_or_else(|| format!("Player {}", player.id)),
                        position: position.clone(),
                        week: params.week,
                        projected: params.projected,
                        points,
                    });
                }
            }
        }
    }

    println!("✓ Found {} players with fantasy points", player_points.len());

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
            println!(
                "{} {} ({}) [week {}] {:.2}",
                player.id.as_u64(),
                player.name,
                player.position,
                player.week.as_u16(),
                player.points,
            );
        }
    }

    Ok(())
}