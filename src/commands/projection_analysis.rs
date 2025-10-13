//! Projection analysis command implementation

use crate::{
    cli::types::{
        filters::{FantasyTeamFilter, InjuryStatusFilter, RosterStatusFilter},
        position::Position,
    },
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::{get_player_data, update_player_points_with_roster_data, PlayerDataRequest},
        types::PlayerPoints,
    },
    storage::PlayerDatabase,
    LeagueId, Result, Season, Week,
};

use super::{
    player_filters::{
        filter_and_convert_players, matches_fantasy_team_filter, matches_injury_filter,
        matches_roster_filter,
    },
    resolve_league_id,
};
use rayon::prelude::*;

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
    injury_status: Option<InjuryStatusFilter>,
    roster_status: Option<RosterStatusFilter>,
    fantasy_team_filter: Option<FantasyTeamFilter>,
) -> Result<()> {
    let league_id = resolve_league_id(league_id)?;
    if !as_json {
        println!("Connecting to database...");
    }
    let db = PlayerDatabase::new()?;

    // Fetch week-specific roster data to match the week being analyzed
    let roster_data = match crate::espn::http::get_league_roster_data(
        false,
        league_id,
        season,
        Some(week),
        refresh,
    )
    .await
    {
        Ok((data, cache_status)) => {
            if !as_json {
                match cache_status {
                    crate::espn::http::CacheStatus::Hit => {
                        println!("✓ Week {} roster status loaded (from cache)", week.as_u16());
                    }
                    crate::espn::http::CacheStatus::Miss => {
                        println!(
                            "✓ Week {} roster status fetched (cache miss)",
                            week.as_u16()
                        );
                    }
                    crate::espn::http::CacheStatus::Refreshed => {
                        println!("✓ Week {} roster status fetched (refreshed)", week.as_u16());
                    }
                }
            }
            Some(data)
        }
        Err(e) => {
            if !as_json {
                println!(
                    "⚠ Could not fetch week {} roster data: {}",
                    week.as_u16(),
                    e
                );
            }
            None
        }
    };

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
        get_player_data(PlayerDataRequest {
            debug: false,
            league_id,
            player_names: player_names.clone(),
            positions: positions.clone(),
            season,
            week,
            injury_status_filter: injury_status.clone(),
            roster_status_filter: roster_status.clone(),
        })
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

    // Calculate ESPN projections for each player in parallel
    let projected_points_data: Vec<(crate::PlayerId, f64)> =
        filter_and_convert_players(players, player_names.clone(), positions.clone())
            .into_par_iter()
            .filter_map(|filtered_player| {
                let player = filtered_player.original_player;
                let player_id = filtered_player.player_id;

                if let Ok(player_value) = serde_json::to_value(&player) {
                    if let Some(weekly_stats) = select_weekly_stats(
                        &player_value,
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

                        Some((player_id, espn_projection))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

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

    // Get current injury/roster status and team data for filtering if needed
    let mut current_status_map = std::collections::HashMap::new();

    if injury_status.is_some() || roster_status.is_some() || fantasy_team_filter.is_some() {
        if !as_json {
            println!("Getting current player status and team data for filtering...");
        }

        // Create PlayerPoints objects for all estimates to get current status in parallel
        let mut temp_player_points: Vec<PlayerPoints> = estimates
            .par_iter()
            .map(|estimate| PlayerPoints::from_estimate(estimate, week))
            .collect();

        // Get current injury/roster status and team data using pre-fetched data
        update_player_points_with_roster_data(
            &mut temp_player_points,
            roster_data.as_ref(),
            false, // not verbose
        );

        // Build status map for filtering
        for player in temp_player_points {
            current_status_map.insert(player.name.clone(), player);
        }
    }

    // Apply filters in parallel (position, injury status, roster status, team)
    let filtered_estimates: Vec<_> = estimates
        .into_par_iter()
        .filter(|estimate| {
            // Apply position filter
            if let Some(pos_filters) = &positions {
                let position_matches = pos_filters.iter().any(|p| {
                    match p {
                        // For flexible positions, check if player position is eligible
                        Position::FLEX => {
                            matches!(estimate.position.as_str(), "RB" | "WR" | "TE")
                        }
                        // For individual positions, compare directly
                        _ => estimate.position == p.to_string(),
                    }
                });
                if !position_matches {
                    return false;
                }
            }

            // Apply injury/roster status filters using current status
            if let Some(player_status) = current_status_map.get(&estimate.name) {
                // Apply injury status filter if specified
                if let Some(injury_filter) = &injury_status {
                    if !matches_injury_filter(player_status, injury_filter) {
                        return false;
                    }
                }

                // Apply roster status filter if specified
                if let Some(roster_filter) = &roster_status {
                    if !matches_roster_filter(player_status, roster_filter) {
                        return false;
                    }
                }

                // Apply fantasy team filter if specified
                if let Some(team_filter) = &fantasy_team_filter {
                    if !matches_fantasy_team_filter(player_status, team_filter) {
                        return false;
                    }
                }
            } else if injury_status.is_some()
                || roster_status.is_some()
                || fantasy_team_filter.is_some()
            {
                // Filters specified but no status info available - exclude this player
                return false;
            }

            true
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
            "{:<20} {:<8} {:<8} {:<8} {:<8} {:<8} Reasoning",
            "Name", "Pos", "ESPN", "Adj", "Final", "Conf%"
        );
        println!(
            "{:<20} {:<8} {:<8} {:<8} {:<8} {:<8} ---------",
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
                "{:<20} {:<8} {:<8.1} {:<8} {:<8.1} {:<8}% {}",
                estimate.name.chars().take(20).collect::<String>(),
                estimate.position,
                estimate.espn_projection,
                adj_str,
                estimate.estimated_points,
                (estimate.confidence * 100.0) as u8,
                estimate.reasoning
            );
        }
    }

    Ok(())
}
