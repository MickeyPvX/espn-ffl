use crate::cli_types::{PlayerId, Season, Week};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

fn de_str_key_map_u8_f64<'de, D>(deserializer: D) -> Result<BTreeMap<u8, f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: BTreeMap<String, f64> = Deserialize::deserialize(deserializer)?;
    raw.into_iter()
        .map(|(k, v)| k.parse::<u8>().map(|kk| (kk, v)).map_err(D::Error::custom))
        .collect()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScoringItem {
    #[serde(rename = "statId")]
    pub stat_id: u16,
    /// Base points for this stat (used when no override exists for the player's slot)
    pub points: f64,
    /// Overrides by lineup slot id (keys come in as strings)
    #[serde(
        rename = "pointsOverrides",
        deserialize_with = "de_str_key_map_u8_f64",
        default
    )]
    pub points_overrides: BTreeMap<u8, f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScoringSettings {
    #[serde(rename = "scoringItems")]
    pub scoring_items: Vec<ScoringItem>,
}

/// Root we deserialize out of mSettings
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeagueSettings {
    #[serde(rename = "scoringSettings")]
    pub scoring_settings: ScoringSettings,
}

/// Top-level envelope for mSettings
#[derive(Deserialize)]
pub struct LeagueEnvelope {
    pub settings: LeagueSettings,
}

/// Player data from ESPN API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Player {
    pub id: PlayerId,
    #[serde(rename = "fullName", default)]
    pub full_name: Option<String>,
    #[serde(rename = "defaultPositionId")]
    pub default_position_id: i8,
    #[serde(default)]
    pub stats: Vec<PlayerStats>,
}

/// Player statistics for a specific period
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlayerStats {
    #[serde(rename = "seasonId")]
    pub season_id: Season,
    #[serde(rename = "scoringPeriodId")]
    pub scoring_period_id: Week,
    #[serde(rename = "statSourceId")]
    pub stat_source_id: u8,
    #[serde(rename = "statSplitTypeId")]
    pub stat_split_type_id: u8,
    #[serde(default)]
    pub stats: BTreeMap<String, f64>,
}

/// Computed player points for display
#[derive(Debug, Clone, Serialize)]
pub struct PlayerPoints {
    pub id: PlayerId,
    pub name: String,
    pub position: String,
    pub week: Week,
    pub projected: bool,
    pub points: f64,
}
