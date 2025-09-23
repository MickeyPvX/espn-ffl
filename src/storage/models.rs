//! Data models for the storage layer

use crate::cli::types::{PlayerId, Season, Week};
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
    pub created_at: u64,
    pub updated_at: u64,
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
    pub confidence: f64, // 0.0 to 1.0
    pub reasoning: String,
}