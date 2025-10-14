//! League data command implementation

use crate::{
    core::league_settings_path, error::EspnError,
    espn::cache_settings::load_or_fetch_league_settings, LeagueId, Result, Season,
    LEAGUE_ID_ENV_VAR,
};

/// Resolve league ID from option or environment variable
pub fn resolve_league_id(league_id: Option<LeagueId>) -> Result<LeagueId> {
    match league_id {
        Some(id) => Ok(id),
        None => match std::env::var(LEAGUE_ID_ENV_VAR) {
            Ok(env_id) => {
                let parsed_id: u32 = env_id.parse().map_err(|_| EspnError::MissingLeagueId {
                    env_var: LEAGUE_ID_ENV_VAR.to_string(),
                })?;

                if parsed_id == 0 {
                    return Err(EspnError::MissingLeagueId {
                        env_var: LEAGUE_ID_ENV_VAR.to_string(),
                    });
                }

                Ok(LeagueId::new(parsed_id))
            }
            Err(_) => Err(EspnError::MissingLeagueId {
                env_var: LEAGUE_ID_ENV_VAR.to_string(),
            }),
        },
    }
}

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
