//! Type-safe wrappers and enums for ESPN Fantasy Football data.

use crate::error::{EspnError, Result};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

#[cfg(test)]
mod tests;

/// Type-safe wrapper for League IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LeagueId(pub u32);

impl LeagueId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

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

impl FromStr for Season {
    type Err = EspnError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self(s.parse()?))
    }
}

impl FromStr for Week {
    type Err = EspnError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self(s.parse()?))
    }
}

/// Type-safe wrapper for Player IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct PlayerId(pub u64);

impl<'de> Deserialize<'de> for PlayerId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let id: i64 = i64::deserialize(deserializer)?;
        // Convert negative IDs to their absolute value
        // ESPN sometimes uses negative IDs for certain player types
        Ok(PlayerId(id.unsigned_abs()))
    }
}

impl PlayerId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
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

/// Type-safe wrapper for Week numbers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Week(pub u16);

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

// TODO - Re-implement with proper query for player data
// /// Availability filter for ESPN players, mapped to filterStatuses
// #[derive(Clone, Copy, Debug)]
// pub enum Availability {
//     All,    // no filterStatuses
//     Free,   // FREEAGENT + WAIVERS
//     OnTeam, // ONTEAM
// }

// impl FromStr for Availability {
//     type Err = String;
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         match s.to_lowercase().as_str() {
//             "all" => Ok(Self::All),
//             "free" => Ok(Self::Free),
//             "onteam" => Ok(Self::OnTeam),
//             other => Err(format!("Invalid availability: {other}")),
//         }
//     }
// }

/// Known ESPN lineup positions.
///
/// Backed by `u8` to match ESPN slot ID ranges.
#[derive(Debug, Eq, Hash, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum Position {
    D = 16,
    FLEX = 23,
    K = 17,
    RB = 2,
    QB = 0,
    TE = 6,
    WR = 4,
}

impl FromStr for Position {
    type Err = String;

    /// Parse user input into a `Position`, case-insensitive.
    ///
    /// Accepts common aliases like `"DEF"`, `"D/ST"`, `"DST"` for defense.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "D" | "D/ST" | "DEF" | "DST" => Ok(Self::D),
            "FLEX" => Ok(Self::FLEX),
            "K" => Ok(Self::K),
            "RB" => Ok(Self::RB),
            "QB" => Ok(Self::QB),
            "TE" => Ok(Self::TE),
            "WR" => Ok(Self::WR),
            _ => Err(format!("Unrecognized player position: {s:?}")),
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Position::QB => "QB",
            Position::RB => "RB",
            Position::WR => "WR",
            Position::TE => "TE",
            Position::K => "K",
            Position::D => "D/ST",
            Position::FLEX => "FLEX",
        };
        write!(f, "{}", s)
    }
}

impl From<Position> for u8 {
    fn from(p: Position) -> u8 {
        p as u8
    }
}

impl TryFrom<u8> for Position {
    type Error = String;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Position::QB),
            1 => Ok(Position::QB), // TQB (Team QB) - map to QB
            2 => Ok(Position::RB),
            3 => Ok(Position::RB), // RB/WR - map to RB since RB is primary
            4 => Ok(Position::WR),
            5 => Ok(Position::WR), // WR/TE - map to WR since WR is primary
            6 => Ok(Position::TE),
            7 => Ok(Position::FLEX),  // OP (Offensive Player) - map to FLEX
            8 => Ok(Position::D),     // DT (Defensive Tackle)
            9 => Ok(Position::D),     // DE (Defensive End)
            10 => Ok(Position::D),    // LB (Linebacker)
            11 => Ok(Position::D),    // DL (Defensive Line)
            12 => Ok(Position::D),    // CB (Cornerback)
            13 => Ok(Position::D),    // S (Safety)
            14 => Ok(Position::D),    // DB (Defensive Back)
            15 => Ok(Position::D),    // DP (Defensive Player)
            16 => Ok(Position::D),    // D/ST (Defense/Special Teams)
            17 => Ok(Position::K),    // K (Kicker)
            18 => Ok(Position::K),    // P (Punter) - map to K since we don't have separate punter
            23 => Ok(Position::FLEX), // RB/WR/TE (FLEX)
            _ => Err(format!("Unknown Position ID: {}", value)),
        }
    }
}
