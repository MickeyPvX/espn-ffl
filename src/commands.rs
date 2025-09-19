//! Command implementations for ESPN Fantasy Football CLI

use crate::{
    Result,
    cli_types::{LeagueId, Season, Week, Position},
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week, select_weekly_stats},
        http::get_player_data,
        types::PlayerPoints,
    },
    cache::league_settings_path,
    error::EspnError,
    LEAGUE_ID_ENV_VAR,
};

/// Handle the league data command
pub async fn handle_league_data(
    league_id: Option<LeagueId>,
    refresh: bool,
    season: Season,
    verbose: bool,
) -> Result<()> {
    let league_id = resolve_league_id(league_id)?;
    let settings = load_or_fetch_league_settings(league_id, refresh, season).await?;

    if verbose {
        let path = league_settings_path(season.as_u16(), league_id.as_u32());
        eprintln!("Cached at: {}", path.display());
        eprintln!(
            "Scoring items: {:?}",
            settings.scoring_settings.scoring_items
        );
    } else {
        println!("League settings successfully retrieved!")
    }

    Ok(())
}

/// Handle the player data command
pub async fn handle_player_data(
    debug: bool,
    as_json: bool,
    league_id: Option<LeagueId>,
    limit: Option<u32>,
    player_name: Option<String>,
    positions: Option<Vec<Position>>,
    projected: bool,
    season: Season,
    week: Week,
) -> Result<()> {
    let league_id = resolve_league_id(league_id)?;

    // Load or fetch league settings to compute points; cached for future runs.
    let settings = load_or_fetch_league_settings(league_id, false, season).await?;
    let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

    let players_val = get_player_data(
        debug,
        league_id,
        limit,
        player_name,
        positions,
        season,
        week,
    )
    .await?;

    let empty = Vec::new();
    let arr = players_val.as_array().unwrap_or(&empty);
    let stat_source = if projected { 1 } else { 0 };

    let mut player_points: Vec<PlayerPoints> = Vec::new();

    for p in arr {
        let id = p.get("id")
            .and_then(|v| v.as_u64())
            .map(crate::cli_types::PlayerId::new)
            .unwrap_or(crate::cli_types::PlayerId::new(0));

        let name = p
            .get("fullName")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let slot_id = p
            .get("defaultPositionId")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u8;

        if let Some(weekly_stats) = select_weekly_stats(p, season.as_u16(), week.as_u16(), stat_source) {
            let points = compute_points_for_week(weekly_stats, slot_id, &scoring_index);
            if points > 0f64 {
                player_points.push(PlayerPoints {
                    id,
                    name,
                    week,
                    projected,
                    points,
                });
            }
        }
    }

    // Sort descending by points
    player_points.sort_by(|a, b| b.points.partial_cmp(&a.points).unwrap_or(std::cmp::Ordering::Equal));

    if as_json {
        println!("{}", serde_json::to_string_pretty(&player_points)?);
    } else {
        for player in player_points {
            println!(
                "{} {} [week {}] {:.2}",
                player.id.as_u64(),
                player.name,
                player.week.as_u16(),
                player.points,
            );
        }
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