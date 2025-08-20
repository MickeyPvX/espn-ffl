mod api;
mod cli;
mod filters;
mod positions;
mod util;

use crate::cli::ESPN;
use crate::filters::Filter;
use crate::util::{Result, maybe_cookie_header_map};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use serde_json::Value;
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<()> {
    let app = ESPN::from_args();

    match app {
        ESPN::Get {
            league_id,
            player_name,
            positions,
            season,
            week,
            debug,
        } => {
            // league id: from flag or env
            let league_id = league_id
                .or_else(|| std::env::var("ESPN_FFL_LEAGUE_ID").ok()?.parse().ok())
                .ok_or("Missing --league-id and ESPN_FFL_LEAGUE_ID")?;

            // query params
            let params = vec![
                ("forLeagueId".to_string(), league_id.to_string()),
                ("scoringPeriodId".to_string(), week.to_string()),
                ("view".to_string(), "kona_player_info".to_string()),
            ];

            // build filter JSON (root-level; optional fields)
            let filter = Filter::default()
                .active(true)
                .name_opt(player_name)
                .slots_opt(positions.map(|v| v.into_iter().map(u8::from).collect()));

            // headers: Accept + x-fantasy-filter + optional cookies
            let mut headers = HeaderMap::new();
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
            headers.insert("x-fantasy-filter", filter.into_header_value()?);

            if let Some(cookie_headers) = maybe_cookie_header_map()? {
                for (k, v) in cookie_headers.iter() {
                    // merge (won't overwrite Accept/x-fantasy-filter)
                    if !headers.contains_key(k) {
                        headers.insert(k, v.clone());
                    }
                }
            }

            let value = api::fetch_players(season, &params, &headers, debug).await?;
            let arr: &[Value] = value
                .as_array()
                .map(|v| v.as_slice())
                .unwrap_or(&[] as &[Value]);
            for p in arr {
                if let Some(name) = p.get("fullName").and_then(|v| v.as_str()) {
                    println!("{name}");
                }
            }
        }
    }

    Ok(())
}
