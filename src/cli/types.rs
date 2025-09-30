//! Type-safe wrappers and enums for ESPN Fantasy Football data.

use crate::error::{EspnError, Result};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

/// Type-safe wrapper for ESPN Fantasy Football League IDs.
///
/// Ensures league IDs are handled consistently throughout the application
/// and provides type safety to prevent mixing up league IDs with other numeric values.
///
/// # Examples
///
/// ```rust
/// use espn_ffl::LeagueId;
///
/// let league_id = LeagueId::new(123456);
/// assert_eq!(league_id.as_u32(), 123456);
/// assert_eq!(league_id.to_string(), "123456");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LeagueId(pub u32);

impl LeagueId {
    /// Create a new LeagueId from a u32 value.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the underlying u32 value.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for LeagueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for LeagueId {
    type Err = EspnError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self(s.parse()?))
    }
}

/// Type-safe wrapper for Player IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub u64);

impl PlayerId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type-safe wrapper for Season years
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Season(pub u16);

impl Season {
    pub fn new(year: u16) -> Self {
        Self(year)
    }

    pub fn as_u16(&self) -> u16 {
        self.0
    }
}

impl Default for Season {
    fn default() -> Self {
        Self(2025)
    }
}

impl fmt::Display for Season {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Season {
    type Err = EspnError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self(s.parse()?))
    }
}

/// Type-safe wrapper for Week numbers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Week(pub u16);

/// Filter for player injury status in CLI commands.
///
/// Allows filtering players by their current injury designation.
/// Some filters work server-side with ESPN API for efficiency, while others
/// require client-side filtering.
///
/// # Server-side vs Client-side Filtering
///
/// - **Server-side** (efficient): `Active`, `Injured`
/// - **Client-side** (less efficient): Specific statuses like `Out`, `Doubtful`, etc.
///
/// # Examples
///
/// ```rust
/// use espn_ffl::cli::types::InjuryStatusFilter;
///
/// let filter = InjuryStatusFilter::Active;
/// assert_eq!(filter.to_string(), "Active");
/// ```
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum InjuryStatusFilter {
    /// Players who are active/healthy
    Active,
    /// Any player with an injury designation
    Injured,
    /// Players ruled out for the game
    Out,
    /// Players with doubtful injury status
    Doubtful,
    /// Players with questionable injury status
    Questionable,
    /// Players with probable injury status
    Probable,
    /// Players listed as day-to-day
    DayToDay,
    /// Players on Injury Reserve
    IR,
}

impl fmt::Display for InjuryStatusFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InjuryStatusFilter::Active => write!(f, "Active"),
            InjuryStatusFilter::Injured => write!(f, "Injured"),
            InjuryStatusFilter::Out => write!(f, "Out"),
            InjuryStatusFilter::Doubtful => write!(f, "Doubtful"),
            InjuryStatusFilter::Questionable => write!(f, "Questionable"),
            InjuryStatusFilter::Probable => write!(f, "Probable"),
            InjuryStatusFilter::DayToDay => write!(f, "Day-to-Day"),
            InjuryStatusFilter::IR => write!(f, "IR"),
        }
    }
}

/// Roster status filter for CLI
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum RosterStatusFilter {
    Rostered, // On any team in the league
    FA,       // Free agent (not on any team)
}

impl fmt::Display for RosterStatusFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RosterStatusFilter::Rostered => write!(f, "Rostered"),
            RosterStatusFilter::FA => write!(f, "Free Agent"),
        }
    }
}

impl Week {
    pub fn new(week: u16) -> Self {
        Self(week)
    }

    pub fn as_u16(&self) -> u16 {
        self.0
    }
}

impl Default for Week {
    fn default() -> Self {
        Self(1)
    }
}

impl fmt::Display for Week {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Week {
    type Err = EspnError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self(s.parse()?))
    }
}

/// Fantasy football player positions.
///
/// Represents the different positions available in ESPN Fantasy Football,
/// including individual positions and flexible roster slots.
///
/// # Examples
///
/// ```rust
/// use espn_ffl::Position;
///
/// let qb = Position::QB;
/// let flex = Position::FLEX;
///
/// // Get ESPN position ID
/// assert_eq!(qb.to_u8(), 0);
///
/// // Check position eligibility
/// assert!(Position::FLEX.get_eligible_positions().contains(&Position::RB));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Position {
    /// Quarterback
    QB,
    /// Running Back
    RB,
    /// Wide Receiver
    WR,
    /// Tight End
    TE,
    /// Kicker
    K,
    /// Defense/Special Teams
    DEF,
    /// Flexible position (RB/WR/TE)
    FLEX,
}

impl Position {
    /// Convert position to ESPN's internal position ID.
    ///
    /// ESPN uses numeric IDs to represent different positions in their API.
    /// This method returns the primary position ID for each position type.
    pub fn to_u8(&self) -> u8 {
        match self {
            Position::QB => 0,
            Position::RB => 2,
            Position::WR => 3, // ESPN actually uses 3 for WRs in responses
            Position::TE => 4, // ESPN uses 4 for pass-catching TEs
            Position::K => 5,  // ESPN uses 5 for kickers in responses
            Position::DEF => 16,
            Position::FLEX => 23,
        }
    }

    /// Returns the positions eligible for this position type.
    /// For FLEX, returns RB, WR, TE. For others, returns the position itself.
    pub fn get_eligible_positions(&self) -> Vec<Position> {
        match self {
            Position::FLEX => vec![Position::RB, Position::WR, Position::TE],
            other => vec![other.clone()],
        }
    }

    /// Returns all possible ESPN position IDs for this position type.
    /// Some positions like K can have multiple IDs (5, 17, 18).
    pub fn get_all_position_ids(&self) -> Vec<u8> {
        match self {
            Position::QB => vec![0, 1], // QB and TQB
            Position::RB => vec![2],
            Position::WR => vec![3],
            Position::TE => vec![4, 6],
            Position::K => vec![5, 17, 18], // K, K, P (punter)
            Position::DEF => vec![16],
            Position::FLEX => vec![2, 3, 4, 6], // RB, WR, TE positions
        }
    }
}

impl TryFrom<u8> for Position {
    type Error = EspnError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Position::QB),
            1 => Ok(Position::QB), // TQB (Team QB) - map to QB
            2 => Ok(Position::RB),
            3 => Ok(Position::WR), // RB/WR - map to WR since these are typically WRs
            4 => Ok(Position::TE), // Pass-catching TEs (Kelce, Andrews, etc.)
            5 => Ok(Position::K),  // Kickers (was incorrectly mapped to TE)
            6 => Ok(Position::TE),
            7 => Ok(Position::FLEX),  // OP (Offensive Player) - map to FLEX
            16 => Ok(Position::DEF),  // D/ST (Defense/Special Teams) - only allowed defense
            17 => Ok(Position::K),    // K (Kicker)
            18 => Ok(Position::K),    // P (Punter) - map to K since we don't have separate punter
            23 => Ok(Position::FLEX), // RB/WR/TE (FLEX)
            // Reject individual defensive players (8-15) - league doesn't allow them
            8..=15 => Err(EspnError::InvalidPosition {
                position: format!(
                    "Individual defensive players not allowed in this league (position ID: {})",
                    value
                ),
            }),
            _ => Err(EspnError::InvalidPosition {
                position: format!("Unknown position ID: {}", value),
            }),
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Position::QB => write!(f, "QB"),
            Position::RB => write!(f, "RB"),
            Position::WR => write!(f, "WR"),
            Position::TE => write!(f, "TE"),
            Position::K => write!(f, "K"),
            Position::DEF => write!(f, "D/ST"),
            Position::FLEX => write!(f, "FLEX"),
        }
    }
}

impl FromStr for Position {
    type Err = EspnError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "QB" => Ok(Position::QB),
            "RB" => Ok(Position::RB),
            "WR" => Ok(Position::WR),
            "TE" => Ok(Position::TE),
            "K" => Ok(Position::K),
            "D/ST" | "DEF" | "DST" => Ok(Position::DEF),
            "FLEX" => Ok(Position::FLEX),
            _ => Err(EspnError::InvalidPosition {
                position: format!("Unknown position: {}", s),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_league_id_new() {
        let id = LeagueId::new(12345);
        assert_eq!(id.as_u32(), 12345);
    }

    #[test]
    fn test_league_id_display() {
        let id = LeagueId::new(12345);
        assert_eq!(format!("{}", id), "12345");
    }

    #[test]
    fn test_league_id_from_str_valid() {
        let id: LeagueId = "12345".parse().unwrap();
        assert_eq!(id.as_u32(), 12345);
    }

    #[test]
    fn test_league_id_from_str_invalid() {
        let result: Result<LeagueId> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_league_id_from_str_negative() {
        let result: Result<LeagueId> = "-1".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_league_id_from_str_zero() {
        let id: LeagueId = "0".parse().unwrap();
        assert_eq!(id.as_u32(), 0);
    }

    #[test]
    fn test_league_id_serde() {
        let id = LeagueId::new(12345);
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: LeagueId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_player_id_new() {
        let id = PlayerId::new(67890);
        assert_eq!(id.as_u64(), 67890);
    }

    #[test]
    fn test_player_id_zero() {
        let id = PlayerId::new(0);
        assert_eq!(id.as_u64(), 0);
    }

    #[test]
    fn test_player_id_max_value() {
        let id = PlayerId::new(u64::MAX);
        assert_eq!(id.as_u64(), u64::MAX);
    }

    #[test]
    fn test_player_id_serde() {
        let id = PlayerId::new(67890);
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: PlayerId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_season_new() {
        let season = Season::new(2023);
        assert_eq!(season.as_u16(), 2023);
    }

    #[test]
    fn test_season_default() {
        let season = Season::default();
        assert_eq!(season.as_u16(), 2025);
    }

    #[test]
    fn test_season_display() {
        let season = Season::new(2023);
        assert_eq!(format!("{}", season), "2023");
    }

    #[test]
    fn test_season_from_str_valid() {
        let season: Season = "2023".parse().unwrap();
        assert_eq!(season.as_u16(), 2023);
    }

    #[test]
    fn test_season_from_str_invalid() {
        let result: Result<Season> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_season_from_str_future() {
        let season: Season = "2030".parse().unwrap();
        assert_eq!(season.as_u16(), 2030);
    }

    #[test]
    fn test_season_serde() {
        let season = Season::new(2023);
        let json = serde_json::to_string(&season).unwrap();
        let deserialized: Season = serde_json::from_str(&json).unwrap();
        assert_eq!(season, deserialized);
    }

    #[test]
    fn test_week_new() {
        let week = Week::new(5);
        assert_eq!(week.as_u16(), 5);
    }

    #[test]
    fn test_week_default() {
        let week = Week::default();
        assert_eq!(week.as_u16(), 1);
    }

    #[test]
    fn test_week_display() {
        let week = Week::new(5);
        assert_eq!(format!("{}", week), "5");
    }

    #[test]
    fn test_week_from_str_valid() {
        let week: Week = "5".parse().unwrap();
        assert_eq!(week.as_u16(), 5);
    }

    #[test]
    fn test_week_from_str_invalid() {
        let result: Result<Week> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_week_zero() {
        let week = Week::new(0);
        assert_eq!(week.as_u16(), 0);
    }

    #[test]
    fn test_week_large_number() {
        let week = Week::new(1000);
        assert_eq!(week.as_u16(), 1000);
    }

    #[test]
    fn test_week_serde() {
        let week = Week::new(5);
        let json = serde_json::to_string(&week).unwrap();
        let deserialized: Week = serde_json::from_str(&json).unwrap();
        assert_eq!(week, deserialized);
    }

    #[test]
    fn test_position_display() {
        assert_eq!(format!("{}", Position::QB), "QB");
        assert_eq!(format!("{}", Position::RB), "RB");
        assert_eq!(format!("{}", Position::WR), "WR");
        assert_eq!(format!("{}", Position::TE), "TE");
        assert_eq!(format!("{}", Position::K), "K");
        assert_eq!(format!("{}", Position::DEF), "D/ST");
    }

    #[test]
    fn test_position_from_str_standard() {
        assert_eq!("QB".parse::<Position>().unwrap(), Position::QB);
        assert_eq!("RB".parse::<Position>().unwrap(), Position::RB);
        assert_eq!("WR".parse::<Position>().unwrap(), Position::WR);
        assert_eq!("TE".parse::<Position>().unwrap(), Position::TE);
        assert_eq!("K".parse::<Position>().unwrap(), Position::K);
    }

    #[test]
    fn test_position_from_str_defense_aliases() {
        assert_eq!("DEF".parse::<Position>().unwrap(), Position::DEF);
        assert_eq!("DST".parse::<Position>().unwrap(), Position::DEF);
        assert_eq!("D/ST".parse::<Position>().unwrap(), Position::DEF);
    }

    #[test]
    fn test_position_from_str_case_insensitive() {
        assert_eq!("qb".parse::<Position>().unwrap(), Position::QB);
        assert_eq!("Rb".parse::<Position>().unwrap(), Position::RB);
        assert_eq!("WR".parse::<Position>().unwrap(), Position::WR);
    }

    #[test]
    fn test_position_from_str_invalid() {
        let result: Result<Position> = "INVALID".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_position_to_u8() {
        assert_eq!(Position::QB.to_u8(), 0);
        assert_eq!(Position::RB.to_u8(), 2);
        assert_eq!(Position::WR.to_u8(), 3);
        assert_eq!(Position::TE.to_u8(), 4);
        assert_eq!(Position::K.to_u8(), 5);
        assert_eq!(Position::DEF.to_u8(), 16);
    }

    #[test]
    fn test_position_try_from_u8_valid() {
        assert_eq!(Position::try_from(0).unwrap(), Position::QB);
        assert_eq!(Position::try_from(2).unwrap(), Position::RB);
        assert_eq!(Position::try_from(3).unwrap(), Position::WR);
        assert_eq!(Position::try_from(4).unwrap(), Position::TE);
        assert_eq!(Position::try_from(5).unwrap(), Position::K);
        assert_eq!(Position::try_from(6).unwrap(), Position::TE);
        assert_eq!(Position::try_from(17).unwrap(), Position::K);
        assert_eq!(Position::try_from(16).unwrap(), Position::DEF);
    }

    #[test]
    fn test_position_try_from_u8_invalid() {
        let result = Position::try_from(99);
        assert!(result.is_err());
    }

    #[test]
    fn test_position_roundtrip_conversion() {
        let positions = vec![
            Position::QB,
            Position::RB,
            Position::WR,
            Position::TE,
            Position::K,
            Position::DEF,
        ];

        for pos in positions {
            let u8_val = pos.to_u8();
            let converted_back = Position::try_from(u8_val).unwrap();
            assert_eq!(pos, converted_back);
        }
    }
}
