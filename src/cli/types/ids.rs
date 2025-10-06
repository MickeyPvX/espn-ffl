//! ID types for ESPN Fantasy Football.

use crate::error::{EspnError, Result};
use serde::{Deserialize, Serialize};
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
