use reqwest::{Client, header::HeaderMap};
use serde_json::Value;

use crate::FlexResult;

/// Base path for ESPN Fantasy Football v3 API.
pub const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";

pub async fn get_league_settings(
    client: &Client,
    headers: HeaderMap,
    league_id: u32,
    season: u16,
) -> FlexResult<Value> {
    let url = format!(
        "{FFL_BASE_URL}/seasons/{}/segments/0/leagues/{}",
        season, league_id
    );
    let params = [("view", "mSettings")];

    let res = client
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
