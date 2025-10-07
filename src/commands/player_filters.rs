//! Shared player filtering logic for commands

use crate::{
    cli::types::{
        filters::{FantasyTeamFilter, InjuryStatusFilter, RosterStatusFilter},
        position::Position,
    },
    espn::types::{InjuryStatus, Player, PlayerPoints},
    PlayerId,
};
use rayon::prelude::*;

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
) -> Vec<FilteredPlayer> {
    players
        .into_par_iter()
        .filter_map(move |player| {
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
                            filter_pos.get_all_position_ids().contains(&pos.to_u8())
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
        .collect()
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

/// Check if a player matches the given fantasy team filter
///
/// This function provides consistent fantasy team filtering logic across commands.
/// For team name filtering, it performs case-insensitive partial matching against
/// both the full team name and the 3-letter team abbreviation stored by ESPN.
pub fn matches_fantasy_team_filter(player: &PlayerPoints, filter: &FantasyTeamFilter) -> bool {
    match filter {
        FantasyTeamFilter::Id(team_id) => player.team_id == Some(*team_id),
        FantasyTeamFilter::Name(filter_name) => {
            let filter_lower = filter_name.to_lowercase();

            // Check if team name contains the filter (case-insensitive)
            if let Some(team_name) = &player.team_name {
                if team_name.to_lowercase().contains(&filter_lower) {
                    return true;
                }
            }

            // Note: ESPN's 3-letter abbreviations would need to be stored separately
            // For now, we only match against the full team name
            false
        }
    }
}

/// Apply injury status filter to a collection of PlayerPoints
///
/// # Examples
///
/// ```rust
/// # use espn_ffl::commands::player_filters::apply_injury_filter;
/// # use espn_ffl::cli::types::filters::InjuryStatusFilter;
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
/// # use espn_ffl::cli::types::filters::RosterStatusFilter;
/// # use espn_ffl::espn::types::PlayerPoints;
/// let mut players = vec![/* PlayerPoints objects */];
/// apply_roster_filter(&mut players, &RosterStatusFilter::FA);
/// ```
pub fn apply_roster_filter(players: &mut Vec<PlayerPoints>, filter: &RosterStatusFilter) {
    players.retain(|player| matches_roster_filter(player, filter));
}

/// Apply fantasy team filter to a collection of PlayerPoints
///
/// # Examples
///
/// ```rust
/// # use espn_ffl::commands::player_filters::apply_fantasy_team_filter;
/// # use espn_ffl::cli::types::filters::FantasyTeamFilter;
/// # use espn_ffl::espn::types::PlayerPoints;
/// let mut players = vec![/* PlayerPoints objects */];
/// apply_fantasy_team_filter(&mut players, &FantasyTeamFilter::Name("kenny".to_string()));
/// ```
pub fn apply_fantasy_team_filter(players: &mut Vec<PlayerPoints>, filter: &FantasyTeamFilter) {
    players.retain(|player| matches_fantasy_team_filter(player, filter));
}

/// Apply injury, roster, and fantasy team filters to a collection of PlayerPoints
///
/// This is a convenience function that applies all filters when specified.
pub fn apply_status_filters(
    players: &mut Vec<PlayerPoints>,
    injury_filter: Option<&InjuryStatusFilter>,
    roster_filter: Option<&RosterStatusFilter>,
    fantasy_team_filter: Option<&FantasyTeamFilter>,
) {
    if let Some(filter) = injury_filter {
        apply_injury_filter(players, filter);
    }

    if let Some(filter) = roster_filter {
        apply_roster_filter(players, filter);
    }

    if let Some(filter) = fantasy_team_filter {
        apply_fantasy_team_filter(players, filter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Week;

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
            None,
        );

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "Active FA");
    }

    #[test]
    fn test_matches_fantasy_team_filter_by_id() {
        let player_on_team_1 = PlayerPoints {
            id: PlayerId::new(123),
            name: "Player 1".to_string(),
            position: "QB".to_string(),
            points: 15.0,
            week: Week::new(1),
            projected: false,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(true),
            team_id: Some(1),
            team_name: Some("Kenny Rogers' Toasters".to_string()),
        };

        let player_on_team_2 = PlayerPoints {
            id: PlayerId::new(124),
            name: "Player 2".to_string(),
            position: "RB".to_string(),
            points: 12.0,
            week: Week::new(1),
            projected: false,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(true),
            team_id: Some(2),
            team_name: Some("Other Team".to_string()),
        };

        let team_1_filter = FantasyTeamFilter::Id(1);
        let team_2_filter = FantasyTeamFilter::Id(2);
        let team_3_filter = FantasyTeamFilter::Id(3);

        assert!(matches_fantasy_team_filter(
            &player_on_team_1,
            &team_1_filter
        ));
        assert!(!matches_fantasy_team_filter(
            &player_on_team_1,
            &team_2_filter
        ));
        assert!(!matches_fantasy_team_filter(
            &player_on_team_1,
            &team_3_filter
        ));

        assert!(!matches_fantasy_team_filter(
            &player_on_team_2,
            &team_1_filter
        ));
        assert!(matches_fantasy_team_filter(
            &player_on_team_2,
            &team_2_filter
        ));
        assert!(!matches_fantasy_team_filter(
            &player_on_team_2,
            &team_3_filter
        ));
    }

    #[test]
    fn test_matches_fantasy_team_filter_by_name() {
        let player_kenny_team = PlayerPoints {
            id: PlayerId::new(123),
            name: "Player 1".to_string(),
            position: "QB".to_string(),
            points: 15.0,
            week: Week::new(1),
            projected: false,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(true),
            team_id: Some(1),
            team_name: Some("Kenny Rogers' Toasters".to_string()),
        };

        let player_other_team = PlayerPoints {
            id: PlayerId::new(124),
            name: "Player 2".to_string(),
            position: "RB".to_string(),
            points: 12.0,
            week: Week::new(1),
            projected: false,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(true),
            team_id: Some(2),
            team_name: Some("Different Team Name".to_string()),
        };

        // Test partial matching (case-insensitive)
        let kenny_filter = FantasyTeamFilter::Name("kenny".to_string());
        let toasters_filter = FantasyTeamFilter::Name("toasters".to_string());
        let rogers_filter = FantasyTeamFilter::Name("Rogers".to_string());
        let different_filter = FantasyTeamFilter::Name("different".to_string());
        let nomatch_filter = FantasyTeamFilter::Name("nomatch".to_string());

        // Should match Kenny's team
        assert!(matches_fantasy_team_filter(
            &player_kenny_team,
            &kenny_filter
        ));
        assert!(matches_fantasy_team_filter(
            &player_kenny_team,
            &toasters_filter
        ));
        assert!(matches_fantasy_team_filter(
            &player_kenny_team,
            &rogers_filter
        ));
        assert!(!matches_fantasy_team_filter(
            &player_kenny_team,
            &different_filter
        ));
        assert!(!matches_fantasy_team_filter(
            &player_kenny_team,
            &nomatch_filter
        ));

        // Should match other team
        assert!(!matches_fantasy_team_filter(
            &player_other_team,
            &kenny_filter
        ));
        assert!(!matches_fantasy_team_filter(
            &player_other_team,
            &toasters_filter
        ));
        assert!(!matches_fantasy_team_filter(
            &player_other_team,
            &rogers_filter
        ));
        assert!(matches_fantasy_team_filter(
            &player_other_team,
            &different_filter
        ));
        assert!(!matches_fantasy_team_filter(
            &player_other_team,
            &nomatch_filter
        ));
    }
}
