use crate::{PlayerId, Season, Week};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

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

/// Roster settings from league configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RosterSettings {
    #[serde(rename = "lineupSlotCounts")]
    pub lineup_slot_counts: std::collections::HashMap<String, u32>,
    #[serde(rename = "positionLimits")]
    pub position_limits: std::collections::HashMap<String, i32>,
}

/// Draft configuration from league settings
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DraftSettings {
    /// `SNAKE`, `AUCTION`, ...
    #[serde(rename = "type", default)]
    pub draft_type: Option<String>,
    /// Scheduled draft time as milliseconds since the Unix epoch.
    #[serde(default)]
    pub date: Option<i64>,
    /// Team ids in first-round pick order.
    #[serde(rename = "pickOrder", default)]
    pub pick_order: Option<Vec<u32>>,
    #[serde(rename = "auctionBudget", default)]
    pub auction_budget: Option<u32>,
    #[serde(rename = "timePerSelection", default)]
    pub time_per_selection: Option<u32>,
    #[serde(rename = "keeperCount", default)]
    pub keeper_count: Option<u32>,
}

/// Root we deserialize out of mSettings
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeagueSettings {
    #[serde(rename = "scoringSettings")]
    pub scoring_settings: ScoringSettings,
    #[serde(rename = "rosterSettings")]
    pub roster_settings: RosterSettings,
    #[serde(rename = "draftSettings", default)]
    pub draft_settings: DraftSettings,
    /// Number of teams in the league.
    #[serde(default)]
    pub size: Option<u32>,
    /// League display name.
    #[serde(default)]
    pub name: Option<String>,
}

impl LeagueSettings {
    /// Lineup slot ids this league actually rosters, with the number of each.
    ///
    /// ESPN returns every slot id 0..=24 in `lineupSlotCounts` regardless of whether the league
    /// uses it, so slots with a zero count must be dropped — otherwise "which slots does this
    /// league use" answers "all of them".
    pub fn active_lineup_slots(&self) -> std::collections::BTreeMap<u8, u32> {
        self.roster_settings
            .lineup_slot_counts
            .iter()
            .filter(|(_, &count)| count > 0)
            .filter_map(|(slot, &count)| slot.parse::<u8>().ok().map(|s| (s, count)))
            .collect()
    }

    /// Slots that make up a starting lineup, excluding bench and IR.
    ///
    /// This is what determines how many players at each position the league starts, and
    /// therefore the replacement level a draft board measures value against.
    pub fn starting_lineup_slots(&self) -> std::collections::BTreeMap<u8, u32> {
        const BENCH: u8 = 20;
        const INJURED_RESERVE: u8 = 21;

        self.active_lineup_slots()
            .into_iter()
            .filter(|(slot, _)| *slot != BENCH && *slot != INJURED_RESERVE)
            .collect()
    }

    /// Lineup slot ids to request from ESPN when no explicit position filter is given.
    ///
    /// Derived from the league's own starting lineup, so an IDP league automatically keeps
    /// its defensive slots while a standard league stops downloading them.
    pub fn rosterable_slot_ids(&self) -> Vec<u8> {
        let mut slots: Vec<u8> = self
            .rosterable_positions()
            .into_iter()
            .flat_map(|p| p.lineup_slot_ids())
            .collect();
        slots.sort_unstable();
        slots.dedup();
        slots
    }

    /// Positions that can occupy a starting slot in this league.
    ///
    /// Used to drop players the league cannot start (individual defensive players, coaches)
    /// from rankings and analysis.
    pub fn rosterable_positions(&self) -> std::collections::HashSet<crate::Position> {
        use crate::Position;

        const ALL_POSITIONS: [Position; 12] = [
            Position::QB,
            Position::RB,
            Position::WR,
            Position::TE,
            Position::K,
            Position::DEF,
            Position::P,
            Position::DT,
            Position::DE,
            Position::LB,
            Position::DB,
            Position::S,
        ];

        let starting_slots = self.starting_lineup_slots();

        ALL_POSITIONS
            .into_iter()
            .filter(|pos| starting_slots.keys().any(|slot| pos.fills_slot(*slot)))
            .collect()
    }
}

/// Top-level envelope for mSettings
#[derive(Deserialize)]
pub struct LeagueEnvelope {
    pub settings: LeagueSettings,
}

/// Player injury status
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
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
    #[default]
    #[serde(other)]
    Unknown,
}

impl InjuryStatus {
    /// Canonical string form, used both for display and for database persistence.
    ///
    /// Display and storage share one table so a value written to the database always reads
    /// back as itself; previously they were separate hand-written matches that could drift.
    pub fn as_str(&self) -> &'static str {
        match self {
            InjuryStatus::Active => "Active",
            InjuryStatus::InjuryReserve => "IR",
            InjuryStatus::Out => "Out",
            InjuryStatus::Doubtful => "Doubtful",
            InjuryStatus::Questionable => "Questionable",
            InjuryStatus::Probable => "Probable",
            InjuryStatus::DayToDay => "Day-to-Day",
            InjuryStatus::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for InjuryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for InjuryStatus {
    type Err = std::convert::Infallible;

    /// Parse the canonical form written by [`InjuryStatus::as_str`].
    ///
    /// Also accepts ESPN's own wire spellings so rows written from raw API values still
    /// resolve. Anything unrecognised becomes `Unknown` rather than failing a whole query.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "Active" | "ACTIVE" => InjuryStatus::Active,
            "IR" | "INJURY_RESERVE" => InjuryStatus::InjuryReserve,
            "Out" | "OUT" => InjuryStatus::Out,
            "Doubtful" | "DOUBTFUL" => InjuryStatus::Doubtful,
            "Questionable" | "QUESTIONABLE" => InjuryStatus::Questionable,
            "Probable" | "PROBABLE" => InjuryStatus::Probable,
            "Day-to-Day" | "DAY_TO_DAY" => InjuryStatus::DayToDay,
            _ => InjuryStatus::Unknown,
        })
    }
}

/// Draft market data ESPN publishes for a player.
///
/// Populated only on bounded preseason `/players` queries; an unbounded request omits it.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Ownership {
    /// Average position at which the player is being drafted across ESPN leagues.
    #[serde(rename = "averageDraftPosition", default)]
    pub average_draft_position: Option<f64>,
    /// Average auction price paid, for auction leagues.
    #[serde(rename = "auctionValueAverage", default)]
    pub auction_value_average: Option<f64>,
    #[serde(rename = "percentOwned", default)]
    pub percent_owned: Option<f64>,
    #[serde(rename = "percentStarted", default)]
    pub percent_started: Option<f64>,
}

/// One of ESPN's draft rankings for a player.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DraftRank {
    pub rank: i32,
    #[serde(rename = "auctionValue", default)]
    pub auction_value: Option<i32>,
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
    /// NFL team id; 0 means free agent / unsigned.
    #[serde(rename = "proTeamId", default)]
    pub pro_team_id: Option<u32>,
    #[serde(default)]
    pub ownership: Option<Ownership>,
    /// Draft ranks keyed by ranking type (`PPR`, `STANDARD`, `SUPERFLEX`, ...).
    #[serde(rename = "draftRanksByRankType", default)]
    pub draft_ranks: Option<std::collections::HashMap<String, DraftRank>>,
}

impl Player {
    /// ESPN's published draft rank for the given ranking type, if present.
    pub fn draft_rank(&self, rank_type: &str) -> Option<i32> {
        self.draft_ranks.as_ref()?.get(rank_type).map(|r| r.rank)
    }

    /// Average draft position, if ESPN has published one.
    pub fn average_draft_position(&self) -> Option<f64> {
        self.ownership.as_ref()?.average_draft_position
    }

    /// Average auction value, if ESPN has published one.
    pub fn auction_value(&self) -> Option<f64> {
        self.ownership.as_ref()?.auction_value_average
    }
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
    pub fn from_estimate(
        estimate: &crate::storage::models::PerformanceEstimate,
        week: crate::Week,
    ) -> Self {
        Self {
            id: estimate.player_id,
            name: estimate.name.clone(),
            position: estimate.position.clone(),
            points: estimate.estimated_points,
            week,
            projected: false,    // Status checking is not projection-specific
            active: None,        // Will be filled by update_player_points_with_roster_info
            injured: None,       // Will be filled by update_player_points_with_roster_info
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

#[cfg(test)]
mod injury_status_tests {
    use super::InjuryStatus;

    const ALL: [InjuryStatus; 8] = [
        InjuryStatus::Active,
        InjuryStatus::InjuryReserve,
        InjuryStatus::Out,
        InjuryStatus::Doubtful,
        InjuryStatus::Questionable,
        InjuryStatus::Probable,
        InjuryStatus::DayToDay,
        InjuryStatus::Unknown,
    ];

    /// The database persists `as_str()` output, so every variant must parse back to itself.
    #[test]
    fn round_trips_through_its_stored_form() {
        for status in ALL {
            let stored = status.as_str();
            assert_eq!(
                stored.parse::<InjuryStatus>().unwrap(),
                status,
                "{} did not round trip",
                stored
            );
        }
    }

    #[test]
    fn accepts_espn_wire_spellings() {
        assert_eq!(
            "INJURY_RESERVE".parse::<InjuryStatus>().unwrap(),
            InjuryStatus::InjuryReserve
        );
        assert_eq!(
            "DAY_TO_DAY".parse::<InjuryStatus>().unwrap(),
            InjuryStatus::DayToDay
        );
    }

    #[test]
    fn unrecognised_values_become_unknown() {
        assert_eq!(
            "SOMETHING_NEW".parse::<InjuryStatus>().unwrap(),
            InjuryStatus::Unknown
        );
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
    /// SWIDs of this team's owners.
    #[serde(default)]
    pub owners: Option<Vec<String>>,
}

impl Team {
    /// Best available human-readable label for the team.
    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.abbrev.clone())
            .unwrap_or_else(|| format!("Team {}", self.id))
    }
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
            let player_id_i64 = player.id.as_i64();

            // Check exact player ID match (no positive/negative conversion)
            let roster_info = player_to_team.get(&player_id_i64);

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
