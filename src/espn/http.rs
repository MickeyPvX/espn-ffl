use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT},
    Client,
};
use serde_json::Value;
use std::sync::LazyLock;

use crate::{
    cli_types::{LeagueId, Position, Season, Week},
    filters::{build_players_filter, IntoHeaderValue},
    util::maybe_cookie_header_map,
    Result,
};

#[cfg(test)]
mod tests;

/// Base path for ESPN Fantasy Football v3 API.
pub const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent("espn-ffl-cli")
        .build()
        .expect("Failed to build http client")
});

fn get_common_headers() -> Result<HeaderMap> {
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

pub async fn get_player_data(
    debug: bool,
    league_id: LeagueId,
    limit: Option<u32>,
    player_name: Option<String>,
    positions: Option<Vec<Position>>,
    season: Season,
    week: Week,
) -> Result<Value> {
    // Build the filters from cli args
    let slots: Option<Vec<u8>> = positions.map(|ps| ps.into_iter().map(u8::from).collect());
    let players_filter = build_players_filter(limit, player_name, slots, None);

    let mut headers = get_common_headers()?;
    headers.insert("x-fantasy-filter", players_filter.to_header_value()?);

    // URL and query params
    let url = format!("{FFL_BASE_URL}/seasons/{}/players", season.as_u16());
    let params = [
        ("forLeagueId", league_id.to_string()),
        ("view", "kona_player_info".to_string()),
        ("scoringPeriodId", week.as_u16().to_string()),
    ];

    if debug {
        // tarpaulin::skip - debug output
        eprintln!(
            "URL => seasons/{}/players?forLeagueId={}&view=kona_player_info&scoringPeriodId={}",
            season.as_u16(),
            league_id,
            week.as_u16()
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
