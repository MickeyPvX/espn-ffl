//! Projection analysis command implementation

use crate::{
    cli::types::position::Position,
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::{get_player_data, update_player_points_with_roster_data, PlayerDataRequest},
        types::PlayerPoints,
    },
    storage::PlayerDatabase,
    Result, Season, Week,
};

use super::{
    common::{
        fetch_roster_data_with_message, scoring_slot_id, CommandParams, CommandParamsBuilder,
    },
    league_data::resolve_league_id,
    player_filters::{
        filter_and_convert_players, matches_fantasy_team_filter, matches_injury_filter,
        matches_roster_filter,
    },
};
use rayon::prelude::*;

/// Configuration for projection analysis.
#[derive(Debug)]
pub struct ProjectionAnalysisParams {
    pub base: CommandParams,
    pub bias_strength: f64,
}

impl ProjectionAnalysisParams {
    /// Create new parameters with required fields.
    pub fn new(season: Season, week: Week, bias_strength: f64) -> Self {
        Self {
            base: CommandParams::new(season, week),
            bias_strength,
        }
    }
}

impl CommandParamsBuilder for ProjectionAnalysisParams {
    fn base_mut(&mut self) -> &mut CommandParams {
        &mut self.base
    }

    fn base(&self) -> &CommandParams {
        &self.base
    }
}

/// Handle the projection analysis command.
pub async fn handle_projection_analysis(params: ProjectionAnalysisParams) -> Result<()> {
    let league_id = resolve_league_id(params.base.league_id)?;
    if !params.base.as_json {
        println!("Connecting to database...");
    }
    let db = PlayerDatabase::new()?;

    // Fetch week-specific roster data to match the week being analyzed
    let roster_data = fetch_roster_data_with_message(
        league_id,
        params.base.season,
        Some(params.base.week),
        params.base.refresh,
        !params.base.as_json,
    )
    .await?;

    // Settings are loaded first so the player request can narrow itself to the slots this
    // league actually rosters.
    if !params.base.as_json {
        println!("Loading league scoring settings...");
    }
    let settings = load_or_fetch_league_settings(league_id, false, params.base.season).await?;
    let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

    // Fetch ESPN projections for the target week (get_player_data handles caching internally)
    let players_val = get_player_data(PlayerDataRequest {
        debug: false,
        refresh: params.base.refresh,
        league_id,
        player_names: params.base.player_names.clone(),
        positions: params.base.positions.clone(),
        season: params.base.season,
        week: params.base.week,
        injury_status_filter: params.base.injury_status.clone(),
        roster_status_filter: params.base.roster_status.clone(),
        fallback_slot_ids: Some(settings.rosterable_slot_ids()),
    })
    .await?;

    let players: Vec<crate::espn::types::Player> = serde_json::from_value(players_val)?;

    // Note: No need to update players table since projection analysis works directly
    // with ESPN API data and doesn't rely on the database players table

    if !players.is_empty() && !params.base.as_json {
        println!(
            "Computing ESPN projections for {} players...",
            players.len()
        );
    }

    // Calculate ESPN projections for each player in parallel
    let projected_points_data: Vec<(crate::PlayerId, f64)> = filter_and_convert_players(
        players,
        params.base.player_names.clone(),
        params.base.positions.clone(),
    )
    .into_par_iter()
    .filter_map(|filtered_player| {
        let player = filtered_player.original_player;
        let player_id = filtered_player.player_id;

        if let Ok(player_value) = serde_json::to_value(&player) {
            if let Some(weekly_stats) = select_weekly_stats(
                &player_value,
                params.base.season.as_u16(),
                params.base.week.as_u16(),
                1, // stat_source = 1 for projected
            ) {
                let slot_id = scoring_slot_id(player.default_position_id as i32);
                let espn_projection =
                    compute_points_for_week(weekly_stats, slot_id, &scoring_index);

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
    if !params.base.as_json {
        println!("Analyzing historical performance bias and generating predictions...");
    }
    let estimates = db.estimate_week_performance(
        params.base.season,
        params.base.week,
        &projected_points_data,
        None,
        params.bias_strength,
    )?;

    if estimates.is_empty() {
        if !params.base.as_json {
            println!(
                "No projection data available for week {}.",
                params.base.week.as_u16()
            ); // tarpaulin::skip
            println!("Make sure to fetch historical data for previous weeks first.");
            // tarpaulin::skip
        }
        return Ok(());
    }

    // Get current injury/roster status and team data for filtering if needed
    let mut current_status_map = std::collections::HashMap::new();

    if params.base.injury_status.is_some()
        || params.base.roster_status.is_some()
        || params.base.fantasy_team_filter.is_some()
    {
        if !params.base.as_json {
            println!("Getting current player status and team data for filtering...");
        }

        // Create PlayerPoints objects for all estimates to get current status in parallel
        let mut temp_player_points: Vec<PlayerPoints> = estimates
            .par_iter()
            .map(|estimate| PlayerPoints::from_estimate(estimate, params.base.week))
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

    // Positions this league can actually start; anything else is dropped from the analysis.
    let rosterable_positions = settings.rosterable_positions();

    // Apply filters in parallel (position, injury status, roster status, team)
    let filtered_estimates: Vec<_> = estimates
        .into_par_iter()
        .filter(|estimate| {
            // First, check if this player's position is one the league can start
            let player_position = estimate.position.parse::<Position>().ok();
            if let Some(position) = player_position {
                if !rosterable_positions.contains(&position) {
                    return false; // Exclude non-fantasy positions
                }
            }

            // Apply user-specified position filter
            if let Some(pos_filters) = &params.base.positions {
                let Some(position) = player_position else {
                    return false; // Unresolvable position cannot satisfy a position filter
                };

                let position_matches = pos_filters.iter().any(|filter| match filter {
                    // Slot-shaped filters match any position eligible for that slot.
                    Position::FLEX | Position::BE | Position::IR => filter
                        .lineup_slot_ids()
                        .iter()
                        .any(|slot| position.fills_slot(*slot)),
                    _ => *filter == position,
                });
                if !position_matches {
                    return false;
                }
            }

            // Apply injury/roster status filters using current status
            if let Some(player_status) = current_status_map.get(&estimate.name) {
                // Apply injury status filter if specified
                if let Some(injury_filter) = &params.base.injury_status {
                    if !matches_injury_filter(player_status, injury_filter) {
                        return false;
                    }
                }

                // Apply roster status filter if specified
                if let Some(roster_filter) = &params.base.roster_status {
                    if !matches_roster_filter(player_status, roster_filter) {
                        return false;
                    }
                }

                // Apply fantasy team filter if specified
                if let Some(team_filter) = &params.base.fantasy_team_filter {
                    if !matches_fantasy_team_filter(player_status, team_filter) {
                        return false;
                    }
                }
            } else if params.base.injury_status.is_some()
                || params.base.roster_status.is_some()
                || params.base.fantasy_team_filter.is_some()
            {
                // Filters specified but no status info available - exclude this player
                return false;
            }

            true
        })
        .collect();

    if !params.base.as_json {
        println!(
            "✓ Generated predictions for {} players",
            filtered_estimates.len()
        );
    }

    if params.base.as_json {
        println!("{}", serde_json::to_string_pretty(&filtered_estimates)?); // tarpaulin::skip
    } else {
        // tarpaulin::skip - console output
        println!(
            "Projection Analysis & Predictions for Week {}",
            params.base.week.as_u16()
        );
        println!("Season: {}", params.base.season.as_u16());
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
