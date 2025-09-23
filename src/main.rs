//! Entry point: parse CLI and dispatch to command handlers.

use clap::Parser;
use espn_ffl::{
    cli::{Commands, GetCmd, ESPN},
    commands::{
        league_data::handle_league_data,
        player_data::{handle_player_data, PlayerDataParams},
        projection_analysis::handle_projection_analysis,
    },
    Result,
};

/// Run the CLI.
#[tokio::main]
async fn main() -> Result<()> {
    let app = ESPN::parse();

    match app.command {
        Commands::Get { cmd } => match cmd {
            GetCmd::LeagueData {
                league_id,
                refresh,
                season,
                verbose,
            } => handle_league_data(league_id, refresh, season, verbose).await?,

            GetCmd::PlayerData {
                filters,
                debug,
                json,
                projected,
                refresh_positions,
                clear_db,
                refresh,
            } => {
                handle_player_data(PlayerDataParams {
                    debug,
                    as_json: json,
                    league_id: filters.league_id,
                    player_name: filters.player_name,
                    positions: filters.positions,
                    projected,
                    season: filters.season,
                    week: filters.week,
                    refresh_positions,
                    clear_db,
                    refresh,
                })
                .await?
            }

            GetCmd::ProjectionAnalysis {
                filters,
                json,
                refresh,
                bias_strength,
            } => {
                // Default to 1.0 (original conservative approach) if not specified
                let bias_factor = bias_strength.unwrap_or(1.0);
                handle_projection_analysis(
                    filters.season,
                    filters.week,
                    filters.league_id,
                    filters.player_name,
                    filters.positions,
                    json,
                    refresh,
                    bias_factor,
                )
                .await?
            }
        },
    }

    Ok(())
}
