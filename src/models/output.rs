//! Output models used for printing and JSON serialization.

use serde::Serialize;

/// Points for a particular scoring week.
#[derive(Debug, Serialize)]
pub struct WeekPoints {
    /// Week number (1..=18).
    pub week: u16,
    /// Applied fantasy points for the week (statSourceId == 0).
    pub points: f64,
}

/// Aggregated player + weekly points payload.
///
/// This structure is designed for easy JSON serialization and DB insertion.
#[derive(Debug, Serialize)]
pub struct PlayerWeekPoints {
    /// ESPN player ID.
    pub id: u64,
    /// Player full name.
    pub name: String,
    /// Points for the selected weeks.
    pub weeks: Vec<WeekPoints>,
}
