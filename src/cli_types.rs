//! Position enum and conversions to ESPN slot IDs.

use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

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
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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

    fn try_from(value: u8) -> Result<Self, Self::Error> {
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
