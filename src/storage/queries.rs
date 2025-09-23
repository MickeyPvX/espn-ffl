//! Basic database query operations

use super::{models::*, schema::PlayerDatabase};
use crate::cli::types::{PlayerId, Position, Season, Week};
use anyhow::Result;
use rusqlite::{params, Row};
use std::time::{SystemTime, UNIX_EPOCH};

impl PlayerDatabase {
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

    /// Get cached player data for a specific season/week combination with filters
    pub fn get_cached_player_data(
        &self,
        season: Season,
        week: Week,
        player_names: Option<&Vec<String>>,
        positions: Option<&Vec<Position>>,
        projected: bool,
    ) -> Result<Vec<(PlayerId, String, String, f64)>> {
        let mut query = String::from(
            "SELECT p.player_id, p.name, p.position,
                    COALESCE(pws.projected_points, pws.actual_points) as points
             FROM players p
             JOIN player_weekly_stats pws ON p.player_id = pws.player_id
             WHERE pws.season = ? AND pws.week = ?"
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(season.as_u16()),
            Box::new(week.as_u16()),
        ];

        // Add projected/actual filter
        if projected {
            query.push_str(" AND pws.projected_points IS NOT NULL");
        } else {
            query.push_str(" AND pws.actual_points IS NOT NULL");
        }

        // Add player name filter if provided
        if let Some(names) = player_names {
            if !names.is_empty() {
                query.push_str(" AND (");
                for (i, name) in names.iter().enumerate() {
                    if i > 0 {
                        query.push_str(" OR ");
                    }
                    query.push_str("p.name LIKE ?");
                    params.push(Box::new(format!("%{}%", name)));
                }
                query.push_str(")");
            }
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

        query.push_str(" ORDER BY points DESC");

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| {
                Ok((
                    PlayerId::new(row.get(0)?),
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                ))
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Check if we already have data for a specific season/week combination
    /// Returns true if any player data exists for the given filters
    pub fn has_data_for_week(
        &self,
        season: Season,
        week: Week,
        player_names: Option<&Vec<String>>,
        positions: Option<&Vec<Position>>,
        projected: Option<bool>,
    ) -> Result<bool> {
        // Build query based on filters
        let mut query = String::from(
            "SELECT COUNT(*) FROM player_weekly_stats pws
             JOIN players p ON pws.player_id = p.player_id
             WHERE pws.season = ? AND pws.week = ?",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(season.as_u16()), Box::new(week.as_u16())];

        // Add projected filter if provided
        if let Some(is_projected) = projected {
            if is_projected {
                query.push_str(" AND pws.projected_points IS NOT NULL");
            } else {
                query.push_str(" AND pws.actual_points IS NOT NULL");
            }
        }

        // Add player name filter if provided
        if let Some(names) = player_names {
            if !names.is_empty() {
                query.push_str(" AND (");
                for (i, name) in names.iter().enumerate() {
                    if i > 0 {
                        query.push_str(" OR ");
                    }
                    query.push_str("p.name LIKE ?");
                    params.push(Box::new(format!("%{}%", name)));
                }
                query.push_str(")");
            }
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
    pub(crate) fn row_to_weekly_stats(&self, row: &Row) -> rusqlite::Result<PlayerWeeklyStats> {
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