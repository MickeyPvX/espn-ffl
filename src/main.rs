//! Entry point: parse CLI, build filters/headers, fetch, and print.

mod cache;
mod cli;
mod cli_types;
mod espn;
mod filters;
mod util;

use serde_json::json;
use structopt::StructOpt;

use crate::cli::{ESPN, GetCmd};
use crate::espn::cache_settings::load_or_fetch_league_settings;
use crate::espn::compute::{build_scoring_index, compute_points_for_week, select_weekly_stats};
use crate::espn::http::get_player_data;

pub type FlexResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const LEAGUE_ID_ENV_VAR: &str = "ESPN_FFL_LEAGUE_ID";

/// Run the CLI.
#[tokio::main]
async fn main() -> FlexResult<()> {
    let app = ESPN::from_args();

    match app {
        ESPN::Get(GetCmd::LeagueData {
            league_id,
            refresh,
            season,
            verbose,
        }) => {
            let league_id = league_id
                .or_else(|| std::env::var(LEAGUE_ID_ENV_VAR).ok()?.parse().ok())
                .ok_or(format!(
                    "League ID not provided and {LEAGUE_ID_ENV_VAR} not set!"
                ))?;

            let settings = load_or_fetch_league_settings(league_id, refresh, season).await?;

            if verbose {
                let path = crate::cache::league_settings_path(season, league_id);
                eprintln!("Cached at: {}", path.display());
                eprintln!(
                    "Scoring items: {:?}",
                    settings.scoring_settings.scoring_items
                );
            } else {
                println!("League settings successfully retrieved!")
            }
        }

        ESPN::Get(GetCmd::PlayerData {
            debug,
            json: as_json,
            league_id,
            limit,
            player_name,
            positions,
            projected,
            season,
            week,
        }) => {
            let league_id = league_id
                .or_else(|| std::env::var(LEAGUE_ID_ENV_VAR).ok()?.parse().ok())
                .ok_or(format!(
                    "League ID not provided and {LEAGUE_ID_ENV_VAR} not set!"
                ))?;

            // Load or fetch league settings to compute points; cached for future runs.
            let settings = load_or_fetch_league_settings(league_id, false, season).await?;
            let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);

            // Avoid borrowing a temporary Vec
            let empty = Vec::new();
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
            let arr = players_val.as_array().unwrap_or(&empty);

            let stat_source = if projected { 1 } else { 0 };

            let mut out = Vec::new();
            for p in arr {
                let id = p.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let name = p
                    .get("fullName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();
                let slot_id = p
                    .get("defaultPositionId")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u8;

                if let Some(weekly_stats) = select_weekly_stats(p, season, week, stat_source) {
                    let points = compute_points_for_week(weekly_stats, slot_id, &scoring_index);
                    if points > 0f64 {
                        out.push(json!(
                            {
                                "id": id,
                                "name": name,
                                "week": week,
                                "projected": projected,
                                "points": points,
                            }
                        ));
                    }
                };
            }

            // Sort descending by points
            out.sort_by(|a, b| {
                let pa = a.get("points").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let pb = b.get("points").and_then(|v| v.as_f64()).unwrap_or(0.0);
                pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
            });

            if as_json {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                for item in out {
                    println!(
                        "{} {} [week {}] {:2}",
                        item.get("id").ok_or("UNKNOWN")?,
                        item.get("name").ok_or("UNKNOWN")?,
                        item.get("week").ok_or("UNKNOWN")?,
                        item.get("points").ok_or("UNKNOWN")?,
                    );
                }
            }
        }
    }

    Ok(())
}
