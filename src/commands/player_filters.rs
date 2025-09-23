//! Shared player filtering logic for commands

use crate::{cli::types::PlayerId, espn::types::Player};

/// Filter result for a player after applying all filtering logic
pub struct FilteredPlayer {
    pub player_id: PlayerId,
    pub original_player: Player,
}

/// Shared player filtering logic used by both player-data and projection-analysis commands
pub fn filter_and_convert_players(
    players: Vec<Player>,
    player_names: Option<Vec<String>>,
) -> impl Iterator<Item = FilteredPlayer> {
    players.into_iter().filter_map(move |player| {
        // Skip invalid player IDs and individual defensive players
        // D/ST teams (position 16) have negative IDs like -16001, which we want to keep
        // Individual defensive players (positions 8-15) are not allowed in this league
        if (player.id < 0 && player.default_position_id != 16)
            || (player.default_position_id >= 8 && player.default_position_id <= 15)
        {
            return None;
        }

        // Apply local player name filtering for multiple names
        if let Some(names) = &player_names {
            if names.len() > 1 {
                let player_name = player.full_name.as_deref().unwrap_or("");
                let matches = names
                    .iter()
                    .any(|name| player_name.to_lowercase().contains(&name.to_lowercase()));
                if !matches {
                    return None;
                }
            }
        }

        // Handle negative IDs for D/ST teams by converting to positive
        let player_id = if player.id < 0 {
            PlayerId::new((-player.id) as u64)
        } else {
            PlayerId::new(player.id as u64)
        };

        Some(FilteredPlayer {
            player_id,
            original_player: player,
        })
    })
}