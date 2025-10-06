//! Data models for the storage layer

use crate::{espn::types::InjuryStatus, PlayerId, Season, Week};
use serde::{Deserialize, Serialize};

/// Player information stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub player_id: PlayerId,
    pub name: String,
    pub position: String,
    pub team: Option<String>,
}

/// Weekly statistics for a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerWeeklyStats {
    pub player_id: PlayerId,
    pub season: Season,
    pub week: Week,
    pub projected_points: Option<f64>,
    pub actual_points: Option<f64>,
    pub active: Option<bool>,
    pub injured: Option<bool>,
    pub injury_status: Option<InjuryStatus>,
    pub is_rostered: Option<bool>,
    pub fantasy_team_id: Option<u32>,
    pub fantasy_team_name: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl PlayerWeeklyStats {
    /// Create a minimal PlayerWeeklyStats for testing
    pub fn test_minimal(
        player_id: PlayerId,
        season: Season,
        week: Week,
        projected_points: Option<f64>,
        actual_points: Option<f64>,
    ) -> Self {
        Self {
            player_id,
            season,
            week,
            projected_points,
            actual_points,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(false),
            fantasy_team_id: None,
            fantasy_team_name: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// Create a PlayerWeeklyStats with all required fields and some defaults
    pub fn test_with_fields(
        player_id: PlayerId,
        season: Season,
        week: Week,
        projected_points: Option<f64>,
        actual_points: Option<f64>,
        created_at: u64,
        updated_at: u64,
    ) -> Self {
        Self {
            player_id,
            season,
            week,
            projected_points,
            actual_points,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(false),
            fantasy_team_id: None,
            fantasy_team_name: None,
            created_at,
            updated_at,
        }
    }
}

/// Analysis of projection accuracy for a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionAnalysis {
    pub name: String,
    pub position: String,
    pub team: Option<String>,
    pub avg_error: f64, // Positive = overestimated, Negative = underestimated
    pub games_count: u32,
}

/// Performance estimation for next week
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEstimate {
    pub player_id: PlayerId,
    pub name: String,
    pub position: String,
    pub team: Option<String>,
    pub espn_projection: f64,  // Original ESPN projection
    pub bias_adjustment: f64,  // +/- adjustment applied
    pub estimated_points: f64, // Final adjusted estimate
    pub confidence: f64,       // 0.0 to 1.0
    pub reasoning: String,
}
