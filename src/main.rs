//! Entry point: parse CLI and dispatch to command handlers.

use clap::Parser;
use espn_ffl::{
    cli::{Commands, GetCmd, ESPN},
    commands::{
        handle_league_data, handle_player_data, handle_projection_analysis, PlayerDataParams,
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
                debug,
                json,
                league_id,
                player_name,
                positions,
                projected,
                season,
                week,
                refresh_positions,
                clear_db,
            } => {
                handle_player_data(PlayerDataParams {
                    debug,
                    as_json: json,
                    league_id,
                    player_name,
                    positions,
                    projected,
                    season,
                    week,
                    refresh_positions,
                    clear_db,
                })
                .await?
            }

            GetCmd::ProjectionAnalysis {
                season,
                week,
                league_id,
                player_name,
                positions,
                json,
            } => {
                handle_projection_analysis(season, week, league_id, player_name, positions, json)
                    .await?
            }
        },
    }

    Ok(())
}
