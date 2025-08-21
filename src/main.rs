//! Entry point: parse CLI, build filters/headers, fetch, and print.

mod cli;
mod positions;
mod filters;
mod api;
mod util;
mod models { pub mod output; }

use crate::cli::ESPN;
use crate::filters::Filter;
use crate::util::{Result, maybe_cookie_header_map, parse_weeks_spec};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use structopt::StructOpt;
use std::collections::HashSet;

/// Run the CLI.
///
/// - Builds the `x-fantasy-filter` header using optional inputs (`--player-name`, `-p` positions).
/// - Omits `scoringPeriodId` and filters weeks from `stats[]`.
/// - Prints human lines or JSON (`--json`).
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
            weeks,
            debug,
            json,
        } => {
            let league_id = league_id
                .or_else(|| std::env::var("ESPN_FFL_LEAGUE_ID").ok()?.parse().ok())
                .ok_or("Missing --league-id and ESPN_FFL_LEAGUE_ID")?;

            // derive requested weeks (single or spec), then to HashSet
            let weeks_vec: Vec<u16> = match (week, weeks) {
                (Some(w), None) => vec![w],
                (None, Some(spec)) => parse_weeks_spec(&spec)?,
                (Some(_), Some(_)) => return Err("--week and --weeks are mutually exclusive".into()),
                (None, None) => return Err("please provide --week or --weeks".into()),
            };
            let weeks_set: HashSet<u16> = weeks_vec.iter().cloned().collect();

            // query params (omit scoringPeriodId for multi-week support)
            let params = vec![
                ("forLeagueId".to_string(), league_id.to_string()),
                ("view".to_string(), "kona_player_info".to_string()),
            ];

            // build filter header
            let filter = Filter::default()
                .active(true)
                .name_opt(player_name)
                .slots_opt(positions.map(|v| v.into_iter().map(u8::from).collect()));

            let mut headers = HeaderMap::new();
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
            headers.insert("x-fantasy-filter", filter.into_header_value()?);

            // add cookies if present (private leagues)
            if let Some(cookie_headers) = maybe_cookie_header_map()? {
                for (k, v) in cookie_headers.iter() {
                    if !headers.contains_key(k) {
                        headers.insert(k, v.clone());
                    }
                }
            }

            // fetch + build typed results
            let value = api::fetch_players(season, &params, &headers, debug).await?;
            let players = api::build_player_weeks_points(&value, season, &weeks_set);

            if json {
                println!("{}", serde_json::to_string_pretty(&players)?);
            } else {
                for p in players {
                    let weeks_str = p.weeks
                        .iter()
                        .map(|w| format!("{{ week: {}, points: {} }}", w.week, w.points))
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("{} {} [{}]", p.id, p.name, weeks_str);
                }
            }
        }
    }

    Ok(())
}
