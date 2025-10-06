//! League data command implementation

use crate::{
    core::league_settings_path, espn::cache_settings::load_or_fetch_league_settings, LeagueId,
    Result, Season,
};

use super::resolve_league_id;

/// Handle the league data command
pub async fn handle_league_data(
    league_id: Option<LeagueId>,
    refresh: bool,
    season: Season,
    verbose: bool,
) -> Result<()> {
    let league_id = resolve_league_id(league_id)?;

    if refresh {
        println!("Fetching fresh league settings from ESPN...");
    } else {
        println!("Loading league settings (cached if available)...");
    }

    // tarpaulin::skip - HTTP/file I/O call, tested via integration tests
    let settings = load_or_fetch_league_settings(league_id, refresh, season).await?;

    println!("✓ League settings loaded successfully");

    if verbose {
        let path = league_settings_path(season.as_u16(), league_id.as_u32());
        println!("League settings cached at: {}", path.display()); // tarpaulin::skip
        println!("League ID: {}, Season: {}", league_id, season); // tarpaulin::skip
        println!(
            "Scoring settings: {} items",
            settings.scoring_settings.scoring_items.len()
        ); // tarpaulin::skip
    }

    Ok(())
}
