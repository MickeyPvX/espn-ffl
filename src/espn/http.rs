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
    pub refresh: bool,
    pub player_names: Option<Vec<String>>,
    pub positions: Option<Vec<Position>>,
    pub injury_status_filter: Option<InjuryStatusFilter>,
    pub roster_status_filter: Option<RosterStatusFilter>,
    /// Lineup slots to request when the caller gave no explicit position filter.
    ///
    /// Without this, ESPN returns every player it knows about — including individual
    /// defensive players, punters and coaches that no standard league can start and that
    /// [`crate::commands::player_filters::filter_and_convert_players`] discards on arrival.
    /// Populating it from the league's own roster settings roughly halves the response.
    pub fallback_slot_ids: Option<Vec<u8>>,
}

impl PlayerDataRequest {
    /// Create new request with required fields.
    pub fn new(league_id: LeagueId, season: Season, week: Week) -> Self {
        Self {
            league_id,
            season,
            week,
            debug: false,
            refresh: false,
            player_names: None,
            positions: None,
            injury_status_filter: None,
            roster_status_filter: None,
            fallback_slot_ids: None,
        }
    }
}

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent("espn-ffl-cli")
        .build()
        .expect("Failed to build http client")
});

/// How many times to attempt a request that ESPN throttles or that fails transiently.
const MAX_ATTEMPTS: u32 = 4;

/// Base delay for exponential backoff between retries.
const RETRY_BASE_DELAY: std::time::Duration = std::time::Duration::from_millis(750);

/// How long a cached draft pool stays usable before it is refetched.
const DRAFT_POOL_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

/// How long cached data for the season in progress stays usable.
///
/// Kept short because a live week moves: projections are revised through the week, rosters
/// churn on waivers, and actual points change play by play. Finished seasons are exempt
/// entirely — see [`season_cache_max_age`].
const LIVE_SEASON_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Disk-cache lifetime for data belonging to a given season.
///
/// A completed season is final, so its cached responses never expire. The season in progress
/// gets [`LIVE_SEASON_MAX_AGE`].
///
/// This deliberately applies to every week of the live season rather than only the current
/// one, which costs little in practice: once a week is settled its numbers land in the local
/// database, and [`crate::commands::player_data::handle_player_data`] serves those without
/// issuing an HTTP request at all. The TTL therefore mostly governs weeks still in motion.
fn season_cache_max_age(season: Season) -> Option<std::time::Duration> {
    if season < Season::current() {
        None
    } else {
        Some(LIVE_SEASON_MAX_AGE)
    }
}

/// Minimum gap between consecutive requests to ESPN.
///
/// ESPN publishes no rate limit, so the tool paces itself rather than discovering one the
/// hard way. Bulk operations issue dozens of multi-megabyte requests back to back, which is
/// exactly the shape of traffic that gets an account throttled.
const MIN_REQUEST_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// Timestamp of the last request, used to space requests out.
static LAST_REQUEST: LazyLock<tokio::sync::Mutex<Option<std::time::Instant>>> =
    LazyLock::new(|| tokio::sync::Mutex::new(None));

/// Wait long enough that this request is at least [`MIN_REQUEST_INTERVAL`] after the last.
async fn pace_request() {
    let mut last = LAST_REQUEST.lock().await;
    if let Some(prev) = *last {
        let elapsed = prev.elapsed();
        if elapsed < MIN_REQUEST_INTERVAL {
            tokio::time::sleep(MIN_REQUEST_INTERVAL - elapsed).await;
        }
    }
    *last = Some(std::time::Instant::now());
}

/// Send a request, backing off and retrying when ESPN throttles or hiccups.
///
/// Retries on 429 and 5xx, honouring a `Retry-After` header when ESPN sends one and falling
/// back to exponential backoff otherwise. Client errors other than 429 (a bad cookie, an
/// unknown league) fail immediately, since retrying them would not help.
async fn send_json(request: reqwest::RequestBuilder, debug: bool) -> Result<Value> {
    for attempt in 1..=MAX_ATTEMPTS {
        // Clone before sending so the builder survives for the next attempt. try_clone only
        // returns None for streaming bodies, which none of these GET requests use; if it
        // ever does, fall through to a single un-retried attempt.
        let Some(attempt_request) = request.try_clone() else {
            pace_request().await;
            return Ok(request.send().await?.error_for_status()?.json().await?);
        };

        pace_request().await;
        let response = attempt_request.send().await?;
        let status = response.status();

        let throttled = status.as_u16() == 429;
        let retryable = throttled || status.is_server_error();

        // Success, or a client error that retrying cannot fix (bad cookie, unknown league).
        if !retryable || attempt == MAX_ATTEMPTS {
            return Ok(response.error_for_status()?.json().await?);
        }

        let delay = retry_after(&response).unwrap_or(RETRY_BASE_DELAY * 2u32.pow(attempt - 1));
        if debug || throttled {
            eprintln!(
                "ESPN returned {} - backing off {:.1}s (attempt {}/{})",
                status,
                delay.as_secs_f64(),
                attempt,
                MAX_ATTEMPTS
            );
        }
        tokio::time::sleep(delay).await;
    }

    // The loop always returns on its final attempt.
    unreachable!("send_json exhausted attempts without returning")
}

/// Parse a `Retry-After` header, which ESPN sends as whole seconds.
fn retry_after(response: &reqwest::Response) -> Option<std::time::Duration> {
    let raw = response.headers().get(reqwest::header::RETRY_AFTER)?;
    let secs: u64 = raw.to_str().ok()?.trim().parse().ok()?;
    // Guard against an absurd value pinning the CLI for hours.
    Some(std::time::Duration::from_secs(secs.min(60)))
}

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
    let res = send_json(CLIENT.get(&url).headers(headers).query(&params), false).await?;

    // Cache the result
    GLOBAL_CACHE.league_settings.put(cache_key, res.clone());

    Ok(res)
}

/// Fetch a league endpoint with an arbitrary set of `view` parameters, bypassing the cache.
///
/// Used for views whose value is their freshness (live draft state), where a cache hit would
/// defeat the purpose.
pub async fn get_league_view(league_id: LeagueId, season: Season, views: &[&str]) -> Result<Value> {
    let url = format!(
        "{FFL_BASE_URL}/seasons/{}/segments/0/leagues/{}",
        season.as_u16(),
        league_id.as_u32()
    );
    let params: Vec<(&str, &str)> = views.iter().map(|v| ("view", *v)).collect();
    let headers = build_espn_headers()?;

    let res = send_json(CLIENT.get(&url).headers(headers).query(&params), false).await?;

    Ok(res)
}

/// Fetch preseason player data suitable for building a draft board.
///
/// Differs from [`get_player_data`] in two ways that matter before a season starts: it asks
/// for `scoringPeriodId=0` (season totals rather than a specific week) and it sends a `limit`
/// plus a draft-rank sort. The limit is not just an optimisation — an unbounded query makes
/// ESPN drop `ownership`, `draftRanksByRankType` and `stats` from every player.
/// Deliberately takes no position filter: replacement levels and value ranks are only
/// meaningful when computed across the whole pool, so narrowing to one position belongs at
/// display time rather than here.
pub async fn get_draft_pool(
    league_id: LeagueId,
    season: Season,
    limit: u32,
    rank_type: &str,
    refresh: bool,
    debug: bool,
) -> Result<Value> {
    use crate::core::cache::DraftPoolCacheKey;
    use crate::core::filters::{PlayersFilter, SortDraftRanks, Val};

    let cache_key = DraftPoolCacheKey {
        league_id,
        season,
        limit,
        rank_type: rank_type.to_string(),
    };

    // The pool is ~10 MB and its contents move slowly: projections are refreshed by ESPN
    // roughly daily and ADP drifts over hours, so a short-lived disk cache spares repeated
    // draft-board runs from re-downloading it while staying current enough to draft on.
    if !refresh {
        if let Some(cached) = GLOBAL_CACHE
            .draft_pool
            .get_fresher_than(&cache_key, Some(DRAFT_POOL_MAX_AGE))
        {
            if debug {
                eprintln!("draft pool: cache hit");
            }
            return Ok(cached);
        }
    }

    let mut filter = PlayersFilter {
        limit: Some(limit),
        sort_draft_ranks: Some(SortDraftRanks::best_first(rank_type)),
        ..Default::default()
    };
    filter.filter_active = Some(Val { value: true });

    let mut headers = build_espn_headers()?;
    headers.insert("x-fantasy-filter", filter.to_header_value()?);

    let url = format!("{FFL_BASE_URL}/seasons/{}/players", season.as_u16());
    let params = [
        ("forLeagueId", league_id.to_string()),
        ("view", "kona_player_info".to_string()),
        // Season totals live under scoring period 0.
        ("scoringPeriodId", "0".to_string()),
    ];

    if debug {
        eprintln!("URL => {}", url);
        eprintln!("Params => {:?}", params);
        if let Some(f) = headers
            .get("x-fantasy-filter")
            .and_then(|v| v.to_str().ok())
        {
            eprintln!("x-fantasy-filter => {}", f);
        }
    }

    let res = send_json(CLIENT.get(&url).headers(headers).query(&params), debug).await?;

    GLOBAL_CACHE.draft_pool.put(cache_key, res.clone());

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

    // Check cache first (but skip if debug mode or refresh flag is set). Data for the live
    // season ages out; a finished season's numbers are final and kept indefinitely.
    if !request.debug && !request.refresh {
        if let Some(cached_result) = GLOBAL_CACHE
            .http_player_data
            .get_fresher_than(&cache_key, season_cache_max_age(request.season))
        {
            return Ok(cached_result);
        }
    }

    // Build the filters from cli args
    // `filterSlotIds` speaks ESPN's lineup-slot space, not the defaultPositionId space.
    // With no explicit position filter, fall back to the slots the league can actually
    // roster rather than downloading every player ESPN tracks.
    let slots: Option<Vec<u8>> = request
        .positions
        .map(|ps| ps.into_iter().flat_map(|p| p.lineup_slot_ids()).collect())
        .or(request.fallback_slot_ids);
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
    let players_val = send_json(
        CLIENT.get(&url).headers(headers).query(&params),
        request.debug,
    )
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

    // Check cache first (but skip if debug mode or refresh flag is set). Rosters churn on
    // waivers and trades, so live-season entries age out the same way player data does.
    if !debug && !refresh {
        if let Some(cached_result) = GLOBAL_CACHE
            .roster_data
            .get_fresher_than(&cache_key, season_cache_max_age(season))
        {
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

    let res = send_json(CLIENT.get(&url).headers(headers).query(&params), debug).await?;

    if debug {
        eprintln!("RAW ROSTER API RESPONSE:");
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&res)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );
    }

    // Cache the result (but not in debug mode)
    if !debug {
        GLOBAL_CACHE.roster_data.put(cache_key, res.clone());
    }

    Ok((res, cache_status))
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
    let (roster_data, cache_status) =
        get_league_rosters_with_cache_status(debug, league_id, season, week, refresh).await?;
    let league_data: crate::espn::types::LeagueData = serde_json::from_value(roster_data)?;
    Ok((league_data, cache_status))
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
