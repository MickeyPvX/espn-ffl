// src/espn/cache_settings.rs
use serde::Deserialize;

use crate::core::{league_settings_path, try_read_to_string, write_string};
use crate::espn::types::LeagueEnvelope;
use crate::espn::{http::get_league_settings, types::LeagueSettings};
use crate::{
    cli::types::{LeagueId, Season},
    Result,
};

/// Wrapper for cached league settings that handles both formats:
/// - Newer format: ESPN response with "settings" field (LeagueEnvelope)
/// - Older format: Direct LeagueSettings object
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CachedLeagueData {
    /// Full ESPN response format with settings wrapper
    Envelope(LeagueEnvelope),
    /// Direct settings format (older cache)
    Direct(LeagueSettings),
}

impl CachedLeagueData {
    fn into_settings(self) -> LeagueSettings {
        match self {
            CachedLeagueData::Envelope(envelope) => envelope.settings,
            CachedLeagueData::Direct(settings) => settings,
        }
    }
}

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
            if let Ok(cached_data) = serde_json::from_str::<CachedLeagueData>(&s) {
                return Ok(cached_data.into_settings());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::espn::types::{ScoringItem, ScoringSettings};
    use serde_json::json;

    #[test]
    fn test_cached_league_data_envelope_format() {
        // Test the newer envelope format with "settings" wrapper
        let envelope_json = json!({
            "settings": {
                "scoringSettings": {
                    "scoringItems": [
                        {
                            "statId": 53,
                            "points": 0.04,
                            "pointsOverrides": {}
                        }
                    ]
                }
            }
        });

        let cached_data: CachedLeagueData = serde_json::from_value(envelope_json).unwrap();
        let settings = cached_data.into_settings();

        assert_eq!(settings.scoring_settings.scoring_items.len(), 1);
        assert_eq!(settings.scoring_settings.scoring_items[0].stat_id, 53);
        assert_eq!(settings.scoring_settings.scoring_items[0].points, 0.04);
    }

    #[test]
    fn test_cached_league_data_direct_format() {
        // Test the older direct format (just the settings object)
        let direct_json = json!({
            "scoringSettings": {
                "scoringItems": [
                    {
                        "statId": 1,
                        "points": 6.0,
                        "pointsOverrides": {
                            "0": 4.0
                        }
                    },
                    {
                        "statId": 20,
                        "points": -2.0,
                        "pointsOverrides": {}
                    }
                ]
            }
        });

        let cached_data: CachedLeagueData = serde_json::from_value(direct_json).unwrap();
        let settings = cached_data.into_settings();

        assert_eq!(settings.scoring_settings.scoring_items.len(), 2);
        assert_eq!(settings.scoring_settings.scoring_items[0].stat_id, 1);
        assert_eq!(settings.scoring_settings.scoring_items[0].points, 6.0);
        assert_eq!(settings.scoring_settings.scoring_items[1].stat_id, 20);
        assert_eq!(settings.scoring_settings.scoring_items[1].points, -2.0);
    }

    #[test]
    fn test_cached_league_data_into_settings_envelope() {
        // Test conversion from envelope format
        let scoring_item = ScoringItem {
            stat_id: 24,
            points: 6.0,
            points_overrides: std::collections::BTreeMap::new(),
        };
        let scoring_settings = ScoringSettings {
            scoring_items: vec![scoring_item],
        };
        let league_settings = LeagueSettings { scoring_settings };
        let envelope = LeagueEnvelope {
            settings: league_settings,
        };

        let cached_data = CachedLeagueData::Envelope(envelope);
        let settings = cached_data.into_settings();

        assert_eq!(settings.scoring_settings.scoring_items.len(), 1);
        assert_eq!(settings.scoring_settings.scoring_items[0].stat_id, 24);
        assert_eq!(settings.scoring_settings.scoring_items[0].points, 6.0);
    }

    #[test]
    fn test_cached_league_data_into_settings_direct() {
        // Test conversion from direct format
        let scoring_item = ScoringItem {
            stat_id: 53,
            points: 0.04,
            points_overrides: std::collections::BTreeMap::new(),
        };
        let scoring_settings = ScoringSettings {
            scoring_items: vec![scoring_item],
        };
        let league_settings = LeagueSettings { scoring_settings };

        let cached_data = CachedLeagueData::Direct(league_settings);
        let settings = cached_data.into_settings();

        assert_eq!(settings.scoring_settings.scoring_items.len(), 1);
        assert_eq!(settings.scoring_settings.scoring_items[0].stat_id, 53);
        assert_eq!(settings.scoring_settings.scoring_items[0].points, 0.04);
    }

    #[test]
    fn test_cached_league_data_deserialization_complex() {
        // Test complex scoring settings with multiple overrides
        let complex_json = json!({
            "scoringSettings": {
                "scoringItems": [
                    {
                        "statId": 53,
                        "points": 0.04,
                        "pointsOverrides": {
                            "0": 0.025,
                            "2": 0.05
                        }
                    },
                    {
                        "statId": 1,
                        "points": 6.0,
                        "pointsOverrides": {
                            "0": 4.0,
                            "1": 6.0,
                            "2": 8.0
                        }
                    }
                ]
            }
        });

        let cached_data: CachedLeagueData = serde_json::from_value(complex_json).unwrap();
        let settings = cached_data.into_settings();

        assert_eq!(settings.scoring_settings.scoring_items.len(), 2);

        let passing_yards = &settings.scoring_settings.scoring_items[0];
        assert_eq!(passing_yards.stat_id, 53);
        assert_eq!(passing_yards.points, 0.04);
        assert_eq!(passing_yards.points_overrides.len(), 2);
        assert_eq!(passing_yards.points_overrides[&0], 0.025);
        assert_eq!(passing_yards.points_overrides[&2], 0.05);

        let passing_tds = &settings.scoring_settings.scoring_items[1];
        assert_eq!(passing_tds.stat_id, 1);
        assert_eq!(passing_tds.points, 6.0);
        assert_eq!(passing_tds.points_overrides.len(), 3);
        assert_eq!(passing_tds.points_overrides[&0], 4.0);
        assert_eq!(passing_tds.points_overrides[&1], 6.0);
        assert_eq!(passing_tds.points_overrides[&2], 8.0);
    }

    #[test]
    fn test_cached_league_data_empty_scoring_items() {
        // Test handling of empty scoring items
        let empty_json = json!({
            "scoringSettings": {
                "scoringItems": []
            }
        });

        let cached_data: CachedLeagueData = serde_json::from_value(empty_json).unwrap();
        let settings = cached_data.into_settings();

        assert_eq!(settings.scoring_settings.scoring_items.len(), 0);
    }
}
