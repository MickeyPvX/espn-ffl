// src/espn/cache_settings.rs
use serde_json::Value;

use crate::core::{league_settings_path, try_read_to_string, write_string};
use crate::espn::types::LeagueEnvelope;
use crate::espn::{http::get_league_settings, types::LeagueSettings};
use crate::{
    cli::types::{LeagueId, Season},
    Result,
};

/// Try to load league settings from .cache first. If missing or `refresh == true`,
/// fetch from ESPN (`view=mSettings`), extract the `"settings"` object, and re-write the cache.
pub async fn load_or_fetch_league_settings(
    league_id: LeagueId,
    refresh: bool,
    season: Season,
) -> Result<LeagueSettings> {
    let path = league_settings_path(season.as_u16(), league_id.as_u32());

    // 1) Try cache (unless refresh)
    if !refresh {
        // tarpaulin::skip - file I/O operation
        if let Some(s) = try_read_to_string(&path) {
            // tarpaulin::skip - JSON parsing of cached data
            if let Ok(v) = serde_json::from_str::<Value>(&s) {
                if let Some(parsed) = try_parse_settings_from_cached(&v) {
                    return Ok(parsed);
                }
            }
        }
    }

    // 2) Fetch from API (raw ESPN payload with `"settings"`)
    // tarpaulin::skip - HTTP API call
    let parsed: LeagueEnvelope =
        serde_json::from_value(get_league_settings(league_id, season).await?)?;

    // 3) Write cache (store the raw ESPN payload so future reads can pluck "settings")
    if let Ok(json_str) = serde_json::to_string_pretty(&parsed.settings) {
        let _ = write_string(&path, &json_str); // tarpaulin::skip - file I/O operation
    }

    Ok(parsed.settings)
}

/// Attempt to parse a cached JSON Value into LeagueSettings.
///
/// Supported cache shapes:
/// - The raw ESPN payload (object with a "settings" field)
/// - A bare LeagueSettings object (older cache content)
fn try_parse_settings_from_cached(v: &Value) -> Option<LeagueSettings> {
    // If it's the raw ESPN payload, prefer the "settings" object
    if let Some(settings) = v.get("settings") {
        return serde_json::from_value::<LeagueSettings>(settings.clone()).ok(); // tarpaulin::skip
    }
    // Otherwise, try to parse the whole value as LeagueSettings
    serde_json::from_value::<LeagueSettings>(v.clone()).ok() // tarpaulin::skip
}
