//! Entry point: parse CLI and dispatch to command handlers.

use clap::Parser;
use espn_ffl::{
    cli::{Commands, ESPN},
    commands::{
        common::CommandParamsBuilder,
        league_data::handle_league_data,
        player_data::{handle_player_data, PlayerDataParams},
        projection_analysis::{handle_projection_analysis, ProjectionAnalysisParams},
        update_all_data::handle_update_all_data,
    },
    Result,
};

/// Run the CLI.
#[tokio::main]
async fn main() -> Result<()> {
    let app = ESPN::parse();

    match app.command {
        Commands::LeagueData {
            league_id,
            refresh,
            season,
            verbose,
        } => handle_league_data(league_id, refresh, season, verbose).await?,

        Commands::PlayerData {
            filters,
            debug,
            json,
            projected,
            refresh_positions,
            clear_db,
            refresh,
        } => {
            let fantasy_team_filter = filters.get_fantasy_team_filter();
            let mut params = PlayerDataParams::new(filters.season, filters.week, projected);

            if let Some(league_id) = filters.league_id {
                params = params.with_league_id(league_id);
            }
            if let Some(player_names) = filters.player_name {
                params = params.with_player_names(player_names);
            }
            if let Some(positions) = filters.positions {
                params = params.with_positions(positions);
            }
            if let Some(injury_status) = filters.injury_status {
                params = params.with_injury_filter(injury_status);
            }
            if let Some(roster_status) = filters.roster_status {
                params = params.with_roster_filter(roster_status);
            }
            if let Some(team_filter) = fantasy_team_filter {
                params = params.with_fantasy_team_filter(team_filter);
            }
            if json {
                params = params.with_json_output();
            }
            if refresh {
                params = params.with_refresh();
            }
            if debug {
                params = params.with_debug();
            }

            params.refresh_positions = refresh_positions;
            params.clear_db = clear_db;

            handle_player_data(params).await?
        }

        Commands::ProjectionAnalysis {
            filters,
            json,
            refresh,
            bias_strength,
        } => {
            // Default to 1.0 (original conservative approach) if not specified
            let bias_factor = bias_strength.unwrap_or(1.0);
            let fantasy_team_filter = filters.get_fantasy_team_filter();

            let mut params =
                ProjectionAnalysisParams::new(filters.season, filters.week, bias_factor);

            if let Some(league_id) = filters.league_id {
                params = params.with_league_id(league_id);
            }
            if let Some(player_names) = filters.player_name {
                params = params.with_player_names(player_names);
            }
            if let Some(positions) = filters.positions {
                params = params.with_positions(positions);
            }
            if let Some(injury_status) = filters.injury_status {
                params = params.with_injury_filter(injury_status);
            }
            if let Some(roster_status) = filters.roster_status {
                params = params.with_roster_filter(roster_status);
            }
            if let Some(team_filter) = fantasy_team_filter {
                params = params.with_fantasy_team_filter(team_filter);
            }
            if json {
                params = params.with_json_output();
            }
            if refresh {
                params = params.with_refresh();
            }

            handle_projection_analysis(params).await?
        }

        Commands::UpdateAllData {
            league_id,
            season,
            through_week,
            verbose,
        } => handle_update_all_data(season, through_week, league_id, verbose).await?,
    }

    Ok(())
}
