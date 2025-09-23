//! Projection analysis command implementation

use crate::{
    cli::types::{LeagueId, PlayerId, Position, Season, Week},
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::get_player_data,
    },
    storage::PlayerDatabase,
    Result,
};

use super::resolve_league_id;

/// Handle the projection analysis command (simplified version)
#[allow(clippy::too_many_arguments)]
pub async fn handle_projection_analysis(
    season: Season,
    week: Week,
    league_id: Option<LeagueId>,
    player_names: Option<Vec<String>>,
    positions: Option<Vec<Position>>,
    as_json: bool,
    refresh: bool,
    bias_strength: f64,
) -> Result<()> {
    let league_id = resolve_league_id(league_id)?;
    if !as_json {
        println!("Connecting to database...");
    }
    let db = PlayerDatabase::new()?;

    // Check if we already have projected data for this week (without filters)
    let skip_api_call = !refresh
        && player_names.is_none()
        && positions.is_none()
        && db.has_data_for_week(season, week, player_names.as_ref(), None, Some(true))?; // Check for projected data

    // Fetch ESPN projections for the target week
    let players_val = if skip_api_call {
        if !as_json {
            println!("Using cached projection data for analysis...");
        }
        // Return empty JSON array to skip processing new data but continue with cached analysis
        serde_json::Value::Array(vec![])
    } else {
        if !as_json {
            println!(
                "Fetching fresh ESPN projections for Week {}...",
                week.as_u16()
            );
        }
        get_player_data(
            false, // debug = false
            league_id,
            None, // Don't pass limit to ESPN API
            player_names.clone(),
            positions.clone(),
            season,
            week,
        )
        .await?
    };

    let players: Vec<crate::espn::types::Player> = serde_json::from_value(players_val)?;

    // Load league settings to compute ESPN projections
    if !as_json {
        println!("Loading league scoring settings...");
    }
    let settings = load_or_fetch_league_settings(league_id, false, season).await?;
    let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

    if !players.is_empty() && !as_json {
        println!(
            "Computing ESPN projections for {} players...",
            players.len()
        );
    }

    let mut projected_points_data = Vec::new();

    // Calculate ESPN projections for each player
    for player in players {
        // Skip invalid player IDs, team placeholder entries, and individual defensive players
        if player.id < 0
            || player.default_position_id == 15
            || (player.default_position_id >= 8 && player.default_position_id <= 15)
        {
            continue;
        }

        // Apply local player name filtering for multiple names
        if let Some(names) = &player_names {
            if names.len() > 1 {
                let player_name = player.full_name.as_deref().unwrap_or("");
                let matches = names
                    .iter()
                    .any(|name| player_name.to_lowercase().contains(&name.to_lowercase()));
                if !matches {
                    continue;
                }
            }
        }

        let player_id = PlayerId::new(player.id as u64);

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
                projected_points_data.push((player_id, espn_projection));
            }
        }
    }

    // Get performance estimates using historical data
    if !as_json {
        println!("Analyzing historical performance bias and generating predictions...");
    }
    let estimates =
        db.estimate_week_performance(season, week, &projected_points_data, None, bias_strength)?;

    if estimates.is_empty() {
        if !as_json {
            println!("No projection data available for week {}.", week.as_u16()); // tarpaulin::skip
            println!("Make sure to fetch historical data for previous weeks first.");
            // tarpaulin::skip
        }
        return Ok(());
    }

    // Apply position filter and sort by estimated points (descending)
    let filtered_estimates: Vec<_> = estimates
        .into_iter()
        .filter(|estimate| {
            if let Some(pos_filters) = &positions {
                pos_filters.iter().any(|p| {
                    p.get_eligible_positions()
                        .iter()
                        .any(|eligible_pos| estimate.position == eligible_pos.to_string())
                })
            } else {
                true
            }
        })
        .collect();

    if !as_json {
        println!(
            "✓ Generated predictions for {} players",
            filtered_estimates.len()
        );
    }

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

        // Print column headers
        println!(
            "{:<20} {:<8} {:<8} {:<8} {:<8} {:<8}",
            "Name", "Pos", "ESPN", "Adj", "Final", "Conf%"
        );
        println!(
            "{:<20} {:<8} {:<8} {:<8} {:<8} {:<8}",
            "----", "---", "----", "---", "-----", "----"
        );

        for estimate in filtered_estimates {
            let adj_str = if estimate.bias_adjustment.abs() < 0.1 {
                "--".to_string()
            } else if estimate.bias_adjustment > 0.0 {
                format!("+{:.1}", estimate.bias_adjustment)
            } else {
                format!("{:.1}", estimate.bias_adjustment)
            };

            println!(
                "{:<20} {:<8} {:<8.1} {:<8} {:<8.1} {:<8}%",
                estimate.name.chars().take(20).collect::<String>(),
                estimate.position,
                estimate.espn_projection,
                adj_str,
                estimate.estimated_points,
                (estimate.confidence * 100.0) as u8
            );
        }
    }

    Ok(())
}
