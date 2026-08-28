//! Entry point: parse CLI and dispatch to command handlers.

use clap::Parser;
use espn_ffl::{
    cli::{Commands, ESPN},
    commands::{
        common::CommandParamsBuilder,
        draft_board::{handle_draft_board, handle_draft_board_watch, DraftBoardParams},
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
            clear_db,
            refresh,
        } => {
            let fantasy_team_filter = filters.get_fantasy_team_filter();
            let mut params = PlayerDataParams::new(filters.season, filters.week, projected)
                .with_optional_league_id(filters.league_id)
                .with_optional_player_names(filters.player_name)
                .with_optional_positions(filters.positions)
                .with_optional_injury_filter(filters.injury_status)
                .with_optional_roster_filter(filters.roster_status)
                .with_optional_fantasy_team_filter(fantasy_team_filter)
                .with_json_output_if(json)
                .with_refresh_if(refresh)
                .with_debug(debug);

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

            let params = ProjectionAnalysisParams::new(filters.season, filters.week, bias_factor)
                .with_optional_league_id(filters.league_id)
                .with_optional_player_names(filters.player_name)
                .with_optional_positions(filters.positions)
                .with_optional_injury_filter(filters.injury_status)
                .with_optional_roster_filter(filters.roster_status)
                .with_optional_fantasy_team_filter(fantasy_team_filter)
                .with_json_output_if(json)
                .with_refresh_if(refresh);

            handle_projection_analysis(params).await?
        }

        Commands::DraftBoard {
            league_id,
            season,
            positions,
            top,
            pool_size,
            rank_type,
            live,
            refresh,
            watch,
            team,
            team_id,
            json,
            debug,
        } => {
            let mut params = DraftBoardParams::new(season);
            params.league_id = league_id;
            params.positions = positions;
            params.top = Some(top);
            params.pool_size = pool_size;
            params.rank_type = rank_type;
            params.live = live;
            params.refresh = refresh;
            params.team = team;
            params.team_id = team_id;
            params.as_json = json;
            params.debug = debug;

            match watch {
                Some(interval) => handle_draft_board_watch(params, interval).await?,
                None => handle_draft_board(params).await?,
            }
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
