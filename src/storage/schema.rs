//! Database schema and connection management

use crate::error::EspnError;
use crate::error::Result;
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

    /// Create an in-memory database for testing
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
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

    /// Schema version this build expects. Bump when adding a migration below.
    const SCHEMA_VERSION: u32 = 1;

    /// Initialize the database schema, applying any migrations the file is missing.
    ///
    /// Version is tracked in SQLite's own `user_version` pragma rather than by issuing
    /// `ALTER TABLE` statements and discarding the errors, so each migration runs exactly once
    /// and a genuine failure is no longer silently swallowed.
    ///
    /// Safe to call repeatedly: once the file is at [`Self::SCHEMA_VERSION`] it returns
    /// immediately without touching the database.
    pub fn initialize_schema(&mut self) -> Result<()> {
        let current: u32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?
            as u32;

        if current >= Self::SCHEMA_VERSION {
            return Ok(());
        }

        let tx = self.conn.transaction()?;

        if current < 1 {
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS players (
                    player_id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    position TEXT NOT NULL,
                    team TEXT
                );

                CREATE TABLE IF NOT EXISTS player_weekly_stats (
                    player_id INTEGER,
                    season INTEGER,
                    week INTEGER,
                    projected_points REAL,
                    actual_points REAL,
                    active INTEGER,
                    injured INTEGER,
                    injury_status TEXT,
                    is_rostered INTEGER,
                    fantasy_team_id INTEGER,
                    fantasy_team_name TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    PRIMARY KEY (player_id, season, week),
                    FOREIGN KEY (player_id) REFERENCES players(player_id)
                );

                CREATE INDEX IF NOT EXISTS idx_player_season_week
                    ON player_weekly_stats(season, week);

                CREATE INDEX IF NOT EXISTS idx_projection_diff
                    ON player_weekly_stats(projected_points, actual_points)
                    WHERE projected_points IS NOT NULL AND actual_points IS NOT NULL;",
            )?;

            // Databases created before user_version tracking may predate these columns.
            Self::add_column_if_missing(&tx, "player_weekly_stats", "active", "INTEGER")?;
            Self::add_column_if_missing(&tx, "player_weekly_stats", "injured", "INTEGER")?;
            Self::add_column_if_missing(&tx, "player_weekly_stats", "injury_status", "TEXT")?;
            Self::add_column_if_missing(&tx, "player_weekly_stats", "is_rostered", "INTEGER")?;
            Self::add_column_if_missing(&tx, "player_weekly_stats", "fantasy_team_id", "INTEGER")?;
            Self::add_column_if_missing(&tx, "player_weekly_stats", "fantasy_team_name", "TEXT")?;
        }

        tx.pragma_update(None, "user_version", Self::SCHEMA_VERSION)?;
        tx.commit()?;

        Ok(())
    }

    /// Add a column only when the table does not already have it.
    fn add_column_if_missing(
        conn: &Connection,
        table: &str,
        column: &str,
        column_type: &str,
    ) -> Result<()> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let existing: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<_>>()?;

        if !existing.iter().any(|name| name == column) {
            conn.execute(
                &format!(
                    "ALTER TABLE {} ADD COLUMN {} {}",
                    table, column, column_type
                ),
                [],
            )?;
        }

        Ok(())
    }
}
