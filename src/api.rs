//! HTTP calls to ESPN and transformation into output structs.

use once_cell::sync::Lazy;
use reqwest::{Client, header::HeaderMap};
use serde_json::Value;
use std::collections::HashSet;
use std::convert::TryFrom;

use crate::cli_types::Position;
use crate::models::output::{PlayerWeekPoints, WeekPoints};
use crate::models::stat_source::StatSource;
use crate::util::Result;

/// Reused reqwest client (connection pooling, UA).
static HTTP: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("espn-cli/0.1")
        .build()
        .expect("Client build")
});

/// Base path for ESPN Fantasy Football v3 API.
const FFL_BASE_URL: &str = "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl";

/// Fetch raw player JSON for a season using provided query params and headers.
///
/// Notes:
/// - We intentionally **omit** `scoringPeriodId` when fetching multiple weeks,
///   since the `stats[]` array in `kona_player_info` contains all weeks.
/// - Use `--debug` to print URL and headers before sending.
pub async fn fetch_players(
    season: u16,
    params: &[(String, String)],
    headers: &HeaderMap,
    debug: bool,
) -> Result<Value> {
    let url = format!("{}/seasons/{}/players", FFL_BASE_URL, season);
    let builder = HTTP.get(&url).headers(headers.clone()).query(params);

    if debug {
        let req = builder.try_clone().unwrap().build()?;
        eprintln!("URL => {}", req.url());
        eprintln!("HEADERS:");
        for (k, v) in req.headers().iter() {
            eprintln!("  {}: {:?}", k, v);
        }
    }

    let v = builder
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    Ok(v)
}

/// Build `PlayerWeekPoints` for a set of requested weeks.
/// Actual (statSourceId=0) or Projected (statSourceId=1) points.
///
/// Filters the `stats[]` array for tuples where:
/// - `seasonId == season`
/// - `scoringPeriodId in weeks`
/// - `statSourceId == 0` (official, not projections)
pub fn build_player_weeks_points(
    value: &Value,
    season: u16,
    source: StatSource,
    weeks: &HashSet<u16>,
) -> Vec<PlayerWeekPoints> {
    let mut out = Vec::new();
    let Some(arr) = value.as_array() else {
        return out;
    };

    for p in arr {
        let id = p.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
        let name = p
            .get("fullName")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let position = p
            .get("defaultPositionId")
            .and_then(|v| v.as_u64())
            .and_then(|id| Position::try_from(id as u8).ok())
            .map(|pos| pos.to_string())
            .unwrap_or(String::from("Unknown"));

        let mut wk = Vec::new();
        if let Some(stats) = p.get("stats").and_then(|v| v.as_array()) {
            for s in stats {
                let sid = s.get("seasonId").and_then(|v| v.as_u64());
                let week = s
                    .get("scoringPeriodId")
                    .and_then(|v| v.as_u64())
                    .map(|x| x as u16);
                let src = s.get("statSourceId").and_then(|v| v.as_u64());
                if sid == Some(season as u64) && src == Some(source.id()) {
                    if let Some(w) = week {
                        if weeks.contains(&w) {
                            let points = s
                                .get("appliedTotal")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            wk.push(WeekPoints { week: w, points });
                        }
                    }
                }
            }
        }

        wk.sort_by_key(|x| x.week);
        out.push(PlayerWeekPoints {
            id,
            name,
            position,
            weeks: wk,
        });
    }

    out
}
