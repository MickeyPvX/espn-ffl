use crate::cli::types::{PlayerId, Season, Week};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

/// Parameters for creating PlayerPoints from cached data to avoid too many function arguments.
#[derive(Debug)]
pub struct CachedPlayerData {
    pub player_id: PlayerId,
    pub name: String,
    pub position: String,
    pub points: f64,
    pub week: Week,
    pub projected: bool,
    pub active: Option<bool>,
    pub injured: Option<bool>,
    pub injury_status: Option<InjuryStatus>,
    pub is_rostered: Option<bool>,
    pub team_id: Option<u32>,
    pub team_name: Option<String>,
}

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

/// Player injury status
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum InjuryStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "INJURY_RESERVE")]
    InjuryReserve,
    #[serde(rename = "OUT")]
    Out,
    #[serde(rename = "DOUBTFUL")]
    Doubtful,
    #[serde(rename = "QUESTIONABLE")]
    Questionable,
    #[serde(rename = "PROBABLE")]
    Probable,
    #[serde(rename = "DAY_TO_DAY")]
    DayToDay,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for InjuryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InjuryStatus::Active => write!(f, "Active"),
            InjuryStatus::InjuryReserve => write!(f, "IR"),
            InjuryStatus::Out => write!(f, "Out"),
            InjuryStatus::Doubtful => write!(f, "Doubtful"),
            InjuryStatus::Questionable => write!(f, "Questionable"),
            InjuryStatus::Probable => write!(f, "Probable"),
            InjuryStatus::DayToDay => write!(f, "Day-to-Day"),
            InjuryStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Player data from ESPN API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Player {
    pub id: i64, // Raw ESPN ID, we'll convert to PlayerId after filtering
    #[serde(rename = "fullName", default)]
    pub full_name: Option<String>,
    #[serde(rename = "defaultPositionId")]
    pub default_position_id: i8,
    #[serde(default)]
    pub stats: Vec<PlayerStats>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub injured: Option<bool>,
    #[serde(rename = "injuryStatus", default)]
    pub injury_status: Option<InjuryStatus>,
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
    pub active: Option<bool>,
    pub injured: Option<bool>,
    pub injury_status: Option<InjuryStatus>,
    pub is_rostered: Option<bool>,
    pub team_id: Option<u32>,
    pub team_name: Option<String>,
}

impl PlayerPoints {
    /// Create a minimal PlayerPoints for testing
    #[cfg(test)]
    pub fn test_minimal(
        id: PlayerId,
        name: String,
        position: String,
        week: Week,
        projected: bool,
        points: f64,
    ) -> Self {
        Self {
            id,
            name,
            position,
            week,
            projected,
            points,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(false),
            team_id: None,
            team_name: None,
        }
    }
    /// Create PlayerPoints from cached data with injury/roster info
    pub fn from_cached_data(params: CachedPlayerData) -> Self {
        Self {
            id: params.player_id,
            name: params.name,
            position: params.position,
            points: params.points,
            week: params.week,
            projected: params.projected,
            active: params.active,
            injured: params.injured,
            injury_status: params.injury_status,
            is_rostered: params.is_rostered,
            team_id: params.team_id,
            team_name: params.team_name,
        }
    }

    /// Create PlayerPoints from PerformanceEstimate for status checking
    pub fn from_estimate(estimate: &crate::storage::models::PerformanceEstimate, week: crate::cli::types::Week) -> Self {
        Self {
            id: estimate.player_id,
            name: estimate.name.clone(),
            position: estimate.position.clone(),
            points: estimate.estimated_points,
            week,
            projected: false, // Status checking is not projection-specific
            active: None,     // Will be filled by update_player_points_with_roster_info
            injured: None,    // Will be filled by update_player_points_with_roster_info
            injury_status: None, // Will be filled by update_player_points_with_roster_info
            is_rostered: None,   // Will be filled by update_player_points_with_roster_info
            team_id: None,       // Will be filled by update_player_points_with_roster_info
            team_name: None,     // Will be filled by update_player_points_with_roster_info
        }
    }

    /// Create PlayerPoints from ESPN player data
    pub fn from_espn_player(
        player_id: PlayerId,
        player: &Player,
        position: String,
        points: f64,
        week: Week,
        projected: bool,
    ) -> Self {
        Self {
            id: player_id,
            name: player
                .full_name
                .clone()
                .unwrap_or_else(|| format!("Player {}", player.id)),
            position,
            points,
            week,
            projected,
            active: player.active,
            injured: player.injured,
            injury_status: player.injury_status.clone(),
            is_rostered: None, // Will be filled later
            team_id: None,     // Will be filled later
            team_name: None,   // Will be filled later
        }
    }
}

/// Roster entry from ESPN API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RosterEntry {
    #[serde(rename = "playerId")]
    pub player_id: i64,
    #[serde(rename = "lineupSlotId")]
    pub lineup_slot_id: u8,
    #[serde(rename = "injuryStatus")]
    pub injury_status: Option<String>,
}

/// Team roster from ESPN API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TeamRoster {
    pub entries: Vec<RosterEntry>,
}

/// Team data from ESPN API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Team {
    pub id: u32,
    pub name: Option<String>,
    pub abbrev: Option<String>,
    pub roster: Option<TeamRoster>,
}

/// League data with teams from ESPN API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeagueData {
    pub teams: Vec<Team>,
}

impl LeagueData {
    /// Create a mapping of player ID to team information
    pub fn create_player_roster_map(
        &self,
    ) -> std::collections::HashMap<i64, (u32, Option<String>, Option<String>)> {
        let mut player_to_team = std::collections::HashMap::new();

        for team in &self.teams {
            if let Some(roster) = &team.roster {
                for entry in &roster.entries {
                    player_to_team.insert(
                        entry.player_id,
                        (team.id, team.name.clone(), team.abbrev.clone()),
                    );
                }
            }
        }

        player_to_team
    }

    /// Update a list of PlayerPoints with roster information
    pub fn update_player_points_with_roster(&self, player_points: &mut [PlayerPoints]) {
        let player_to_team = self.create_player_roster_map();

        for player in player_points.iter_mut() {
            let player_id_i64 = player.id.as_u64() as i64;
            let negative_player_id_i64 = -(player_id_i64);

            // Check both positive and negative versions of the ID
            // D/ST teams often have negative IDs in roster data but positive IDs in player data
            let roster_info = player_to_team.get(&player_id_i64)
                .or_else(|| player_to_team.get(&negative_player_id_i64));

            if let Some((team_id, team_name, _team_abbrev)) = roster_info {
                player.is_rostered = Some(true);
                player.team_id = Some(*team_id);
                player.team_name = team_name.clone();
            } else {
                player.is_rostered = Some(false);
            }
        }
    }
}
