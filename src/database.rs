use crate::cli_types::{PlayerId, Season, Week};
use crate::error::EspnError;
use anyhow::Result;
use dirs::cache_dir;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests;

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

/// Database connection manager for player data
pub struct PlayerDatabase {
    conn: Connection,
}

impl PlayerDatabase {
    /// Create a new database connection and ensure tables exist
    pub fn new() -> Result<Self> {
        let db_path = Self::database_path()?;

        // Ensure the cache directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        let mut db = Self { conn };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Get the path to the database file
    fn database_path() -> Result<PathBuf> {
        let cache_dir = cache_dir().ok_or_else(|| EspnError::Cache {
            message: "Could not determine cache directory".to_string(),
        })?;
        Ok(cache_dir.join("espn-ffl").join("players.db"))
    }

    /// Initialize the database schema
    fn initialize_schema(&mut self) -> Result<()> {
        // Create players table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS players (
                player_id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                position TEXT NOT NULL,
                team TEXT
            )",
            [],
        )?;

        // Create player_weekly_stats table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS player_weekly_stats (
                player_id INTEGER,
                season INTEGER,
                week INTEGER,
                projected_points REAL,
                actual_points REAL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (player_id, season, week),
                FOREIGN KEY (player_id) REFERENCES players(player_id)
            )",
            [],
        )?;

        // Create indexes for performance
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_player_season_week
             ON player_weekly_stats(season, week)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_projection_diff
             ON player_weekly_stats(projected_points, actual_points)
             WHERE projected_points IS NOT NULL AND actual_points IS NOT NULL",
            [],
        )?;

        Ok(())
    }

    /// Insert or update a player's basic information
    pub fn upsert_player(&mut self, player: &Player) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO players (player_id, name, position, team)
             VALUES (?, ?, ?, ?)",
            params![
                player.player_id.as_u64(),
                player.name,
                player.position,
                player.team
            ],
        )?;
        Ok(())
    }

    /// Insert or update weekly stats for a player
    /// Only updates if force_update is true or if the data doesn't exist
    pub fn upsert_weekly_stats(
        &mut self,
        stats: &PlayerWeeklyStats,
        force_update: bool,
    ) -> Result<bool> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        if force_update {
            // Force update existing record
            let rows_affected = self.conn.execute(
                "INSERT OR REPLACE INTO player_weekly_stats
                 (player_id, season, week, projected_points, actual_points, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?,
                         COALESCE((SELECT created_at FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?), ?), ?)",
                params![
                    stats.player_id.as_u64(),
                    stats.season.as_u16(),
                    stats.week.as_u16(),
                    stats.projected_points,
                    stats.actual_points,
                    stats.player_id.as_u64(),
                    stats.season.as_u16(),
                    stats.week.as_u16(),
                    now,
                    now
                ],
            )?;
            Ok(rows_affected > 0)
        } else {
            // Only insert if doesn't exist
            let rows_affected = self.conn.execute(
                "INSERT OR IGNORE INTO player_weekly_stats
                 (player_id, season, week, projected_points, actual_points, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    stats.player_id.as_u64(),
                    stats.season.as_u16(),
                    stats.week.as_u16(),
                    stats.projected_points,
                    stats.actual_points,
                    now,
                    now
                ],
            )?;
            Ok(rows_affected > 0)
        }
    }

    /// Get weekly stats for a specific player, season, and week
    pub fn get_weekly_stats(
        &self,
        player_id: PlayerId,
        season: Season,
        week: Week,
    ) -> Result<Option<PlayerWeeklyStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT player_id, season, week, projected_points, actual_points, created_at, updated_at
             FROM player_weekly_stats
             WHERE player_id = ? AND season = ? AND week = ?",
        )?;

        let result = stmt.query_row(
            params![player_id.as_u64(), season.as_u16(), week.as_u16()],
            |row| self.row_to_weekly_stats(row),
        );

        match result {
            Ok(stats) => Ok(Some(stats)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all weekly stats for a player in a season
    pub fn get_player_season_stats(
        &self,
        player_id: PlayerId,
        season: Season,
    ) -> Result<Vec<PlayerWeeklyStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT player_id, season, week, projected_points, actual_points, created_at, updated_at
             FROM player_weekly_stats
             WHERE player_id = ? AND season = ?
             ORDER BY week",
        )?;

        let rows = stmt.query_map(params![player_id.as_u64(), season.as_u16()], |row| {
            self.row_to_weekly_stats(row)
        })?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(row?);
        }
        Ok(stats)
    }

    /// Get players with the biggest projection errors (over/under estimated)
    pub fn get_projection_analysis(
        &self,
        season: Season,
        week: Option<Week>,
        limit: Option<u32>,
    ) -> Result<Vec<ProjectionAnalysis>> {
        let mut query = String::from(
            "SELECT p.name, p.position, p.team,
                    AVG(s.projected_points - s.actual_points) as avg_error,
                    COUNT(*) as games_count
             FROM players p
             JOIN player_weekly_stats s ON p.player_id = s.player_id
             WHERE s.season = ?
               AND s.projected_points IS NOT NULL
               AND s.actual_points IS NOT NULL",
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(season.as_u16())];

        if let Some(w) = week {
            query.push_str(" AND s.week < ?");
            params.push(Box::new(w.as_u16()));
        }

        query.push_str(" GROUP BY p.player_id, p.name, p.position, p.team ORDER BY avg_error DESC");

        if let Some(l) = limit {
            query.push_str(" LIMIT ?");
            params.push(Box::new(l));
        }

        let mut stmt = self.conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(&param_refs[..], |row| {
            Ok(ProjectionAnalysis {
                name: row.get(0)?,
                position: row.get(1)?,
                team: row.get(2)?,
                avg_error: row.get(3)?,
                games_count: row.get(4)?,
            })
        })?;

        let mut analysis = Vec::new();
        for row in rows {
            analysis.push(row?);
        }
        Ok(analysis)
    }

    /// Estimate performance for a specific week based on ESPN projections and historical bias
    pub fn estimate_week_performance(
        &self,
        season: Season,
        target_week: Week,
        projected_points_data: &[(PlayerId, f64)], // ESPN projections for target week
        limit: Option<u32>,
    ) -> Result<Vec<PerformanceEstimate>> {
        let mut estimates = Vec::new();

        for (player_id, espn_projection) in projected_points_data
            .iter()
            .take(limit.map(|l| l as usize).unwrap_or(usize::MAX))
        {
            // Get player info first
            let mut player_stmt = self
                .conn
                .prepare("SELECT name, position, team FROM players WHERE player_id = ?")?;

            let player_info = player_stmt.query_row(params![player_id.as_u64()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            });

            let (name, position, team) = match player_info {
                Ok(info) => info,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // Player not found in database, skip
                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            // Get individual bias values for this player
            let mut bias_stmt = self.conn.prepare(
                "SELECT (s.projected_points - s.actual_points) as bias
                 FROM player_weekly_stats s
                 WHERE s.player_id = ?
                   AND s.season = ?
                   AND s.week < ?
                   AND s.projected_points IS NOT NULL
                   AND s.actual_points IS NOT NULL",
            )?;

            let bias_rows = bias_stmt.query_map(
                params![player_id.as_u64(), season.as_u16(), target_week.as_u16()],
                |row| row.get::<_, f64>(0),
            )?;

            let mut bias_values = Vec::new();
            for bias_result in bias_rows {
                bias_values.push(bias_result?);
            }

            let games_count = bias_values.len() as u32;
            if games_count == 0 {
                // No historical data, skip to fallback
                continue;
            }

            // Calculate average bias
            let avg_bias: f64 = bias_values.iter().sum::<f64>() / bias_values.len() as f64;

            // Calculate standard deviation manually
            let variance: f64 = if bias_values.len() > 1 {
                let sum_squared_diffs: f64 = bias_values
                    .iter()
                    .map(|&bias| (bias - avg_bias).powi(2))
                    .sum();
                sum_squared_diffs / (bias_values.len() - 1) as f64 // Sample standard deviation
            } else {
                0.0
            };
            let stddev_bias = variance.sqrt();

            // Start with ESPN's projection
            let base_projection = *espn_projection;

            // Adjust based on historical bias
            let bias_adjustment = if games_count >= 3 && avg_bias.abs() > 0.5 {
                // Apply bias correction, but gradually (don't fully correct)
                let correction_factor = (games_count as f64 / 10.0).min(0.8);
                -avg_bias * correction_factor
            } else {
                0.0
            };

            let estimated_points = (base_projection + bias_adjustment).max(0.0);

            // Calculate confidence based on sample size and consistency
            let sample_confidence: f64 = if games_count >= 5 {
                0.8
            } else if games_count >= 3 {
                0.6
            } else {
                0.3 // Low confidence with limited data
            };

            // Adjust confidence based on consistency (lower std dev = higher confidence)
            let consistency_factor = if stddev_bias > 0.0 {
                // High standard deviation (inconsistent) reduces confidence
                // We use a sigmoid-like function to map stddev to a multiplier
                let normalized_stddev = (stddev_bias / 10.0).min(2.0); // Cap at reasonable range
                1.0 / (1.0 + normalized_stddev.powi(2)) // Returns 0.2 to 1.0 range
            } else {
                1.0 // Perfect consistency
            };

            let confidence = (sample_confidence * consistency_factor).max(0.1).min(1.0);

            // Generate reasoning
            let reasoning = if games_count < 3 {
                format!(
                    "Limited data ({} games) - using ESPN projection",
                    games_count
                )
            } else if bias_adjustment.abs() > 1.0 {
                if avg_bias > 0.0 {
                    format!(
                        "ESPN typically overestimates by {:.1} pts, adjusted down {:.1} pts",
                        avg_bias,
                        bias_adjustment.abs()
                    )
                } else {
                    format!(
                        "ESPN typically underestimates by {:.1} pts, adjusted up {:.1} pts",
                        avg_bias.abs(),
                        bias_adjustment
                    )
                }
            } else {
                format!(
                    "ESPN projection {:.1} pts - minimal bias detected",
                    base_projection
                )
            };

            estimates.push(PerformanceEstimate {
                player_id: *player_id,
                name,
                position,
                team,
                estimated_points,
                confidence,
                reasoning,
            });
        }

        // Add fallback for players not found in database but in ESPN data
        for (player_id, espn_projection) in projected_points_data
            .iter()
            .take(limit.map(|l| l as usize).unwrap_or(usize::MAX))
        {
            // Check if we already processed this player
            if estimates.iter().any(|e| e.player_id == *player_id) {
                continue;
            }

            // No historical data, use ESPN projection as-is
            estimates.push(PerformanceEstimate {
                player_id: *player_id,
                name: "Unknown".to_string(),
                position: "Unknown".to_string(),
                team: None,
                estimated_points: *espn_projection,
                confidence: 0.3,
                reasoning: "No historical data - using ESPN projection".to_string(),
            });
        }

        // Sort by estimated points descending
        estimates.sort_by(|a, b| {
            b.estimated_points
                .partial_cmp(&a.estimated_points)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(estimates)
    }

    /// Insert or merge weekly stats, preserving existing projected/actual points
    pub fn merge_weekly_stats(&mut self, stats: &PlayerWeeklyStats) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Use INSERT OR REPLACE with COALESCE to merge data
        self.conn.execute(
            "INSERT OR REPLACE INTO player_weekly_stats
             (player_id, season, week, projected_points, actual_points, created_at, updated_at)
             VALUES (?, ?, ?,
                     COALESCE(?, (SELECT projected_points FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?)),
                     COALESCE(?, (SELECT actual_points FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?)),
                     COALESCE((SELECT created_at FROM player_weekly_stats
                              WHERE player_id = ? AND season = ? AND week = ?), ?), ?)",
            params![
                stats.player_id.as_u64(),
                stats.season.as_u16(),
                stats.week.as_u16(),
                stats.projected_points,
                stats.player_id.as_u64(),
                stats.season.as_u16(),
                stats.week.as_u16(),
                stats.actual_points,
                stats.player_id.as_u64(),
                stats.season.as_u16(),
                stats.week.as_u16(),
                stats.player_id.as_u64(),
                stats.season.as_u16(),
                stats.week.as_u16(),
                now,
                now
            ],
        )?;
        Ok(())
    }

    /// Check if we already have data for a specific season/week combination
    /// Returns true if any player data exists for the given filters
    pub fn has_data_for_week(
        &self,
        season: Season,
        week: Week,
        player_name: Option<&String>,
        positions: Option<&Vec<crate::cli_types::Position>>,
    ) -> Result<bool> {
        // Build query based on filters
        let mut query = String::from(
            "SELECT COUNT(*) FROM player_weekly_stats pws
             JOIN players p ON pws.player_id = p.player_id
             WHERE pws.season = ? AND pws.week = ?",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(season.as_u16()), Box::new(week.as_u16())];

        // Add player name filter if provided
        if let Some(name) = player_name {
            query.push_str(" AND p.name LIKE ?");
            params.push(Box::new(format!("%{}%", name)));
        }

        // Add position filter if provided
        if let Some(pos_list) = positions {
            if !pos_list.is_empty() {
                query.push_str(" AND p.position IN (");
                for (i, pos) in pos_list.iter().enumerate() {
                    if i > 0 {
                        query.push_str(", ");
                    }
                    query.push('?');
                    params.push(Box::new(pos.to_string()));
                }
                query.push(')');
            }
        }

        let mut stmt = self.conn.prepare(&query)?;
        let count: i64 = stmt.query_row(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    /// Clear all data from the database (useful for starting fresh)
    pub fn clear_all_data(&mut self) -> Result<()> {
        // Delete all data from both tables (weekly stats first due to foreign key)
        self.conn.execute("DELETE FROM player_weekly_stats", [])?;
        self.conn.execute("DELETE FROM players", [])?;
        Ok(())
    }

    /// Helper to convert database row to PlayerWeeklyStats
    fn row_to_weekly_stats(&self, row: &Row) -> rusqlite::Result<PlayerWeeklyStats> {
        Ok(PlayerWeeklyStats {
            player_id: PlayerId::new(row.get(0)?),
            season: Season::new(row.get(1)?),
            week: Week::new(row.get(2)?),
            projected_points: row.get(3)?,
            actual_points: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
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
    pub estimated_points: f64,
    pub confidence: f64, // 0.0 to 1.0
    pub reasoning: String,
}
