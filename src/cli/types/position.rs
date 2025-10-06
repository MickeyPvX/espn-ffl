//! Fantasy football position types and utilities.

use crate::error::EspnError;
use std::fmt;
use std::str::FromStr;

/// Fantasy football player positions.
///
/// Represents the different positions available in ESPN Fantasy Football,
/// including individual positions and flexible roster slots.
///
/// # Position Types
///
/// - **Individual positions**: QB, RB, WR, TE, K, D/ST
/// - **Flexible positions**: FLEX (RB/WR/TE), SUPERFLEX (QB/RB/WR/TE)
/// - **Roster slots**: BE (bench), IR (injured reserve)
///
/// # Examples
///
/// ```rust
/// use espn_ffl::Position;
///
/// let qb = Position::QB;
/// let flex = Position::FLEX;
/// assert_eq!(qb.to_string(), "QB");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
pub enum Position {
    QB,
    RB,
    WR,
    TE,
    #[clap(name = "D/ST")]
    DEF,
    K,
    FLEX,
    #[clap(name = "Super FLEX")]
    SUPERFLEX,
    BE,
    IR,
}

impl Position {
    /// Get all ESPN position IDs that this position can represent.
    ///
    /// For specific positions, returns a single ID. For flexible positions
    /// like FLEX, returns multiple IDs representing all eligible positions.
    pub fn get_all_position_ids(&self) -> Vec<u8> {
        match self {
            Position::QB => vec![0],
            Position::RB => vec![2],
            Position::WR => vec![3],
            Position::TE => vec![4, 6], // TE can be position 4 or 6 in ESPN
            Position::DEF => vec![16],
            Position::K => vec![5, 17], // K can be position 5 or 17
            Position::FLEX => vec![2, 3, 4, 6], // RB, WR, TE
            Position::SUPERFLEX => vec![0, 2, 3, 4, 6], // QB, RB, WR, TE
            Position::BE => vec![0, 2, 3, 4, 5, 6, 16, 17], // All positions
            Position::IR => vec![0, 2, 3, 4, 5, 6, 16, 17], // All positions
        }
    }

    /// Convert a single ESPN position ID to a Position enum.
    ///
    /// Returns the most specific position type for the given ID.
    pub fn try_from(id: u8) -> Result<Self, EspnError> {
        match id {
            0 => Ok(Position::QB),
            2 => Ok(Position::RB),
            3 => Ok(Position::WR),
            4 | 6 => Ok(Position::TE),
            5 | 17 => Ok(Position::K),
            16 => Ok(Position::DEF),
            _ => Err(EspnError::InvalidPosition {
                position: (id as u32).to_string(),
            }),
        }
    }

    /// Get the primary ESPN position ID for this position.
    ///
    /// For positions that can have multiple IDs, returns the most common one.
    pub fn to_u8(&self) -> u8 {
        match self {
            Position::QB => 0,
            Position::RB => 2,
            Position::WR => 3,
            Position::TE => 4,
            Position::DEF => 16,
            Position::K => 5,
            Position::FLEX => 23,      // ESPN's FLEX position ID
            Position::SUPERFLEX => 25, // ESPN's SuperFlex position ID
            Position::BE => 20,        // ESPN's Bench position ID
            Position::IR => 21,        // ESPN's IR position ID
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
            Position::DEF => "D/ST",
            Position::K => "K",
            Position::FLEX => "FLEX",
            Position::SUPERFLEX => "SUPERFLEX",
            Position::BE => "BE",
            Position::IR => "IR",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for Position {
    type Err = EspnError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "QB" => Ok(Position::QB),
            "RB" => Ok(Position::RB),
            "WR" => Ok(Position::WR),
            "TE" => Ok(Position::TE),
            "DEF" | "D/ST" | "DST" => Ok(Position::DEF),
            "K" => Ok(Position::K),
            "FLEX" => Ok(Position::FLEX),
            "SUPERFLEX" | "SUPER FLEX" => Ok(Position::SUPERFLEX),
            "BE" | "BENCH" => Ok(Position::BE),
            "IR" => Ok(Position::IR),
            _ => Err(EspnError::InvalidPosition {
                position: "999".to_string(), // Use 999 for string parse errors
            }),
        }
    }
}
