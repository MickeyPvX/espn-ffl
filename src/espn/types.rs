use serde::{Deserialize, Deserializer, Serialize, de::Error};
use std::collections::BTreeMap;

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
