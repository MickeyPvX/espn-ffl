use reqwest::{header::HeaderValue, Client};
use serde_json::Value;
use std::sync::LazyLock;

use crate::{
    cli::types::{
        filters::{InjuryStatusFilter, RosterStatusFilter},
        position::Position,
    },
    core::{
        build_players_filter,
        cache::{HttpPlayerDataCacheKey, LeagueSettingsCacheKey, RosterDataCacheKey, GLOBAL_CACHE},
        IntoHeaderValue,
    },
    LeagueId, Result, Season, Week,
};
use reqwest::header::{HeaderMap, ACCEPT, COOKIE};

#[cfg(test)]
mod tests;

/// Base path for ESPN Fantasy Football v3 API.
pub const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";

/// Parameters for player data retrieval.
#[derive(Debug)]
pub struct PlayerDataRequest {
    pub league_id: LeagueId,
    pub season: Season,
    pub week: Week,
    pub debug: bool,
    pub player_names: Option<Vec<String>>,
    pub positions: Option<Vec<Position>>,
    pub injury_status_filter: Option<InjuryStatusFilter>,
    pub roster_status_filter: Option<RosterStatusFilter>,
}

impl PlayerDataRequest {
    /// Create new request with required fields.
    pub fn new(league_id: LeagueId, season: Season, week: Week) -> Self {
        Self {
            league_id,
            season,
            week,
            debug: false,
            player_names: None,
            positions: None,
            injury_status_filter: None,
            roster_status_filter: None,
        }
    }

    /// Enable debug output.
    pub fn with_debug(mut self) -> Self {
        self.debug = true;
        self
    }

    /// Filter by specific player names.
    pub fn with_player_names(mut self, names: Vec<String>) -> Self {
        self.player_names = Some(names);
        self
    }

    /// Filter by positions.
    pub fn with_positions(mut self, positions: Vec<Position>) -> Self {
        self.positions = Some(positions);
        self
    }

    /// Filter by injury status.
    pub fn with_injury_filter(mut self, filter: InjuryStatusFilter) -> Self {
        self.injury_status_filter = Some(filter);
        self
    }

    /// Filter by roster status.
    pub fn with_roster_filter(mut self, filter: RosterStatusFilter) -> Self {
        self.roster_status_filter = Some(filter);
        self
    }
}

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent("espn-ffl-cli")
        .build()
        .expect("Failed to build http client")
});

/// Build HTTP headers for ESPN API requests.
///
/// Always includes Accept: application/json header.
/// Includes cookies if ESPN_SWID and ESPN_S2 environment variables are set.
fn build_espn_headers() -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    let swid = std::env::var("ESPN_SWID").ok();
    let s2 = std::env::var("ESPN_S2").ok();
    if let (Some(swid), Some(s2)) = (swid, s2) {
        let cookie = format!("SWID={}; espn_s2={}", swid, s2);
        headers.insert(COOKIE, HeaderValue::from_str(&cookie)?);
    }

    Ok(headers)
}

pub async fn get_league_settings(league_id: LeagueId, season: Season) -> Result<Value> {
    // Create cache key
    let cache_key = LeagueSettingsCacheKey { league_id, season };

    // Check cache first
    if let Some(cached_result) = GLOBAL_CACHE.league_settings.get(&cache_key) {
        return Ok(cached_result);
    }

    let url = format!(
        "{FFL_BASE_URL}/seasons/{}/segments/0/leagues/{}",
        season.as_u16(),
        league_id.as_u32()
    );
    let params = [("view", "mSettings")];
    let headers = build_espn_headers()?;

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

    // Cache the result
    GLOBAL_CACHE.league_settings.put(cache_key, res.clone());

    Ok(res)
}

pub async fn get_player_data(request: PlayerDataRequest) -> Result<Value> {
    // Create cache key - note: we need to determine if this is projected or not
    // For now, we'll assume this is actual data (projected is handled separately)
    let cache_key = HttpPlayerDataCacheKey {
        league_id: request.league_id,
        season: request.season,
        week: request.week,
        player_names: request.player_names.clone(),
        positions: request.positions.clone(),
        projected: false, // This function gets actual data
    };

    // Check cache first (but skip if debug mode to see the actual request)
    if !request.debug {
        if let Some(cached_result) = GLOBAL_CACHE.http_player_data.get(&cache_key) {
            return Ok(cached_result);
        }
    }

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

    let mut headers = build_espn_headers()?;
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

    // Cache the result (but not in debug mode)
    if !request.debug {
        GLOBAL_CACHE
            .http_player_data
            .put(cache_key, players_val.clone());
    }

    Ok(players_val)
}

/// Get league roster information with cache status (teams and their players)
pub async fn get_league_rosters_with_cache_status(
    debug: bool,
    league_id: LeagueId,
    season: Season,
    week: Option<Week>,
    refresh: bool,
) -> Result<(Value, CacheStatus)> {
    // Create cache key
    let cache_key = RosterDataCacheKey {
        league_id,
        season,
        week,
    };

    // Check cache first (but skip if debug mode or refresh flag is set)
    if !debug && !refresh {
        if let Some(cached_result) = GLOBAL_CACHE.roster_data.get(&cache_key) {
            return Ok((cached_result, CacheStatus::Hit));
        }
    }

    let cache_status = if refresh {
        CacheStatus::Refreshed
    } else {
        CacheStatus::Miss
    };
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

    let headers = build_espn_headers()?;

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

    // Cache the result (but not in debug mode)
    if !debug {
        GLOBAL_CACHE.roster_data.put(cache_key, res.clone());
    }

    Ok((res, cache_status))
}

/// Get league roster information (teams and their players) - backward compatibility
pub async fn get_league_rosters(
    debug: bool,
    league_id: LeagueId,
    season: Season,
    week: Option<Week>,
    refresh: bool,
) -> Result<Value> {
    let (data, _status) = get_league_rosters_with_cache_status(debug, league_id, season, week, refresh).await?;
    Ok(data)
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

    let headers = build_espn_headers()?;

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

    let headers = build_espn_headers()?;

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

/// Cache status for roster data
#[derive(Debug, Clone)]
pub enum CacheStatus {
    Hit,
    Miss,
    Refreshed,
}

/// Get league roster data and return team information with rosters
pub async fn get_league_roster_data(
    debug: bool,
    league_id: LeagueId,
    season: Season,
    week: Option<Week>,
    refresh: bool,
) -> Result<(crate::espn::types::LeagueData, CacheStatus)> {
    let (roster_data, cache_status) = get_league_rosters_with_cache_status(debug, league_id, season, week, refresh).await?;
    let league_data: crate::espn::types::LeagueData = serde_json::from_value(roster_data)?;
    Ok((league_data, cache_status))
}

/// Fetch roster data and update PlayerPoints with roster information
/// Fetch current league roster data once for efficient reuse
///
/// This function fetches the most recent roster information, which is what we need
/// for determining current team affiliations. Unlike historical data, current roster
/// information doesn't change based on the week being queried.
pub async fn fetch_current_roster_data(
    league_id: LeagueId,
    season: Season,
    verbose: bool,
    refresh: bool,
) -> Result<Option<(crate::espn::types::LeagueData, CacheStatus)>> {
    if verbose {
        println!("Checking league roster status...");
    }

    match get_league_roster_data(false, league_id, season, None, refresh).await {
        Ok((league_data, cache_status)) => {
            if verbose {
                println!("✓ Roster status fetched");
            }
            Ok(Some((league_data, cache_status)))
        }
        Err(e) => {
            if verbose {
                println!("⚠ Could not fetch roster data: {}", e);
            }
            Ok(None)
        }
    }
}

/// Update player points with pre-fetched roster information
///
/// This is more efficient than the original function as it doesn't make a separate
/// API call for roster data.
pub fn update_player_points_with_roster_data(
    player_points: &mut [crate::espn::types::PlayerPoints],
    roster_data: Option<&crate::espn::types::LeagueData>,
    verbose: bool,
) {
    if player_points.is_empty() {
        return;
    }

    if let Some(league_data) = roster_data {
        league_data.update_player_points_with_roster(player_points);
        if verbose {
            println!("✓ Roster status updated");
        }
    } else {
        if verbose {
            println!("⚠ No roster data available");
        }
        // Set all players as unknown roster status
        for player in player_points.iter_mut() {
            player.is_rostered = None;
        }
    }
}

/// Legacy function - kept for backward compatibility
///
/// This function is less efficient as it makes a separate API call.
/// Use `fetch_current_roster_data` + `update_player_points_with_roster_data` instead.
pub async fn update_player_points_with_roster_info(
    player_points: &mut [crate::espn::types::PlayerPoints],
    league_id: LeagueId,
    season: Season,
    verbose: bool,
    refresh: bool,
) -> Result<()> {
    let roster_data = fetch_current_roster_data(league_id, season, verbose, refresh).await?;
    if let Some((league_data, _cache_status)) = roster_data {
        update_player_points_with_roster_data(player_points, Some(&league_data), verbose);
    } else {
        update_player_points_with_roster_data(player_points, None, verbose);
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

    let mut headers = build_espn_headers()?;
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
