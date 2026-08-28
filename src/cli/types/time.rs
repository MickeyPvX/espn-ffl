//! Time-related types for ESPN Fantasy Football seasons and weeks.

use crate::error::{EspnError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Type-safe wrapper for Season years
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Season(pub u16);

/// Month (1-based) in which ESPN rolls its fantasy data over to the next season.
///
/// A season labelled N runs from September N into February N+1, and ESPN publishes the
/// following season's players and rankings from roughly March onward.
const SEASON_ROLLOVER_MONTH: u32 = 3;

impl Season {
    pub fn new(year: u16) -> Self {
        Self(year)
    }

    pub fn as_u16(&self) -> u16 {
        self.0
    }

    /// The NFL season currently in progress, derived from the system clock.
    ///
    /// Before March the active season is still the previous calendar year's, since the
    /// playoffs of season N run into January and February of N+1.
    pub fn current() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let (year, month) = civil_year_month_from_unix(now);

        if month >= SEASON_ROLLOVER_MONTH {
            Self(year)
        } else {
            Self(year.saturating_sub(1))
        }
    }
}

/// Convert a Unix timestamp to a (year, month) pair in UTC.
///
/// Implemented locally to avoid taking on a date/time dependency for one calculation.
/// Uses the civil-from-days algorithm (Howard Hinnant's `civil_from_days`).
fn civil_year_month_from_unix(secs: u64) -> (u16, u32) {
    let days = (secs / 86_400) as i64;

    // Shift the epoch to 0000-03-01 so leap days land at the end of the cycle.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;

    // mp counts months from March; map back to a calendar month.
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    (year as u16, month as u32)
}

impl Default for Season {
    /// Defaults to the season currently in progress rather than a pinned year, so the CLI
    /// does not need a code change each August.
    fn default() -> Self {
        Self::current()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_civil_year_month_from_unix() {
        // Known instants, verified against date(1).
        assert_eq!(civil_year_month_from_unix(0), (1970, 1)); // 1970-01-01
        assert_eq!(civil_year_month_from_unix(1_756_339_200), (2025, 8)); // 2025-08-28
        assert_eq!(civil_year_month_from_unix(1_788_309_000), (2026, 9)); // 2026-09-01 (draft day)
        assert_eq!(civil_year_month_from_unix(1_767_225_600), (2026, 1)); // 2026-01-01
        assert_eq!(civil_year_month_from_unix(1_709_164_800), (2024, 2)); // 2024-02-29, a leap day
    }

    #[test]
    fn test_season_rolls_over_in_march() {
        // January still belongs to the prior season (playoffs of season N run into N+1).
        let january_2026 = 1_767_225_600;
        let (year, month) = civil_year_month_from_unix(january_2026);
        assert_eq!((year, month), (2026, 1));
        assert!(month < SEASON_ROLLOVER_MONTH);

        // September is unambiguously the new season.
        let (year, month) = civil_year_month_from_unix(1_788_309_000);
        assert_eq!((year, month), (2026, 9));
        assert!(month >= SEASON_ROLLOVER_MONTH);
    }

    #[test]
    fn test_season_current_is_plausible() {
        // Guards against an off-by-one-era bug without pinning to a specific year.
        let season = Season::current().as_u16();
        assert!(
            (2020..=2100).contains(&season),
            "derived season {} is implausible",
            season
        );
        assert_eq!(Season::default(), Season::current());
    }
}
