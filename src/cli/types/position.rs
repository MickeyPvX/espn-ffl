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
/// - **Flexible positions**: FLEX (RB/WR/TE)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Position {
    QB,
    RB,
    WR,
    TE,
    DEF,
    K,
    FLEX,
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
            Position::QB => vec![0, 1], // ESPN uses both 0 and 1 for QB
            Position::RB => vec![2],
            Position::WR => vec![3],
            Position::TE => vec![4, 6], // TE can be position 4 or 6 in ESPN
            Position::DEF => vec![16],
            Position::K => vec![5, 17], // K can be position 5 or 17
            Position::FLEX => vec![2, 3, 4, 6], // RB, WR, TE
            Position::BE => vec![0, 1, 2, 3, 4, 5, 6, 16, 17], // All positions
            Position::IR => vec![0, 1, 2, 3, 4, 5, 6, 16, 17], // All positions
        }
    }

    /// Convert a single ESPN position ID to a Position enum.
    ///
    /// Returns the most specific position type for the given ID.
    pub fn try_from(id: u8) -> Result<Self, EspnError> {
        match id {
            0 | 1 => Ok(Position::QB), // ESPN uses both 0 and 1 for QB
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
            Position::FLEX => 23, // ESPN's FLEX position ID
            Position::BE => 20,   // ESPN's Bench position ID
            Position::IR => 21,   // ESPN's IR position ID
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
            "BE" | "BENCH" => Ok(Position::BE),
            "IR" => Ok(Position::IR),
            _ => Err(EspnError::InvalidPosition {
                position: "999".to_string(), // Use 999 for string parse errors
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_position_id_mappings() {
        // Test that all ESPN position IDs map correctly to Position enums
        // and that flexible positions include all their eligible position IDs

        // Test primary position IDs
        assert_eq!(Position::try_from(0).unwrap(), Position::QB);
        assert_eq!(Position::try_from(1).unwrap(), Position::QB); // Alternate QB ID
        assert_eq!(Position::try_from(2).unwrap(), Position::RB);
        assert_eq!(Position::try_from(3).unwrap(), Position::WR);
        assert_eq!(Position::try_from(4).unwrap(), Position::TE);
        assert_eq!(Position::try_from(5).unwrap(), Position::K);
        assert_eq!(Position::try_from(6).unwrap(), Position::TE); // Alternate TE ID
        assert_eq!(Position::try_from(16).unwrap(), Position::DEF);
        assert_eq!(Position::try_from(17).unwrap(), Position::K); // Alternate K ID

        // Test invalid position ID
        assert!(Position::try_from(99).is_err());

        // Test that get_all_position_ids includes all variants
        assert_eq!(Position::QB.get_all_position_ids(), vec![0, 1]);
        assert_eq!(Position::RB.get_all_position_ids(), vec![2]);
        assert_eq!(Position::WR.get_all_position_ids(), vec![3]);
        assert_eq!(Position::TE.get_all_position_ids(), vec![4, 6]);
        assert_eq!(Position::K.get_all_position_ids(), vec![5, 17]);
        assert_eq!(Position::DEF.get_all_position_ids(), vec![16]);

        // Test FLEX includes RB, WR, TE
        let flex_ids = Position::FLEX.get_all_position_ids();
        assert!(flex_ids.contains(&2)); // RB
        assert!(flex_ids.contains(&3)); // WR
        assert!(flex_ids.contains(&4)); // TE primary
        assert!(flex_ids.contains(&6)); // TE alternate
        assert!(!flex_ids.contains(&0)); // Not QB
        assert!(!flex_ids.contains(&5)); // Not K
    }

    #[test]
    fn test_position_string_conversion() {
        // Test that position enums convert to correct strings
        assert_eq!(Position::QB.to_string(), "QB");
        assert_eq!(Position::RB.to_string(), "RB");
        assert_eq!(Position::WR.to_string(), "WR");
        assert_eq!(Position::TE.to_string(), "TE");
        assert_eq!(Position::K.to_string(), "K");
        assert_eq!(Position::DEF.to_string(), "D/ST");
        assert_eq!(Position::FLEX.to_string(), "FLEX");
    }

    #[test]
    fn test_position_primary_ids() {
        // Test that to_u8() returns the primary/most common ID
        assert_eq!(Position::QB.to_u8(), 0);
        assert_eq!(Position::RB.to_u8(), 2);
        assert_eq!(Position::WR.to_u8(), 3);
        assert_eq!(Position::TE.to_u8(), 4); // Primary TE ID is 4, not 6
        assert_eq!(Position::K.to_u8(), 5); // Primary K ID is 5, not 17
        assert_eq!(Position::DEF.to_u8(), 16);
    }
}
