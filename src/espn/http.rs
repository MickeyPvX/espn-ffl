use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT},
    Client,
};
use serde_json::Value;
use std::sync::LazyLock;

use crate::{
    cli::types::{InjuryStatusFilter, LeagueId, Position, RosterStatusFilter, Season, Week},
    core::{build_players_filter, maybe_cookie_header_map, IntoHeaderValue},
    Result,
};

#[cfg(test)]
mod tests;

/// Base path for ESPN Fantasy Football v3 API.
pub const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";

/// Parameters for player data retrieval to avoid too many function arguments.
#[derive(Debug)]
pub struct PlayerDataRequest {
    pub debug: bool,
    pub league_id: LeagueId,
    pub player_names: Option<Vec<String>>,
    pub positions: Option<Vec<Position>>,
    pub season: Season,
    pub week: Week,
    pub injury_status_filter: Option<InjuryStatusFilter>,
    pub roster_status_filter: Option<RosterStatusFilter>,
}

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent("espn-ffl-cli")
        .build()
        .expect("Failed to build http client")
});

fn get_common_headers() -> Result<HeaderMap> {
    // Try to get headers with cookies if present, otherwise build basic headers
    if let Some(headers) = maybe_cookie_header_map()? {
        Ok(headers)
    } else {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        Ok(headers)
    }
}

pub async fn get_league_settings(league_id: LeagueId, season: Season) -> Result<Value> {
    let url = format!(
        "{FFL_BASE_URL}/seasons/{}/segments/0/leagues/{}",
        season.as_u16(),
        league_id.as_u32()
    );
    let params = [("view", "mSettings")];
    let headers = get_common_headers()?;

    // tarpaulin::skip - HTTP client call
    let res = CLIENT
        .get(&url)
        .headers(headers)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;

    Ok(res)
}

pub async fn get_player_data(request: PlayerDataRequest) -> Result<Value> {
    // Build the filters from cli args
    let slots: Option<Vec<u8>> = request.positions.map(|ps| {
        ps.into_iter()
            .flat_map(|p| p.get_all_position_ids())
            .collect()
    });
    let players_filter = build_players_filter(
        request.player_names,
        slots,
        None,
        request.injury_status_filter.as_ref(),
        request.roster_status_filter.as_ref(),
    );

    let mut headers = get_common_headers()?;
    headers.insert("x-fantasy-filter", players_filter.to_header_value()?);

    // URL and query params
    let url = format!("{FFL_BASE_URL}/seasons/{}/players", request.season.as_u16());
    let params = [
        ("forLeagueId", request.league_id.to_string()),
        ("view", "kona_player_info".to_string()),
        ("view", "players_wl".to_string()),
        ("scoringPeriodId", request.week.as_u16().to_string()),
    ];

    if request.debug {
        // tarpaulin::skip - debug output
        eprintln!(
            "URL => seasons/{}/players?forLeagueId={}&view=kona_player_info&scoringPeriodId={}",
            request.season.as_u16(),
            request.league_id,
            request.week.as_u16()
        );
        for (k, v) in &headers {
            if let Ok(s) = v.to_str() {
                eprintln!("{}: {}", k, s); // tarpaulin::skip
            }
        }
    }

    // tarpaulin::skip - HTTP client call
    let players_val = CLIENT
        .get(&url)
        .headers(headers)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;

    Ok(players_val)
}

/// Get league roster information (teams and their players)
pub async fn get_league_rosters(
    debug: bool,
    league_id: LeagueId,
    season: Season,
    week: Option<Week>,
) -> Result<Value> {
    let url = format!(
        "{FFL_BASE_URL}/seasons/{}/segments/0/leagues/{}",
        season.as_u16(),
        league_id.as_u32()
    );

    let mut params = vec![
        ("view".to_string(), "mRoster".to_string()),
        ("view".to_string(), "mTeam".to_string()),
    ];

    if let Some(w) = week {
        params.push(("scoringPeriodId".to_string(), w.as_u16().to_string()));
    }

    let headers = get_common_headers()?;

    if debug {
        eprintln!("URL => {}", url);
        eprintln!("Params => {:?}", params);
    }

    let res = CLIENT
        .get(&url)
        .headers(headers)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;

    Ok(res)
}

/// Get detailed player information including injury status
pub async fn get_player_info(
    debug: bool,
    league_id: LeagueId,
    season: Season,
    week: Week,
) -> Result<Value> {
    let url = format!("{FFL_BASE_URL}/seasons/{}/players", season.as_u16());
    let params = [
        ("forLeagueId", league_id.to_string()),
        ("view", "players_wl".to_string()), // "wl" often means "with lineup" or detailed info
        ("scoringPeriodId", week.as_u16().to_string()),
    ];

    let headers = get_common_headers()?;

    if debug {
        eprintln!("URL => {}", url);
        eprintln!("Params => {:?}", params);
    }

    let res = CLIENT
        .get(&url)
        .headers(headers)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;

    Ok(res)
}

/// Test different view parameters to find player status information
pub async fn get_player_data_with_view(
    debug: bool,
    league_id: LeagueId,
    season: Season,
    week: Week,
    view: &str,
) -> Result<Value> {
    let url = format!("{FFL_BASE_URL}/seasons/{}/players", season.as_u16());
    let params = [
        ("forLeagueId", league_id.to_string()),
        ("view", view.to_string()),
        ("scoringPeriodId", week.as_u16().to_string()),
    ];

    let headers = get_common_headers()?;

    if debug {
        eprintln!("URL => {}", url);
        eprintln!("Params => {:?}", params);
    }

    let res = CLIENT
        .get(&url)
        .headers(headers)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;

    Ok(res)
}

/// Get league roster data and return team information with rosters
pub async fn get_league_roster_data(
    debug: bool,
    league_id: LeagueId,
    season: Season,
    week: Option<Week>,
) -> Result<crate::espn::types::LeagueData> {
    let roster_data = get_league_rosters(debug, league_id, season, week).await?;
    let league_data: crate::espn::types::LeagueData = serde_json::from_value(roster_data)?;
    Ok(league_data)
}

/// Fetch roster data and update PlayerPoints with roster information
pub async fn update_player_points_with_roster_info(
    player_points: &mut [crate::espn::types::PlayerPoints],
    league_id: LeagueId,
    season: Season,
    week: Week,
    verbose: bool,
) -> Result<()> {
    if player_points.is_empty() {
        return Ok(());
    }

    if verbose {
        println!("Checking league roster status...");
    }

    match get_league_roster_data(false, league_id, season, Some(week)).await {
        Ok(league_data) => {
            league_data.update_player_points_with_roster(player_points);
            if verbose {
                println!("✓ Roster status updated");
            }
        }
        Err(e) => {
            if verbose {
                println!("⚠ Could not fetch roster data: {}", e);
            }
            // Set all players as unknown roster status
            for player in player_points.iter_mut() {
                player.is_rostered = None;
            }
        }
    }

    Ok(())
}

/// Test function to try custom filter parameters with ESPN API
pub async fn get_player_data_with_custom_filter(
    debug: bool,
    league_id: LeagueId,
    season: Season,
    week: Week,
    custom_filter_json: &str,
) -> Result<Value> {
    let url = format!("{FFL_BASE_URL}/seasons/{}/players", season.as_u16());
    let params = [
        ("forLeagueId", league_id.to_string()),
        ("view", "kona_player_info".to_string()),
        ("scoringPeriodId", week.as_u16().to_string()),
    ];

    let mut headers = get_common_headers()?;
    headers.insert(
        "x-fantasy-filter",
        HeaderValue::from_str(custom_filter_json)?,
    );

    if debug {
        eprintln!("URL => {}", url);
        eprintln!("Params => {:?}", params);
        eprintln!("Custom filter => {}", custom_filter_json);
    }

    let res = CLIENT
        .get(&url)
        .headers(headers)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;

    Ok(res)
}
