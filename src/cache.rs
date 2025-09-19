use dirs;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[cfg(test)]
mod tests;

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
