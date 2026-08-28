//! Basic database query operations

use super::{models::*, schema::PlayerDatabase};
use crate::commands::common::CommandParams;
use crate::error::Result;
use crate::{PlayerId, Position, Season, Week};
use rusqlite::{params, Row};
use std::time::{SystemTime, UNIX_EPOCH};

/// Bind parameters for `PlayerDatabase::MERGE_WEEKLY_STATS_SQL`.
///
/// A macro rather than a function because `params!` borrows the temporaries it wraps, so the
/// resulting array cannot outlive a function call.
macro_rules! merge_stats_params {
    ($stats:expr, $now:expr) => {
        params![
            $stats.player_id.as_i64(),
            $stats.season.as_u16(),
            $stats.week.as_u16(),
            $stats.projected_points,
            $stats.actual_points,
            $stats.active,
            $stats.injured,
            $stats.injury_status.as_ref().map(|s| s.as_str()),
            $stats.is_rostered,
            $stats.fantasy_team_id,
            $stats.fantasy_team_name,
            $now,
        ]
    };
}

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
                player.player_id.as_i64(),
                player.name,
                player.position,
                player.team
            ],
        )?;
        Ok(())
    }

    /// Update players table with ESPN player data
    /// Converts ESPN player format to database format and upserts
    ///
    /// Runs as one transaction; a league-wide refresh touches thousands of rows.
    pub fn update_players_from_espn(
        &mut self,
        espn_players: &[crate::espn::types::Player],
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO players (player_id, name, position, team)
                 VALUES (?, ?, ?, ?)",
            )?;

            for player in espn_players {
                let position = (player.default_position_id >= 0)
                    .then(|| {
                        Position::from_default_position_id(player.default_position_id as u8).ok()
                    })
                    .flatten()
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "UNKNOWN".to_string());

                stmt.execute(params![
                    player.id,
                    player
                        .full_name
                        .clone()
                        .unwrap_or_else(|| format!("Player {}", player.id)),
                    position,
                    None::<String>, // ESPN API doesn't provide team in this format
                ])?;
            }
        }
        tx.commit()?;
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
                    stats.player_id.as_i64(),
                    stats.season.as_u16(),
                    stats.week.as_u16(),
                    stats.projected_points,
                    stats.actual_points,
                    stats.active,
                    stats.injured,
                    stats.injury_status.as_ref().map(|s| s.as_str()),
                    stats.is_rostered,
                    stats.fantasy_team_id,
                    stats.fantasy_team_name,
                    stats.player_id.as_i64(),
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
                    stats.player_id.as_i64(),
                    stats.season.as_u16(),
                    stats.week.as_u16(),
                    stats.projected_points,
                    stats.actual_points,
                    stats.active,
                    stats.injured,
                    stats.injury_status.as_ref().map(|s| s.as_str()),
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
        let mut stmt = self.conn.prepare(
            "SELECT player_id, season, week, projected_points, actual_points,
                    active, injured, injury_status, is_rostered, fantasy_team_id, fantasy_team_name,
                    created_at, updated_at
             FROM player_weekly_stats
             WHERE player_id = ? AND season = ? AND week = ?",
        )?;

        let result = stmt.query_row(
            params![player_id.as_i64(), season.as_u16(), week.as_u16()],
            |row| self.row_to_weekly_stats(row),
        );

        let final_result = match result {
            Ok(stats) => Some(stats),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };

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

        let rows = stmt.query_map(params![player_id.as_i64(), season.as_u16()], |row| {
            self.row_to_weekly_stats(row)
        })?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(row?);
        }
        Ok(stats)
    }

    /// SQL for an upsert that preserves existing points/status when the incoming value is NULL
    /// but always overwrites roster fields.
    ///
    /// `ON CONFLICT DO UPDATE` reads the existing row through the table name and the incoming
    /// row through `excluded`, so the merge needs no correlated subqueries — the previous form
    /// issued six of them per row.
    const MERGE_WEEKLY_STATS_SQL: &'static str = "
        INSERT INTO player_weekly_stats
            (player_id, season, week, projected_points, actual_points,
             active, injured, injury_status, is_rostered, fantasy_team_id, fantasy_team_name,
             created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
        ON CONFLICT (player_id, season, week) DO UPDATE SET
            projected_points  = COALESCE(excluded.projected_points, projected_points),
            actual_points     = COALESCE(excluded.actual_points, actual_points),
            active            = COALESCE(excluded.active, active),
            injured           = COALESCE(excluded.injured, injured),
            injury_status     = COALESCE(excluded.injury_status, injury_status),
            is_rostered       = excluded.is_rostered,
            fantasy_team_id   = excluded.fantasy_team_id,
            fantasy_team_name = excluded.fantasy_team_name,
            updated_at        = excluded.updated_at";

    /// Insert or merge weekly stats, preserving existing projected/actual points but updating roster info
    pub fn merge_weekly_stats(&mut self, stats: &PlayerWeeklyStats) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let mut stmt = self.conn.prepare_cached(Self::MERGE_WEEKLY_STATS_SQL)?;
        stmt.execute(merge_stats_params!(stats, now))?;
        Ok(())
    }

    /// Merge many rows inside a single transaction with one prepared statement.
    ///
    /// Bulk updates previously committed one implicit transaction per row, which dominated
    /// the runtime of a full-league refresh.
    pub fn merge_weekly_stats_batch<'a>(
        &mut self,
        stats: impl IntoIterator<Item = &'a PlayerWeeklyStats>,
    ) -> Result<usize> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let tx = self.conn.transaction()?;
        let mut merged = 0;
        {
            let mut stmt = tx.prepare_cached(Self::MERGE_WEEKLY_STATS_SQL)?;
            for row in stats {
                stmt.execute(merge_stats_params!(row, now))?;
                merged += 1;
            }
        }
        tx.commit()?;
        Ok(merged)
    }

    /// Get cached player data for a specific season/week combination with filters
    pub fn get_cached_player_data(
        &self,
        params: &CommandParams,
        projected: bool,
    ) -> Result<Vec<CachedPlayerDataRow>> {
        let mut query = String::from(
            "SELECT p.player_id, p.name, p.position,
                    CASE WHEN ? = 1 THEN pws.projected_points ELSE pws.actual_points END as points,
                    pws.active, pws.injured, pws.injury_status,
                    pws.is_rostered, pws.fantasy_team_id, pws.fantasy_team_name
             FROM players p
             JOIN player_weekly_stats pws ON p.player_id = pws.player_id
             WHERE pws.season = ? AND pws.week = ?",
        );

        let mut sql_params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(if projected { 1 } else { 0 }),
            Box::new(params.season.as_u16()),
            Box::new(params.week.as_u16()),
        ];

        // Add projected/actual filter
        if projected {
            query.push_str(" AND pws.projected_points IS NOT NULL");
        } else {
            query.push_str(" AND pws.actual_points IS NOT NULL");
        }

        // Add player name filter if provided
        if let Some(names) = &params.player_names {
            if !names.is_empty() {
                query.push_str(" AND (");
                for (i, name) in names.iter().enumerate() {
                    if i > 0 {
                        query.push_str(" OR ");
                    }
                    query.push_str("p.name LIKE ?");
                    sql_params.push(Box::new(format!("%{}%", name)));
                }
                query.push(')');
            }
        }

        // Add position filter if provided
        if let Some(pos_list) = &params.positions {
            if !pos_list.is_empty() {
                query.push_str(" AND p.position IN (");
                for (i, pos) in pos_list.iter().enumerate() {
                    if i > 0 {
                        query.push_str(", ");
                    }
                    query.push('?');
                    sql_params.push(Box::new(pos.to_string()));
                }
                query.push(')');
            }
        }

        query.push_str(" ORDER BY points DESC");

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(sql_params.iter().map(|p| p.as_ref())),
            |row| {
                let injury_status_str: Option<String> = row.get(6)?;
                let injury_status = injury_status_str.map(|s| {
                    s.parse::<crate::espn::types::InjuryStatus>()
                        .unwrap_or_default()
                });

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

    /// Update roster information for a week from current roster data.
    ///
    /// Returns the number of rostered players recorded.
    ///
    /// Written as one sweep plus one upsert per rostered player rather than a row per known
    /// player. The old shape wrote a row for every player in the database — thousands of
    /// them carrying nothing but `is_rostered = false` — even though a row with no points
    /// can never be returned by [`Self::get_cached_player_data`], which requires a non-null
    /// points column. Those rows were pure write amplification and table bloat.
    pub fn update_all_players_roster_info(
        &mut self,
        roster_data: &crate::espn::types::LeagueData,
        season: Season,
        week: Week,
    ) -> Result<usize> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let player_to_team = roster_data.create_player_roster_map();

        let tx = self.conn.transaction()?;

        // Clear last run's roster marks for this week in a single statement, so players
        // dropped since then stop reporting a stale team.
        tx.execute(
            "UPDATE player_weekly_stats
                SET is_rostered = 0,
                    fantasy_team_id = NULL,
                    fantasy_team_name = NULL,
                    updated_at = ?
              WHERE season = ? AND week = ?
                AND (is_rostered IS NOT 0 OR fantasy_team_id IS NOT NULL)",
            params![now, season.as_u16(), week.as_u16()],
        )?;

        // Then mark the players actually on a roster. A rostered player may have no stats
        // row yet, so this has to be an upsert rather than an update.
        let mut rostered = 0;
        {
            // The WHERE EXISTS guard skips players the players table has not seen yet,
            // which would otherwise trip the foreign key. Display-time roster status comes
            // from the live ESPN response, so nothing is lost by waiting for the next fetch.
            let mut stmt = tx.prepare_cached(
                "INSERT INTO player_weekly_stats
                    (player_id, season, week, is_rostered, fantasy_team_id, fantasy_team_name,
                     created_at, updated_at)
                 SELECT ?1, ?2, ?3, 1, ?4, ?5, ?6, ?6
                  WHERE EXISTS (SELECT 1 FROM players WHERE player_id = ?1)
                 ON CONFLICT (player_id, season, week) DO UPDATE SET
                    is_rostered       = 1,
                    fantasy_team_id   = excluded.fantasy_team_id,
                    fantasy_team_name = excluded.fantasy_team_name,
                    updated_at        = excluded.updated_at",
            )?;

            for (player_id, (team_id, team_name, _abbrev)) in &player_to_team {
                rostered += stmt.execute(params![
                    player_id,
                    season.as_u16(),
                    week.as_u16(),
                    team_id,
                    team_name,
                    now,
                ])?;
            }
        }

        tx.commit()?;
        Ok(rostered)
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
        let injury_status =
            injury_status_str.map(|s| s.parse::<InjuryStatus>().unwrap_or_default());

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
