//! Fantasy football position types and utilities.
//!
//! # ESPN's two ID spaces
//!
//! ESPN uses two *different* numeric tables that are easy to conflate:
//!
//! - **`defaultPositionId`** — what a player *is* (QB = 1, RB = 2, WR = 3, TE = 4, K = 5, D/ST = 16).
//!   This is what appears on a player object.
//! - **`lineupSlotId`** — a *roster slot* a player may occupy (QB = 0, RB = 2, WR = 4, TE = 6,
//!   D/ST = 16, K = 17, FLEX = 23, BE = 20, IR = 21). This is what `eligibleSlots`,
//!   `lineupSlotCounts` and the API's `filterSlotIds` filter speak.
//!
//! They overlap numerically without agreeing (slot 4 is WR, position 4 is TE), so the two are
//! kept strictly apart here: [`Position::from_default_position_id`] / [`Position::default_position_id`]
//! for the former, [`Position::lineup_slot_ids`] for the latter.

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
/// assert_eq!(qb.to_string(), "QB");
///
/// // A player object carries a defaultPositionId...
/// assert_eq!(Position::from_default_position_id(1).unwrap(), Position::QB);
/// // ...but the API's filterSlotIds wants lineup slot ids.
/// assert_eq!(Position::QB.lineup_slot_ids(), vec![0]);
/// ```
/// Variants are ordered as they conventionally appear on a draft board (skill positions
/// first, then kicker and defense), and `Ord` follows that declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    QB,
    RB,
    WR,
    TE,
    DEF,
    K,
    P,  // Punter
    DT, // Defensive Tackle
    DE, // Defensive End
    LB, // Linebacker
    DB, // Defensive Back/Cornerback
    S,  // Safety
    FLEX,
    BE,
    IR,
}

/// Lineup slot id for the bench, which every player is eligible for.
const SLOT_BENCH: u8 = 20;
/// Lineup slot id for injured reserve, which every player is eligible for.
const SLOT_IR: u8 = 21;

impl Position {
    /// Lineup slot ids matching this position, for the API's `filterSlotIds` filter.
    ///
    /// A player is returned by `filterSlotIds` when *any* of their `eligibleSlots` matches,
    /// so a single slot per position is enough to select exactly that position. FLEX (23)
    /// is eligible only for RB/WR/TE, which makes it an exact FLEX selector.
    pub fn lineup_slot_ids(&self) -> Vec<u8> {
        match self {
            Position::QB => vec![0],
            Position::RB => vec![2],
            Position::WR => vec![4],
            Position::TE => vec![6],
            Position::DT => vec![8],
            Position::DE => vec![9],
            Position::LB => vec![10],
            Position::DB => vec![12],
            Position::S => vec![13],
            Position::DEF => vec![16],
            Position::K => vec![17],
            Position::P => vec![18],
            Position::FLEX => vec![23],
            Position::BE => vec![SLOT_BENCH],
            Position::IR => vec![SLOT_IR],
        }
    }

    /// Convert an ESPN `defaultPositionId` (from a player object) into a `Position`.
    ///
    /// Rejects non-roster entries such as head coaches (14) and team-QB aggregates (15).
    pub fn from_default_position_id(id: u8) -> Result<Self, EspnError> {
        match id {
            1 => Ok(Position::QB),
            2 => Ok(Position::RB),
            3 => Ok(Position::WR),
            4 => Ok(Position::TE),
            5 => Ok(Position::K),
            7 => Ok(Position::P),
            9 => Ok(Position::DT),
            10 => Ok(Position::DE),
            11 => Ok(Position::LB),
            12 => Ok(Position::DB),
            13 => Ok(Position::S),
            16 => Ok(Position::DEF),
            14 => Err(EspnError::InvalidPosition {
                position: format!("COACH (id: {})", id),
            }),
            15 => Err(EspnError::InvalidPosition {
                position: format!("TEAM_QB (id: {})", id),
            }),
            _ => Err(EspnError::InvalidPosition {
                position: (id as u32).to_string(),
            }),
        }
    }

    /// The ESPN `defaultPositionId` for this position.
    ///
    /// Returns `None` for slot-only pseudo-positions (FLEX, BE, IR), which describe a
    /// roster slot rather than a kind of player.
    pub fn default_position_id(&self) -> Option<u8> {
        match self {
            Position::QB => Some(1),
            Position::RB => Some(2),
            Position::WR => Some(3),
            Position::TE => Some(4),
            Position::K => Some(5),
            Position::P => Some(7),
            Position::DT => Some(9),
            Position::DE => Some(10),
            Position::LB => Some(11),
            Position::DB => Some(12),
            Position::S => Some(13),
            Position::DEF => Some(16),
            Position::FLEX | Position::BE | Position::IR => None,
        }
    }

    /// Whether this position can fill the given lineup slot.
    ///
    /// FLEX accepts RB/WR/TE; bench and IR accept anyone.
    pub fn fills_slot(&self, slot: u8) -> bool {
        match slot {
            SLOT_BENCH | SLOT_IR => true,
            23 => matches!(self, Position::RB | Position::WR | Position::TE),
            _ => self.lineup_slot_ids().contains(&slot),
        }
    }

    /// Positions that occupy a starting lineup slot in a standard league.
    ///
    /// Used to decide which positions a draft board should rank.
    pub fn is_offensive_skill(&self) -> bool {
        matches!(
            self,
            Position::QB | Position::RB | Position::WR | Position::TE
        )
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
            Position::P => "P",
            Position::DT => "DT",
            Position::DE => "DE",
            Position::LB => "LB",
            Position::DB => "DB",
            Position::S => "S",
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
            "P" => Ok(Position::P),
            "DT" => Ok(Position::DT),
            "DE" => Ok(Position::DE),
            "LB" => Ok(Position::LB),
            "DB" => Ok(Position::DB),
            "S" => Ok(Position::S),
            "FLEX" => Ok(Position::FLEX),
            "BE" | "BENCH" => Ok(Position::BE),
            "IR" => Ok(Position::IR),
            other => Err(EspnError::InvalidPosition {
                position: other.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Slot ids verified against live `filterSlotIds` responses: querying each slot returns
    /// players of exactly one `defaultPositionId` (except FLEX, which returns RB/WR/TE).
    #[test]
    fn test_lineup_slot_ids_are_position_exact() {
        assert_eq!(Position::QB.lineup_slot_ids(), vec![0]);
        assert_eq!(Position::RB.lineup_slot_ids(), vec![2]);
        assert_eq!(Position::WR.lineup_slot_ids(), vec![4]);
        assert_eq!(Position::TE.lineup_slot_ids(), vec![6]);
        assert_eq!(Position::DEF.lineup_slot_ids(), vec![16]);
        assert_eq!(Position::K.lineup_slot_ids(), vec![17]);
        assert_eq!(Position::FLEX.lineup_slot_ids(), vec![23]);
    }

    #[test]
    fn test_default_position_id_round_trip() {
        for pos in [
            Position::QB,
            Position::RB,
            Position::WR,
            Position::TE,
            Position::K,
            Position::P,
            Position::DT,
            Position::DE,
            Position::LB,
            Position::DB,
            Position::S,
            Position::DEF,
        ] {
            let id = pos.default_position_id().expect("real position has an id");
            assert_eq!(
                Position::from_default_position_id(id).unwrap(),
                pos,
                "round trip failed for {}",
                pos
            );
        }
    }

    #[test]
    fn test_slot_only_positions_have_no_default_position_id() {
        assert_eq!(Position::FLEX.default_position_id(), None);
        assert_eq!(Position::BE.default_position_id(), None);
        assert_eq!(Position::IR.default_position_id(), None);
    }

    #[test]
    fn test_from_default_position_id_rejects_non_players() {
        // 14 = head coach, 15 = team QB aggregate; neither is a draftable player.
        assert!(Position::from_default_position_id(14).is_err());
        assert!(Position::from_default_position_id(15).is_err());
        assert!(Position::from_default_position_id(99).is_err());
        // 0 is not a valid defaultPositionId (it is the QB *slot*).
        assert!(Position::from_default_position_id(0).is_err());
    }

    #[test]
    fn test_position_and_slot_spaces_differ() {
        // The regression this module guards: slot 4 is WR, but position 4 is TE.
        assert_eq!(Position::WR.lineup_slot_ids(), vec![4]);
        assert_eq!(Position::from_default_position_id(4).unwrap(), Position::TE);
        // Likewise slot 17 is K while position 17 is meaningless.
        assert_eq!(Position::K.lineup_slot_ids(), vec![17]);
        assert!(Position::from_default_position_id(17).is_err());
    }

    #[test]
    fn test_fills_slot() {
        assert!(Position::RB.fills_slot(23));
        assert!(Position::WR.fills_slot(23));
        assert!(Position::TE.fills_slot(23));
        assert!(!Position::QB.fills_slot(23));
        assert!(!Position::K.fills_slot(23));

        assert!(Position::QB.fills_slot(0));
        assert!(!Position::RB.fills_slot(0));

        // Everyone can sit on the bench or land on IR.
        assert!(Position::K.fills_slot(SLOT_BENCH));
        assert!(Position::DEF.fills_slot(SLOT_IR));
    }

    #[test]
    fn test_position_string_conversion() {
        assert_eq!(Position::QB.to_string(), "QB");
        assert_eq!(Position::DEF.to_string(), "D/ST");
        assert_eq!(Position::FLEX.to_string(), "FLEX");
        assert_eq!("dst".parse::<Position>().unwrap(), Position::DEF);
        assert_eq!("flex".parse::<Position>().unwrap(), Position::FLEX);
        assert!("nonsense".parse::<Position>().is_err());
    }
}
