//! Command implementations for ESPN Fantasy Football CLI

use std::io::Write;

use crate::{
    cache::league_settings_path,
    cli_types::{LeagueId, Position, Season, Week},
    database::{Player, PlayerDatabase, PlayerWeeklyStats},
    error::EspnError,
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::get_player_data,
        types::PlayerPoints,
    },
    Result, LEAGUE_ID_ENV_VAR,
};

/// Parameters for the player data command
#[derive(Debug)]
pub struct PlayerDataParams {
    pub debug: bool,
    pub as_json: bool,
    pub league_id: Option<LeagueId>,
    pub player_name: Option<String>,
    pub positions: Option<Vec<Position>>,
    pub projected: bool,
    pub season: Season,
    pub week: Week,
    pub refresh_positions: bool,
    pub clear_db: bool,
}

#[cfg(test)]
mod tests;

/// Handle the league data command
pub async fn handle_league_data(
    league_id: Option<LeagueId>,
    refresh: bool,
    season: Season,
    verbose: bool,
) -> Result<()> {
    let league_id = resolve_league_id(league_id)?;
    // tarpaulin::skip - HTTP/file I/O call, tested via integration tests
    let settings = load_or_fetch_league_settings(league_id, refresh, season).await?;

    if verbose {
        let path = league_settings_path(season.as_u16(), league_id.as_u32());
        eprintln!("Cached at: {}", path.display()); // tarpaulin::skip
                                                    // tarpaulin::skip - console output
        eprintln!(
            "Scoring items: {:?}",
            settings.scoring_settings.scoring_items
        );
    } else {
        println!("League settings successfully retrieved!"); // tarpaulin::skip
    }

    Ok(())
}

/// Handle the player data command
pub async fn handle_player_data(params: PlayerDataParams) -> Result<()> {
    let league_id = resolve_league_id(params.league_id)?;
    let mut db = PlayerDatabase::new()?;

    // If clear_db flag is set, clear all database data first
    if params.clear_db {
        println!("Clearing all database data..."); // tarpaulin::skip
        db.clear_all_data()?;
        println!("Database cleared successfully!"); // tarpaulin::skip
    }

    // If refresh_positions flag is set, fetch multiple weeks to update player positions
    if params.refresh_positions {
        println!("Refreshing player positions from multiple weeks..."); // tarpaulin::skip

        // Load league settings once for all weeks
        let settings = load_or_fetch_league_settings(league_id, false, params.season).await?;
        let _scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

        // Fetch data from weeks 1-18 to get comprehensive player position data
        for week_num in 1..=18 {
            let week = Week::new(week_num);

            // Fetch player data for this week (limit to recent relevant players)
            match get_player_data(
                false, // debug = false for batch operation
                league_id,
                Some(200), // Limit to top 200 players per week
                None,      // No name filter
                None,      // No position filter
                params.season,
                week,
            )
            .await
            {
                Ok(players_val) => {
                    if let Ok(players) =
                        serde_json::from_value::<Vec<crate::espn::types::Player>>(players_val)
                    {
                        // Update player records for this week (this will update positions)
                        for player in players {
                            // Skip team placeholder entries (position 15)
                            if player.default_position_id == 15 {
                                continue;
                            }

                            let position = if player.default_position_id < 0 {
                                "UNKNOWN".to_string()
                            } else {
                                Position::try_from(player.default_position_id as u8)
                                    .map(|p| p.to_string())
                                    .unwrap_or_else(|_| "UNKNOWN".to_string())
                            };

                            let db_player = crate::database::Player {
                                player_id: player.id,
                                name: player
                                    .full_name
                                    .clone()
                                    .unwrap_or_else(|| format!("Player {}", player.id.as_u64())),
                                position: position.clone(),
                                team: None,
                            };
                            let _ = db.upsert_player(&db_player); // Update player with correct position
                        }
                        print!("."); // Progress indicator
                        Write::flush(&mut std::io::stdout()).unwrap();
                    }
                }
                Err(_) => {
                    // Skip weeks that fail (likely future weeks with no data)
                    continue;
                }
            }
        }
        println!("\nPlayer positions refreshed successfully!"); // tarpaulin::skip
        return Ok(());
    }

    // Load or fetch league settings to compute points; cached for future runs.
    // tarpaulin::skip - HTTP/file I/O call, tested via integration tests
    let settings = load_or_fetch_league_settings(league_id, false, params.season).await?;
    let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

    // Check if we already have comprehensive data for this week (without filters)
    // Only skip API call if we have a substantial amount of general data and no specific filters
    let skip_api_call = params.player_name.is_none()
        && params.positions.is_none()
        && db.has_data_for_week(params.season, params.week, None, None)?;

    let players_val = if skip_api_call {
        if params.debug {
            println!(
                "Using cached data for Season {} Week {}",
                params.season.as_u16(),
                params.week.as_u16()
            );
        }
        // Return empty JSON array to skip processing new data but continue with displaying cached data
        serde_json::Value::Array(vec![])
    } else {
        // tarpaulin::skip - HTTP call, tested via integration tests
        get_player_data(
            params.debug,
            league_id,
            None, // Don't pass limit to ESPN API
            params.player_name,
            params.positions,
            params.season,
            params.week,
        )
        .await?
    };

    // Deserialize directly into Vec<Player>
    let players: Vec<crate::espn::types::Player> = serde_json::from_value(players_val)?;
    let stat_source = if params.projected { 1 } else { 0 };

    let mut player_points: Vec<PlayerPoints> = Vec::new();

    for player in players {
        // Skip team placeholder entries (position 15 = team aggregates like "Bills TQB")
        // These duplicate individual player stats and aren't useful for fantasy analysis
        if player.default_position_id == 15 {
            continue;
        }

        // Convert position ID to string using existing Position type
        // Handle negative position IDs (like -1) by treating them as unknown
        let position = if player.default_position_id < 0 {
            "UNKNOWN".to_string()
        } else {
            Position::try_from(player.default_position_id as u8)
                .map(|p| p.to_string())
                .unwrap_or_else(|_| "UNKNOWN".to_string())
        };

        // Store player info in database
        let db_player = Player {
            player_id: player.id,
            name: player
                .full_name
                .clone()
                .unwrap_or_else(|| format!("Player {}", player.id.as_u64())),
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
            // Handle negative position IDs by using 0 (QB) as fallback for scoring
            let position_id = if player.default_position_id < 0 {
                0u8 // Default to QB position for scoring purposes
            } else {
                player.default_position_id as u8
            };
            let points = compute_points_for_week(weekly_stats, position_id, &scoring_index);

            // Store weekly stats in database
            let weekly_db_stats = PlayerWeeklyStats {
                player_id: player.id,
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
                    id: player.id,
                    name: player
                        .full_name
                        .unwrap_or_else(|| format!("Player {}", player.id.as_u64())),
                    position: position.clone(),
                    week: params.week,
                    projected: params.projected,
                    points,
                });
            }
        }
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

/// Handle the projection analysis command
pub async fn handle_projection_analysis(
    season: Season,
    week: Week,
    league_id: Option<LeagueId>,
    player_name: Option<String>,
    positions: Option<Vec<Position>>,
    as_json: bool,
) -> Result<()> {
    let league_id = resolve_league_id(league_id)?;
    let db = PlayerDatabase::new()?;

    // Check if we already have projected data for this week (without filters)
    // Only skip API call if we have comprehensive data and no specific filters
    let skip_api_call = player_name.is_none()
        && positions.is_none()
        && db.has_data_for_week(season, week, None, None)?;

    // Fetch ESPN projections for the target week
    let players_val = if skip_api_call {
        // Return empty JSON array to skip processing new data but continue with cached analysis
        serde_json::Value::Array(vec![])
    } else {
        get_player_data(
            false, // debug = false
            league_id,
            None, // Don't pass limit to ESPN API - we'll limit results after processing
            player_name.clone(),
            positions.clone(),
            season,
            week,
        )
        .await?
    };

    let players: Vec<crate::espn::types::Player> = serde_json::from_value(players_val)?;

    // Load league settings to compute ESPN projections
    let settings = load_or_fetch_league_settings(league_id, false, season).await?;
    let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

    let mut projected_points_data = Vec::new();

    // Calculate ESPN projections for each player
    for player in players {
        // Skip team placeholder entries
        if player.default_position_id == 15 {
            continue;
        }

        // Apply name filter if specified
        if let Some(name_filter) = &player_name {
            if let Some(full_name) = &player.full_name {
                if !full_name
                    .to_lowercase()
                    .contains(&name_filter.to_lowercase())
                {
                    continue;
                }
            } else {
                continue; // Skip players without names when name filter is active
            }
        }

        if let Some(weekly_stats) = select_weekly_stats(
            &serde_json::to_value(&player)?,
            season.as_u16(),
            week.as_u16(),
            1, // stat_source = 1 for projected
        ) {
            let position_id = if player.default_position_id < 0 {
                0u8
            } else {
                player.default_position_id as u8
            };
            let espn_projection =
                compute_points_for_week(weekly_stats, position_id, &scoring_index);

            if espn_projection > 0.0 {
                projected_points_data.push((player.id, espn_projection));
            }
        }
    }

    // Get performance estimates using historical data
    let estimates = db.estimate_week_performance(season, week, &projected_points_data, None)?;

    if estimates.is_empty() {
        println!("No projection data available for week {}.", week.as_u16()); // tarpaulin::skip
        println!("Make sure to fetch historical data for previous weeks first."); // tarpaulin::skip
        return Ok(());
    }

    // Apply position filter and sort by estimated points (descending)
    let mut filtered_estimates: Vec<_> = estimates
        .into_iter()
        .filter(|estimate| {
            if let Some(pos_filters) = &positions {
                pos_filters
                    .iter()
                    .any(|p| estimate.position == p.to_string())
            } else {
                true
            }
        })
        .collect();

    // Sort by estimated_points in descending order
    filtered_estimates.sort_by(|a, b| {
        b.estimated_points
            .partial_cmp(&a.estimated_points)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if as_json {
        println!("{}", serde_json::to_string_pretty(&filtered_estimates)?); // tarpaulin::skip
    } else {
        // tarpaulin::skip - console output
        println!(
            "Projection Analysis & Predictions for Week {}",
            week.as_u16()
        );
        println!("Season: {}", season.as_u16());
        println!();
        println!(
            "{:<20} {:<8} {:<8} {:<8} {:<8} {:<8}",
            "Player", "Position", "ESPN", "Bias", "Adjusted", "Conf%"
        );
        println!("{}", "-".repeat(75));

        for estimate in filtered_estimates {
            // Find corresponding ESPN projection
            let espn_proj = projected_points_data
                .iter()
                .find(|(id, _)| *id == estimate.player_id)
                .map(|(_, proj)| *proj)
                .unwrap_or(0.0);

            let bias = espn_proj - estimate.estimated_points;
            let confidence_pct = (estimate.confidence * 100.0) as u8;

            println!(
                "{:<20} {:<8} {:<8.1} {:<+8.1} {:<8.1} {:<8}%",
                estimate.name.chars().take(20).collect::<String>(),
                estimate.position,
                espn_proj,
                bias,
                estimate.estimated_points,
                confidence_pct
            );
        }

        println!();
        println!("ESPN = ESPN's projected points");
        println!("Bias = ESPN projection - our adjustment (+overestimate, -underestimate)");
        println!("Adjusted = our calculated points based on historical bias");
        println!("Conf% = confidence level based on data quality and consistency");
    }

    Ok(())
}

/// Resolve league ID from option or environment variable
fn resolve_league_id(league_id: Option<LeagueId>) -> Result<LeagueId> {
    league_id
        .or_else(|| {
            std::env::var(LEAGUE_ID_ENV_VAR)
                .ok()
                .and_then(|s| s.parse::<LeagueId>().ok())
        })
        .ok_or_else(|| EspnError::MissingLeagueId {
            env_var: LEAGUE_ID_ENV_VAR.to_string(),
        })
}
