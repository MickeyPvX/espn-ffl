//! Basic database query operations

use super::{models::*, schema::PlayerDatabase};
use crate::core::cache::{PlayerDataCacheKey, WeeklyStatsCacheKey, GLOBAL_CACHE};
use crate::{PlayerId, Position, Season, Week};
use anyhow::Result;
use rusqlite::{params, Row};
use std::time::{SystemTime, UNIX_EPOCH};

/// Type alias for the complex return type of cached player data queries
pub type CachedPlayerDataRow = (
    PlayerId,
    String,
    String,
    f64,
    Option<bool>,
    Option<bool>,
    Option<crate::espn::types::InjuryStatus>,
    Option<bool>,
    Option<u32>,
    Option<String>,
);

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
                 (player_id, season, week, projected_points, actual_points,
                  active, injured, injury_status, is_rostered, fantasy_team_id, fantasy_team_name,
                  created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                         COALESCE((SELECT created_at FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?), ?), ?)",
                params![
                    stats.player_id.as_u64(),
                    stats.season.as_u16(),
                    stats.week.as_u16(),
                    stats.projected_points,
                    stats.actual_points,
                    stats.active,
                    stats.injured,
                    stats.injury_status.as_ref().map(|s| s.to_string()),
                    stats.is_rostered,
                    stats.fantasy_team_id,
                    stats.fantasy_team_name,
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
                 (player_id, season, week, projected_points, actual_points,
                  active, injured, injury_status, is_rostered, fantasy_team_id, fantasy_team_name,
                  created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    stats.player_id.as_u64(),
                    stats.season.as_u16(),
                    stats.week.as_u16(),
                    stats.projected_points,
                    stats.actual_points,
                    stats.active,
                    stats.injured,
                    stats.injury_status.as_ref().map(|s| s.to_string()),
                    stats.is_rostered,
                    stats.fantasy_team_id,
                    stats.fantasy_team_name,
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
        // Create cache key
        let cache_key = WeeklyStatsCacheKey {
            player_id,
            season,
            week,
        };

        // Check cache first
        if let Some(cached_result) = GLOBAL_CACHE.weekly_stats.get(&cache_key) {
            return Ok(cached_result);
        }
        let mut stmt = self.conn.prepare(
            "SELECT player_id, season, week, projected_points, actual_points,
                    active, injured, injury_status, is_rostered, fantasy_team_id, fantasy_team_name,
                    created_at, updated_at
             FROM player_weekly_stats
             WHERE player_id = ? AND season = ? AND week = ?",
        )?;

        let result = stmt.query_row(
            params![player_id.as_u64(), season.as_u16(), week.as_u16()],
            |row| self.row_to_weekly_stats(row),
        );

        let final_result = match result {
            Ok(stats) => Some(stats),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };

        // Cache the result
        GLOBAL_CACHE
            .weekly_stats
            .put(cache_key, final_result.clone());

        Ok(final_result)
    }

    /// Get all weekly stats for a player in a season
    pub fn get_player_season_stats(
        &self,
        player_id: PlayerId,
        season: Season,
    ) -> Result<Vec<PlayerWeeklyStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT player_id, season, week, projected_points, actual_points,
                    active, injured, injury_status, is_rostered, fantasy_team_id, fantasy_team_name,
                    created_at, updated_at
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

    /// Insert or merge weekly stats, preserving existing projected/actual points but updating roster info
    pub fn merge_weekly_stats(&mut self, stats: &PlayerWeeklyStats) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Use INSERT OR REPLACE with COALESCE to merge data
        // Always update roster info but preserve existing points if available
        self.conn.execute(
            "INSERT OR REPLACE INTO player_weekly_stats
             (player_id, season, week, projected_points, actual_points,
              active, injured, injury_status, is_rostered, fantasy_team_id, fantasy_team_name,
              created_at, updated_at)
             VALUES (?, ?, ?,
                     COALESCE(?, (SELECT projected_points FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?)),
                     COALESCE(?, (SELECT actual_points FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?)),
                     COALESCE(?, (SELECT active FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?)),
                     COALESCE(?, (SELECT injured FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?)),
                     COALESCE(?, (SELECT injury_status FROM player_weekly_stats
                                  WHERE player_id = ? AND season = ? AND week = ?)),
                     ?, ?, ?,
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
                stats.active,
                stats.player_id.as_u64(),
                stats.season.as_u16(),
                stats.week.as_u16(),
                stats.injured,
                stats.player_id.as_u64(),
                stats.season.as_u16(),
                stats.week.as_u16(),
                stats.injury_status.as_ref().map(|s| s.to_string()),
                stats.player_id.as_u64(),
                stats.season.as_u16(),
                stats.week.as_u16(),
                stats.is_rostered,
                stats.fantasy_team_id,
                stats.fantasy_team_name,
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
    ) -> Result<Vec<CachedPlayerDataRow>> {
        // Create cache key
        let cache_key = PlayerDataCacheKey {
            season,
            week,
            player_names: player_names.cloned(),
            positions: positions.cloned(),
            projected,
        };

        // Check cache first
        if let Some(cached_result) = GLOBAL_CACHE.player_data.get(&cache_key) {
            return Ok(cached_result);
        }
        let mut query = String::from(
            "SELECT p.player_id, p.name, p.position,
                    CASE WHEN ? = 1 THEN pws.projected_points ELSE pws.actual_points END as points,
                    pws.active, pws.injured, pws.injury_status,
                    pws.is_rostered, pws.fantasy_team_id, pws.fantasy_team_name
             FROM players p
             JOIN player_weekly_stats pws ON p.player_id = pws.player_id
             WHERE pws.season = ? AND pws.week = ?",
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(if projected { 1 } else { 0 }),
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
                query.push(')');
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
                let injury_status_str: Option<String> = row.get(6)?;
                let injury_status = injury_status_str
                    .map(|s| match s.as_str() {
                        "Active" => Some(crate::espn::types::InjuryStatus::Active),
                        "IR" => Some(crate::espn::types::InjuryStatus::InjuryReserve),
                        "Out" => Some(crate::espn::types::InjuryStatus::Out),
                        "Doubtful" => Some(crate::espn::types::InjuryStatus::Doubtful),
                        "Questionable" => Some(crate::espn::types::InjuryStatus::Questionable),
                        "Probable" => Some(crate::espn::types::InjuryStatus::Probable),
                        "Day-to-Day" => Some(crate::espn::types::InjuryStatus::DayToDay),
                        _ => Some(crate::espn::types::InjuryStatus::Unknown),
                    })
                    .unwrap_or(None);

                Ok((
                    PlayerId::new(row.get(0)?), // player_id
                    row.get(1)?,                // name
                    row.get(2)?,                // position
                    row.get(3)?,                // points
                    row.get(4)?,                // active
                    row.get(5)?,                // injured
                    injury_status,              // injury_status
                    row.get(7)?,                // is_rostered
                    row.get(8)?,                // fantasy_team_id
                    row.get(9)?,                // fantasy_team_name
                ))
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        // Cache the results
        GLOBAL_CACHE.player_data.put(cache_key, results.clone());

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
                query.push(')');
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

    /// Get all players from the database
    pub fn get_all_players(&self) -> Result<Vec<Player>> {
        let mut stmt = self
            .conn
            .prepare("SELECT player_id, name, position, team FROM players ORDER BY name")?;

        let rows = stmt.query_map([], |row| {
            Ok(Player {
                player_id: PlayerId::new(row.get(0)?),
                name: row.get(1)?,
                position: row.get(2)?,
                team: row.get(3)?,
            })
        })?;

        let mut players = Vec::new();
        for row in rows {
            players.push(row?);
        }
        Ok(players)
    }

    /// Update roster information for ALL players based on current roster data
    /// This ensures that roster assignments are current for all players in database
    pub fn update_all_players_roster_info(
        &mut self,
        roster_data: &crate::espn::types::LeagueData,
        season: Season,
        week: Week,
    ) -> Result<usize> {
        let player_to_team = roster_data.create_player_roster_map();
        let mut updated_count = 0;

        // Get all players from database
        let all_players = self.get_all_players()?;

        for player in all_players {
            let player_id_i64 = player.player_id.as_u64() as i64;
            let negative_player_id_i64 = -(player_id_i64);

            // Check both positive and negative versions of the ID
            let roster_info = player_to_team
                .get(&player_id_i64)
                .or_else(|| player_to_team.get(&negative_player_id_i64));

            let (is_rostered, team_id, team_name) =
                if let Some((team_id, team_name, _team_abbrev)) = roster_info {
                    (Some(true), Some(*team_id), team_name.clone())
                } else {
                    (Some(false), None, None)
                };

            // Update or create a minimal weekly stats entry to store roster info
            // This ensures roster info is available even for players without performance stats
            let minimal_stats = PlayerWeeklyStats {
                player_id: player.player_id,
                season,
                week,
                projected_points: None,
                actual_points: None,
                active: None,
                injured: None,
                injury_status: None,
                is_rostered,
                fantasy_team_id: team_id,
                fantasy_team_name: team_name,
                created_at: 0, // Will be set by database
                updated_at: 0, // Will be set by database
            };

            // Use merge to preserve any existing stats while updating roster info
            self.merge_weekly_stats(&minimal_stats)?;
            updated_count += 1;
        }

        Ok(updated_count)
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
        use crate::espn::types::InjuryStatus;

        let injury_status_str: Option<String> = row.get(7)?;
        let injury_status = injury_status_str
            .map(|s| match s.as_str() {
                "Active" => Some(InjuryStatus::Active),
                "IR" => Some(InjuryStatus::InjuryReserve),
                "Out" => Some(InjuryStatus::Out),
                "Doubtful" => Some(InjuryStatus::Doubtful),
                "Questionable" => Some(InjuryStatus::Questionable),
                "Probable" => Some(InjuryStatus::Probable),
                "Day-to-Day" => Some(InjuryStatus::DayToDay),
                _ => Some(InjuryStatus::Unknown),
            })
            .unwrap_or(None);

        Ok(PlayerWeeklyStats {
            player_id: PlayerId::new(row.get(0)?),
            season: Season::new(row.get(1)?),
            week: Week::new(row.get(2)?),
            projected_points: row.get(3)?,
            actual_points: row.get(4)?,
            active: row.get(5)?,
            injured: row.get(6)?,
            injury_status,
            is_rostered: row.get(8)?,
            fantasy_team_id: row.get(9)?,
            fantasy_team_name: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    }
}
