//! Command implementations for ESPN Fantasy Football CLI

pub mod league_data;
pub mod player_data;
pub mod projection_analysis;

use crate::{cli::types::LeagueId, error::EspnError, Result, LEAGUE_ID_ENV_VAR};

#[cfg(test)]
mod tests;

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
