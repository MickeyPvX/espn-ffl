//! Unified caching system for both in-memory LRU cache and persistent file storage
//!
//! This module provides a two-tier caching system:
//! - L1 Cache: In-memory LRU cache for fast access
//! - L2 Cache: File system persistence for longer-term storage
//!
//! The system automatically promotes frequently accessed items to memory cache
//! and provides fallback to disk storage for larger datasets.

use dirs;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    hash::Hash,
    io::{Read, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::cli::types::filters::{FantasyTeamFilter, InjuryStatusFilter, RosterStatusFilter};
use crate::{LeagueId, PlayerId, Position, Season, Week};

/// Path: ~/.cache/league_settings-{season}-{league_id}.json
pub fn league_settings_path(season: u16, league_id: u32) -> PathBuf {
    let base = dirs::cache_dir().unwrap_or_else(|| {
        let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.push(".cache");
        home
    });
    base.join("espn-ffl")
        .join(format!("league-settings_{}_{}.json", season, league_id))
}

/// Try to read a file into a String
pub fn try_read_to_string(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut s = String::new();

    f.read_to_string(&mut s).ok()?;

    Some(s)
}

/// Write a string to file
pub fn write_string(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut f = fs::File::create(path)?;
    f.write_all(contents.as_bytes())
}

/// Generic cache key that can be used for both memory and disk caching
pub trait CacheKey: Hash + Eq + Clone + Send + Sync {
    /// Generate a string representation for file system storage
    fn to_file_key(&self) -> String;

    /// Generate the file path for this cache entry
    fn to_file_path(&self) -> PathBuf {
        let base = dirs::cache_dir().unwrap_or_else(|| {
            let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            home.push(".cache");
            home
        });
        base.join("espn-ffl")
            .join(format!("{}.json", self.to_file_key()))
    }
}

/// Cache key for database player data queries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerDataCacheKey {
    pub season: Season,
    pub week: Week,
    pub player_names: Option<Vec<String>>,
    pub positions: Option<Vec<Position>>,
    pub projected: bool,
    pub injury_status: Option<InjuryStatusFilter>,
    pub roster_status: Option<RosterStatusFilter>,
    pub fantasy_team_filter: Option<FantasyTeamFilter>,
}

impl CacheKey for PlayerDataCacheKey {
    fn to_file_key(&self) -> String {
        let names_hash = self
            .player_names
            .as_ref()
            .map(|names| format!("names_{}", names.join("_")))
            .unwrap_or_else(|| "all_names".to_string());

        let positions_hash = self
            .positions
            .as_ref()
            .map(|positions| {
                format!(
                    "pos_{}",
                    positions
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join("_")
                )
            })
            .unwrap_or_else(|| "all_pos".to_string());

        let injury_hash = self
            .injury_status
            .as_ref()
            .map(|status| format!("inj_{}", status.to_string().to_lowercase()))
            .unwrap_or_else(|| "all_inj".to_string());

        let roster_hash = self
            .roster_status
            .as_ref()
            .map(|status| format!("ros_{}", status.to_string().to_lowercase()))
            .unwrap_or_else(|| "all_ros".to_string());

        let team_hash = self
            .fantasy_team_filter
            .as_ref()
            .map(|filter| match filter {
                FantasyTeamFilter::Id(id) => format!("team_id_{}", id),
                FantasyTeamFilter::Name(name) => {
                    format!("team_name_{}", name.to_lowercase().replace(' ', "_"))
                }
            })
            .unwrap_or_else(|| "all_teams".to_string());

        format!(
            "player_data_s{}_w{}_{}_{}_{}_{}_{}_{}",
            self.season.as_u16(),
            self.week.as_u16(),
            names_hash,
            positions_hash,
            injury_hash,
            roster_hash,
            team_hash,
            if self.projected { "proj" } else { "actual" }
        )
    }
}

/// Cache key for weekly stats queries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WeeklyStatsCacheKey {
    pub player_id: PlayerId,
    pub season: Season,
    pub week: Week,
}

impl CacheKey for WeeklyStatsCacheKey {
    fn to_file_key(&self) -> String {
        format!(
            "weekly_stats_p{}_s{}_w{}",
            self.player_id.as_i64(),
            self.season.as_u16(),
            self.week.as_u16()
        )
    }
}

/// Cache key for HTTP league settings
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LeagueSettingsCacheKey {
    pub league_id: LeagueId,
    pub season: Season,
}

impl CacheKey for LeagueSettingsCacheKey {
    fn to_file_key(&self) -> String {
        format!(
            "league_settings_l{}_s{}",
            self.league_id.as_u32(),
            self.season.as_u16()
        )
    }
}

/// Cache key for HTTP player data requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HttpPlayerDataCacheKey {
    pub league_id: LeagueId,
    pub season: Season,
    pub week: Week,
    pub player_names: Option<Vec<String>>,
    pub positions: Option<Vec<Position>>,
    pub projected: bool,
}

impl CacheKey for HttpPlayerDataCacheKey {
    fn to_file_key(&self) -> String {
        let names_hash = self
            .player_names
            .as_ref()
            .map(|names| format!("names_{}", names.join("_")))
            .unwrap_or_else(|| "all_names".to_string());

        let positions_hash = self
            .positions
            .as_ref()
            .map(|positions| {
                format!(
                    "pos_{}",
                    positions
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join("_")
                )
            })
            .unwrap_or_else(|| "all_pos".to_string());

        format!(
            "http_player_data_l{}_s{}_w{}_{}_{}_{}",
            self.league_id.as_u32(),
            self.season.as_u16(),
            self.week.as_u16(),
            names_hash,
            positions_hash,
            if self.projected { "proj" } else { "actual" }
        )
    }
}

/// Cache key for HTTP roster data
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RosterDataCacheKey {
    pub league_id: LeagueId,
    pub season: Season,
    pub week: Option<Week>,
}

impl CacheKey for RosterDataCacheKey {
    fn to_file_key(&self) -> String {
        let week_str = self
            .week
            .map(|w| format!("w{}", w.as_u16()))
            .unwrap_or_else(|| "current".to_string());

        format!(
            "roster_data_l{}_s{}_{}",
            self.league_id.as_u32(),
            self.season.as_u16(),
            week_str
        )
    }
}

/// Unified cache that combines LRU memory cache with file system persistence
pub struct UnifiedCache<K, V>
where
    K: CacheKey,
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    memory_cache: Arc<Mutex<LruCache<K, V>>>,
    memory_capacity: usize,
}

impl<K, V> UnifiedCache<K, V>
where
    K: CacheKey,
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new unified cache with specified memory capacity
    pub fn new(memory_capacity: usize) -> Self {
        Self {
            memory_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(memory_capacity).unwrap(),
            ))),
            memory_capacity,
        }
    }

    /// Get an item from cache (checks memory first, then disk)
    pub fn get(&self, key: &K) -> Option<V> {
        // First check memory cache
        if let Some(value) = self.memory_cache.lock().unwrap().get(key) {
            return Some(value.clone());
        }

        // Fall back to disk cache
        if let Some(value) = self.get_from_disk(key) {
            // Promote to memory cache
            self.memory_cache
                .lock()
                .unwrap()
                .put(key.clone(), value.clone());
            return Some(value);
        }

        None
    }

    /// Put an item into cache (stores in both memory and disk)
    pub fn put(&self, key: K, value: V) {
        // Store in memory cache
        self.memory_cache
            .lock()
            .unwrap()
            .put(key.clone(), value.clone());

        // Store in disk cache for persistence
        let _ = self.put_to_disk(&key, &value);
    }

    /// Get item from disk cache only
    fn get_from_disk(&self, key: &K) -> Option<V> {
        let path = key.to_file_path();
        let content = try_read_to_string(&path)?;
        serde_json::from_str(&content).ok()
    }

    /// Put item to disk cache only
    fn put_to_disk(&self, key: &K, value: &V) -> std::io::Result<()> {
        let path = key.to_file_path();
        let content = serde_json::to_string_pretty(value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_string(&path, &content)
    }

    /// Clear memory cache only (keeps disk cache)
    pub fn clear_memory(&self) {
        self.memory_cache.lock().unwrap().clear();
    }

    /// Clear both memory and disk cache
    pub fn clear_all(&self) {
        self.clear_memory();
        // Note: We don't clear disk cache by default as it's more expensive
        // Add a method to clear disk cache if needed
    }

    /// Clear disk cache for a specific key (used when underlying data changes)
    pub fn invalidate_disk_cache(&self, key: &K) -> std::io::Result<()> {
        let path = key.to_file_path();
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Get memory cache statistics
    pub fn memory_stats(&self) -> (usize, usize) {
        let cache = self.memory_cache.lock().unwrap();
        (cache.len(), self.memory_capacity)
    }
}

/// Global cache manager for the entire application
pub struct CacheManager {
    pub player_data:
        UnifiedCache<PlayerDataCacheKey, Vec<crate::storage::queries::CachedPlayerDataRow>>,
    pub weekly_stats:
        UnifiedCache<WeeklyStatsCacheKey, Option<crate::storage::models::PlayerWeeklyStats>>,
    pub league_settings: UnifiedCache<LeagueSettingsCacheKey, Value>,
    pub http_player_data: UnifiedCache<HttpPlayerDataCacheKey, Value>,
    pub roster_data: UnifiedCache<RosterDataCacheKey, Value>,
}

impl CacheManager {
    /// Create a new cache manager with reasonable defaults
    pub fn new() -> Self {
        Self {
            player_data: UnifiedCache::new(100), // Cache up to 100 player data queries
            weekly_stats: UnifiedCache::new(500), // Cache up to 500 individual player weekly stats
            league_settings: UnifiedCache::new(50), // Cache up to 50 league settings
            http_player_data: UnifiedCache::new(100), // Cache up to 100 HTTP player data responses
            roster_data: UnifiedCache::new(50),  // Cache up to 50 roster data responses
        }
    }

    /// Clear all memory caches
    pub fn clear_all_memory(&self) {
        self.player_data.clear_memory();
        self.weekly_stats.clear_memory();
        self.league_settings.clear_memory();
        self.http_player_data.clear_memory();
        self.roster_data.clear_memory();
    }

    /// Get memory usage statistics for all caches
    pub fn memory_stats(&self) -> HashMap<String, (usize, usize)> {
        let mut stats = HashMap::new();
        stats.insert("player_data".to_string(), self.player_data.memory_stats());
        stats.insert("weekly_stats".to_string(), self.weekly_stats.memory_stats());
        stats.insert(
            "league_settings".to_string(),
            self.league_settings.memory_stats(),
        );
        stats.insert(
            "http_player_data".to_string(),
            self.http_player_data.memory_stats(),
        );
        stats.insert("roster_data".to_string(), self.roster_data.memory_stats());
        stats
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global cache instance using lazy_static pattern
use std::sync::LazyLock;

/// Global cache manager instance for use across the application
pub static GLOBAL_CACHE: LazyLock<CacheManager> = LazyLock::new(CacheManager::new);

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_league_settings_path() {
        let path = league_settings_path(2023, 12345);
        let path_str = path.to_string_lossy();

        assert!(path_str.contains("espn-ffl"));
        assert!(path_str.contains("league-settings_2023_12345.json"));
    }

    #[test]
    fn test_try_read_to_string_existing_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        fs::write(&file_path, "hello world").unwrap();

        let content = try_read_to_string(&file_path);
        assert_eq!(content, Some("hello world".to_string()));
    }

    #[test]
    fn test_try_read_to_string_nonexistent_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("nonexistent.txt");

        let content = try_read_to_string(&file_path);
        assert_eq!(content, None);
    }

    #[test]
    fn test_write_string() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("output.txt");

        write_string(&file_path, "test content").unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_write_string_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("subdir").join("output.txt");

        write_string(&file_path, "test content").unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_cache_key_generation() {
        let key = PlayerDataCacheKey {
            season: Season::new(2025),
            week: Week::new(1),
            player_names: Some(vec!["Josh Allen".to_string()]),
            positions: None,
            projected: false,
            injury_status: None,
            roster_status: None,
            fantasy_team_filter: None,
        };

        let file_key = key.to_file_key();
        assert!(file_key.contains("player_data"));
        assert!(file_key.contains("s2025"));
        assert!(file_key.contains("w1"));
        assert!(file_key.contains("Josh Allen"));
        assert!(file_key.contains("actual"));
    }

    #[test]
    fn test_unified_cache_memory_operations() {
        let cache: UnifiedCache<WeeklyStatsCacheKey, Option<String>> = UnifiedCache::new(2);

        // Use unique test keys to avoid cache conflicts with real data
        let key1 = WeeklyStatsCacheKey {
            player_id: PlayerId::new(999991),
            season: Season::new(2099),
            week: Week::new(99),
        };

        let key2 = WeeklyStatsCacheKey {
            player_id: PlayerId::new(999992),
            season: Season::new(2099),
            week: Week::new(99),
        };

        // Clear memory to start fresh
        cache.clear_memory();

        // Test cache put and hit
        cache.put(key1.clone(), Some("test_data".to_string()));
        assert_eq!(cache.get(&key1), Some(Some("test_data".to_string())));

        // Test LRU eviction
        cache.put(key2.clone(), Some("test_data2".to_string()));
        let key3 = WeeklyStatsCacheKey {
            player_id: PlayerId::new(999993),
            season: Season::new(2099),
            week: Week::new(99),
        };
        cache.put(key3.clone(), Some("test_data3".to_string()));

        // Memory cache should be at capacity
        let stats = cache.memory_stats();
        assert_eq!(stats.0, 2); // Only 2 items in memory cache
        assert_eq!(stats.1, 2); // Capacity is 2
    }

    #[test]
    fn test_cache_manager_creation() {
        let manager = CacheManager::new();
        let stats = manager.memory_stats();

        assert!(stats.contains_key("player_data"));
        assert!(stats.contains_key("weekly_stats"));
        assert!(stats.contains_key("league_settings"));
        assert!(stats.contains_key("http_player_data"));
        assert!(stats.contains_key("roster_data"));

        // All caches should start empty
        for (_, (used, _capacity)) in stats {
            assert_eq!(used, 0);
        }
    }
}
