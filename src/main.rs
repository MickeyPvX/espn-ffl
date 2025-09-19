//! Entry point: parse CLI and dispatch to command handlers.

use clap::Parser;
use espn_ffl::{
    cli::{ESPN, Commands, GetCmd},
    commands::{handle_league_data, handle_player_data},
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
                limit,
                player_name,
                positions,
                projected,
                season,
                week,
            } => {
                handle_player_data(
                    debug, json, league_id, limit, player_name, positions, projected, season, week,
                )
                .await?
            }
        },
    }

    Ok(())
}
