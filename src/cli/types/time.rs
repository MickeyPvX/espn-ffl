//! Time-related types for ESPN Fantasy Football seasons and weeks.

use crate::error::{EspnError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

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
