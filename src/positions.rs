//! Position enum and conversions to ESPN slot IDs.

use std::str::FromStr;

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

impl From<Position> for u8 {
    fn from(p: Position) -> u8 { p as u8 }
}
