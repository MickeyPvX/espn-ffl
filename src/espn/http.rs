use once_cell::sync::Lazy;
use reqwest::{
    Client,
    header::{ACCEPT, HeaderMap, HeaderValue},
};
use serde_json::Value;

use crate::{
    FlexResult,
    cli_types::Position,
    filters::{IntoHeaderValue, build_players_filter},
    util::maybe_cookie_header_map,
};

/// Base path for ESPN Fantasy Football v3 API.
pub const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";

static CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("espn-ffl-cli")
        .build()
        .expect("Failed to build http client")
});

fn get_common_headers() -> FlexResult<HeaderMap> {
    // Build common headers
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    // Try to add cookies if present
    if let Some(cookie_headers) = maybe_cookie_header_map()? {
        for (k, v) in cookie_headers {
            headers.insert(k.unwrap(), v); // `k` is Option<HeaderName>
        }
    }

    Ok(headers)
}

pub async fn get_league_settings(league_id: u32, season: u16) -> FlexResult<Value> {
    let url = format!(
        "{FFL_BASE_URL}/seasons/{}/segments/0/leagues/{}",
        season, league_id
    );
    let params = [("view", "mSettings")];
    let headers = get_common_headers()?;

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

pub async fn get_player_data(
    debug: bool,
    league_id: u32,
    limit: Option<u32>,
    player_name: Option<String>,
    positions: Option<Vec<Position>>,
    season: u16,
    week: u16,
) -> FlexResult<Value> {
    // Build the filters from cli args
    let slots: Option<Vec<u8>> = positions.map(|ps| ps.into_iter().map(u8::from).collect());
    let players_filter = build_players_filter(limit, player_name, slots, None);

    let mut headers = get_common_headers()?;
    headers.insert("x-fantasy-filter", players_filter.into_header_value()?);

    // URL and query params
    let url = format!("{FFL_BASE_URL}/seasons/{}/players", season);
    let params = [
        ("forLeagueId", league_id.to_string()),
        ("view", "kona_player_info".to_string()),
        ("scoringPeriodId", week.to_string()),
    ];

    if debug {
        eprintln!(
            "URL => seasons/{}/players?forLeagueId={}&view=kona_player_info&scoringPeriodId={}",
            season, league_id, week
        );
        for (k, v) in &headers {
            if let Ok(s) = v.to_str() {
                eprintln!("{}: {}", k, s);
            }
        }
    }

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
