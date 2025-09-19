//! Type-safe wrappers and enums for ESPN Fantasy Football data.

use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::error::{EspnError, Result};

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
            2 => Ok(Position::RB),
            4 => Ok(Position::WR),
            6 => Ok(Position::TE),
            16 => Ok(Position::D),
            17 => Ok(Position::K),
            23 => Ok(Position::FLEX),
            _ => Err(format!("Unknown Position ID: {}", value)),
        }
    }
}
