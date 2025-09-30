//! Shared player filtering logic for commands

use crate::{
    cli::types::{InjuryStatusFilter, PlayerId, Position, RosterStatusFilter},
    espn::types::{InjuryStatus, Player, PlayerPoints},
};

/// Filter result for a player after applying all filtering logic
pub struct FilteredPlayer {
    pub player_id: PlayerId,
    pub original_player: Player,
}

/// Shared player filtering logic used by both player-data and projection-analysis commands
pub fn filter_and_convert_players(
    players: Vec<Player>,
    player_names: Option<Vec<String>>,
    position_filter: Option<Vec<Position>>,
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

        // Apply position filtering on the client side to ensure accuracy
        if let Some(positions) = &position_filter {
            let player_position = if player.default_position_id < 0 {
                None
            } else {
                Position::try_from(player.default_position_id as u8).ok()
            };

            if let Some(pos) = player_position {
                let matches = positions.iter().any(|filter_pos| {
                    // For FLEX, check if player position is eligible
                    if *filter_pos == Position::FLEX {
                        filter_pos.get_eligible_positions().contains(&pos)
                    } else {
                        *filter_pos == pos
                    }
                });
                if !matches {
                    return None;
                }
            } else {
                // Player has no valid position, exclude it
                return None;
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

/// Check if a player matches the given injury status filter
///
/// This function provides consistent injury status filtering logic across commands.
/// For `Active` and `Injured` filters, it uses server-side filtering hints when available.
pub fn matches_injury_filter(player: &PlayerPoints, filter: &InjuryStatusFilter) -> bool {
    match filter {
        InjuryStatusFilter::Active => {
            // For Active filter, prefer injury_status if available, otherwise check injured field
            matches!(player.injury_status, Some(InjuryStatus::Active))
                || (player.injury_status.is_none() && player.injured != Some(true))
        }
        InjuryStatusFilter::Injured => {
            // For Injured filter, check both injured field and non-Active injury status
            player.injured == Some(true)
                || matches!(&player.injury_status, Some(status) if *status != InjuryStatus::Active)
        }
        InjuryStatusFilter::Out => {
            matches!(player.injury_status, Some(InjuryStatus::Out))
        }
        InjuryStatusFilter::Doubtful => {
            matches!(player.injury_status, Some(InjuryStatus::Doubtful))
        }
        InjuryStatusFilter::Questionable => {
            matches!(player.injury_status, Some(InjuryStatus::Questionable))
        }
        InjuryStatusFilter::Probable => {
            matches!(player.injury_status, Some(InjuryStatus::Probable))
        }
        InjuryStatusFilter::DayToDay => {
            matches!(player.injury_status, Some(InjuryStatus::DayToDay))
        }
        InjuryStatusFilter::IR => {
            matches!(player.injury_status, Some(InjuryStatus::InjuryReserve))
        }
    }
}

/// Check if a player matches the given roster status filter
///
/// This function provides consistent roster status filtering logic across commands.
pub fn matches_roster_filter(player: &PlayerPoints, filter: &RosterStatusFilter) -> bool {
    match filter {
        RosterStatusFilter::Rostered => player.is_rostered.unwrap_or(false),
        RosterStatusFilter::FA => !player.is_rostered.unwrap_or(true),
    }
}

/// Apply injury status filter to a collection of PlayerPoints
///
/// # Examples
///
/// ```rust
/// # use espn_ffl::commands::player_filters::apply_injury_filter;
/// # use espn_ffl::cli::types::InjuryStatusFilter;
/// # use espn_ffl::espn::types::PlayerPoints;
/// let mut players = vec![/* PlayerPoints objects */];
/// apply_injury_filter(&mut players, &InjuryStatusFilter::Active);
/// ```
pub fn apply_injury_filter(players: &mut Vec<PlayerPoints>, filter: &InjuryStatusFilter) {
    players.retain(|player| matches_injury_filter(player, filter));
}

/// Apply roster status filter to a collection of PlayerPoints
///
/// # Examples
///
/// ```rust
/// # use espn_ffl::commands::player_filters::apply_roster_filter;
/// # use espn_ffl::cli::types::RosterStatusFilter;
/// # use espn_ffl::espn::types::PlayerPoints;
/// let mut players = vec![/* PlayerPoints objects */];
/// apply_roster_filter(&mut players, &RosterStatusFilter::FA);
/// ```
pub fn apply_roster_filter(players: &mut Vec<PlayerPoints>, filter: &RosterStatusFilter) {
    players.retain(|player| matches_roster_filter(player, filter));
}

/// Apply both injury and roster status filters to a collection of PlayerPoints
///
/// This is a convenience function that applies both filters when specified.
pub fn apply_status_filters(
    players: &mut Vec<PlayerPoints>,
    injury_filter: Option<&InjuryStatusFilter>,
    roster_filter: Option<&RosterStatusFilter>,
) {
    if let Some(filter) = injury_filter {
        apply_injury_filter(players, filter);
    }

    if let Some(filter) = roster_filter {
        apply_roster_filter(players, filter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::types::Week;

    fn create_test_player(
        name: &str,
        injured: Option<bool>,
        injury_status: Option<InjuryStatus>,
        is_rostered: Option<bool>,
    ) -> PlayerPoints {
        PlayerPoints {
            id: PlayerId::new(123),
            name: name.to_string(),
            position: "QB".to_string(),
            points: 15.0,
            week: Week::new(1),
            projected: false,
            active: Some(!injured.unwrap_or(false)),
            injured,
            injury_status,
            is_rostered,
            team_id: None,
            team_name: None,
        }
    }

    #[test]
    fn test_matches_injury_filter_active() {
        let active_player = create_test_player(
            "Active Player",
            Some(false),
            Some(InjuryStatus::Active),
            None,
        );
        let injured_player =
            create_test_player("Injured Player", Some(true), Some(InjuryStatus::Out), None);

        assert!(matches_injury_filter(
            &active_player,
            &InjuryStatusFilter::Active
        ));
        assert!(!matches_injury_filter(
            &injured_player,
            &InjuryStatusFilter::Active
        ));
    }

    #[test]
    fn test_matches_injury_filter_injured() {
        let active_player = create_test_player(
            "Active Player",
            Some(false),
            Some(InjuryStatus::Active),
            None,
        );
        let injured_player =
            create_test_player("Injured Player", Some(true), Some(InjuryStatus::Out), None);

        assert!(!matches_injury_filter(
            &active_player,
            &InjuryStatusFilter::Injured
        ));
        assert!(matches_injury_filter(
            &injured_player,
            &InjuryStatusFilter::Injured
        ));
    }

    #[test]
    fn test_matches_injury_filter_specific_status() {
        let questionable_player = create_test_player(
            "Questionable Player",
            Some(true),
            Some(InjuryStatus::Questionable),
            None,
        );
        let out_player =
            create_test_player("Out Player", Some(true), Some(InjuryStatus::Out), None);

        assert!(matches_injury_filter(
            &questionable_player,
            &InjuryStatusFilter::Questionable
        ));
        assert!(!matches_injury_filter(
            &questionable_player,
            &InjuryStatusFilter::Out
        ));
        assert!(matches_injury_filter(&out_player, &InjuryStatusFilter::Out));
    }

    #[test]
    fn test_matches_roster_filter() {
        let rostered_player = create_test_player("Rostered Player", None, None, Some(true));
        let fa_player = create_test_player("FA Player", None, None, Some(false));

        assert!(matches_roster_filter(
            &rostered_player,
            &RosterStatusFilter::Rostered
        ));
        assert!(!matches_roster_filter(
            &rostered_player,
            &RosterStatusFilter::FA
        ));
        assert!(!matches_roster_filter(
            &fa_player,
            &RosterStatusFilter::Rostered
        ));
        assert!(matches_roster_filter(&fa_player, &RosterStatusFilter::FA));
    }

    #[test]
    fn test_apply_status_filters() {
        let mut players = vec![
            create_test_player(
                "Active Rostered",
                Some(false),
                Some(InjuryStatus::Active),
                Some(true),
            ),
            create_test_player(
                "Active FA",
                Some(false),
                Some(InjuryStatus::Active),
                Some(false),
            ),
            create_test_player(
                "Injured Rostered",
                Some(true),
                Some(InjuryStatus::Out),
                Some(true),
            ),
            create_test_player(
                "Injured FA",
                Some(true),
                Some(InjuryStatus::Out),
                Some(false),
            ),
        ];

        // Filter for active free agents
        apply_status_filters(
            &mut players,
            Some(&InjuryStatusFilter::Active),
            Some(&RosterStatusFilter::FA),
        );

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "Active FA");
    }
}
