//! Database schema and connection management

use crate::error::EspnError;
use anyhow::Result;
use dirs::cache_dir;
use rusqlite::Connection;
use std::path::PathBuf;

/// Database connection manager for player data
pub struct PlayerDatabase {
    pub(crate) conn: Connection,
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
    pub(crate) fn initialize_schema(&mut self) -> Result<()> {
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
}