//! File system caching utilities

use dirs;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

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
}
