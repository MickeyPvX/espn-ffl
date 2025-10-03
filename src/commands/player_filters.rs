//! Shared player filtering logic for commands

use crate::{
    cli::types::{InjuryStatusFilter, PlayerId, Position, RosterStatusFilter, TeamFilter},
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

/// Check if a player matches the given team filter
///
/// This function provides consistent team filtering logic across commands.
/// Uses flexible matching for team names - searches for partial matches case-insensitively.
/// Examples: "kenny" matches "Kenny Rogers' Toasters", "mike" matches "Mike's Misfits"
pub fn matches_team_filter(player: &PlayerPoints, filter: &TeamFilter) -> bool {
    match filter {
        TeamFilter::TeamId(id) => player.team_id == Some(*id),
        TeamFilter::TeamName(search_term) => {
            if let Some(player_team_name) = &player.team_name {
                // Flexible matching: check if search term appears anywhere in team name (case-insensitive)
                player_team_name
                    .to_lowercase()
                    .contains(&search_term.to_lowercase())
            } else {
                false
            }
        }
        TeamFilter::TeamIds(ids) => {
            if let Some(player_team_id) = player.team_id {
                ids.contains(&player_team_id)
            } else {
                false
            }
        }
        TeamFilter::TeamNames(search_terms) => {
            if let Some(player_team_name) = &player.team_name {
                let player_name_lower = player_team_name.to_lowercase();
                // Match if ANY of the search terms appears in the team name
                search_terms
                    .iter()
                    .any(|term| player_name_lower.contains(&term.to_lowercase()))
            } else {
                false
            }
        }
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

/// Apply team filter to a collection of PlayerPoints
///
/// # Examples
///
/// ```rust
/// # use espn_ffl::commands::player_filters::apply_team_filter;
/// # use espn_ffl::cli::types::TeamFilter;
/// # use espn_ffl::espn::types::PlayerPoints;
/// let mut players = vec![/* PlayerPoints objects */];
/// apply_team_filter(&mut players, &TeamFilter::TeamName("kenny".to_string()));
/// ```
pub fn apply_team_filter(players: &mut Vec<PlayerPoints>, filter: &TeamFilter) {
    players.retain(|player| matches_team_filter(player, filter));
}

/// Apply injury, roster, and team status filters to a collection of PlayerPoints
///
/// This is a convenience function that applies all filters when specified.
pub fn apply_status_filters(
    players: &mut Vec<PlayerPoints>,
    injury_filter: Option<&InjuryStatusFilter>,
    roster_filter: Option<&RosterStatusFilter>,
    team_filter: Option<&TeamFilter>,
) {
    if let Some(filter) = injury_filter {
        apply_injury_filter(players, filter);
    }

    if let Some(filter) = roster_filter {
        apply_roster_filter(players, filter);
    }

    if let Some(filter) = team_filter {
        apply_team_filter(players, filter);
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
            None,
        );

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "Active FA");
    }

    #[test]
    fn test_filter_and_convert_players_basic() {
        use crate::espn::types::Player;

        let players = vec![
            Player {
                id: 12345,
                full_name: Some("Tom Brady".to_string()),
                default_position_id: 0, // QB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 23456,
                full_name: Some("Ezekiel Elliott".to_string()),
                default_position_id: 2, // RB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
        ];

        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players, None, None).collect();
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].player_id.as_u64(), 12345);
        assert_eq!(filtered[1].player_id.as_u64(), 23456);
    }

    #[test]
    fn test_filter_and_convert_players_with_name_filter() {
        use crate::espn::types::Player;

        let players = vec![
            Player {
                id: 12345,
                full_name: Some("Tom Brady".to_string()),
                default_position_id: 0, // QB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 23456,
                full_name: Some("Aaron Rodgers".to_string()),
                default_position_id: 0, // QB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
        ];

        // Test filtering with multiple names (should include both Brady and Rodgers)
        let player_names = Some(vec!["Brady".to_string(), "Rodgers".to_string()]);
        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players, player_names, None).collect();
        assert_eq!(filtered.len(), 2);

        // Test filtering with single name that doesn't match
        let players = vec![Player {
            id: 12345,
            full_name: Some("Tom Brady".to_string()),
            default_position_id: 0,
            stats: vec![],
            active: Some(true),
            injured: Some(false),
            injury_status: Some(InjuryStatus::Active),
        }];
        let player_names = Some(vec!["NonExistent".to_string(), "Player".to_string()]);
        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players, player_names, None).collect();
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_and_convert_players_with_position_filter() {
        use crate::espn::types::Player;

        let players = vec![
            Player {
                id: 12345,
                full_name: Some("Tom Brady".to_string()),
                default_position_id: 0, // QB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 23456,
                full_name: Some("Ezekiel Elliott".to_string()),
                default_position_id: 2, // RB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 34567,
                full_name: Some("Travis Kelce".to_string()),
                default_position_id: 4, // TE
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
        ];

        // Filter for QB only
        let position_filter = Some(vec![Position::QB]);
        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players.clone(), None, position_filter).collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].original_player.default_position_id, 0);

        // Filter for FLEX positions (should include RB and TE but not QB)
        let position_filter = Some(vec![Position::FLEX]);
        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players, None, position_filter).collect();
        assert_eq!(filtered.len(), 2); // RB and TE are FLEX eligible
    }

    #[test]
    fn test_filter_and_convert_players_defensive_filtering() {
        use crate::espn::types::Player;

        let players = vec![
            Player {
                id: -16001, // D/ST team (negative ID, position 16)
                full_name: Some("New England Patriots".to_string()),
                default_position_id: 16, // D/ST
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 12345, // Valid player
                full_name: Some("Tom Brady".to_string()),
                default_position_id: 0, // QB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: -12345, // Individual defensive player (negative ID, defensive position)
                full_name: Some("Defensive Player".to_string()),
                default_position_id: 10, // Individual defensive position
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
        ];

        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players, None, None).collect();

        // Should include D/ST team and regular player, but exclude individual defensive player
        assert_eq!(filtered.len(), 2);

        // Check that D/ST ID was converted to positive
        let dst_player = filtered
            .iter()
            .find(|p| p.original_player.default_position_id == 16)
            .unwrap();
        assert_eq!(dst_player.player_id.as_u64(), 16001); // Converted to positive
    }

    #[test]
    fn test_filter_and_convert_players_invalid_positions() {
        use crate::espn::types::Player;

        let players = vec![
            Player {
                id: 12345,
                full_name: Some("Valid Player".to_string()),
                default_position_id: 0, // Valid QB position
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 23456,
                full_name: Some("Invalid Position Player".to_string()),
                default_position_id: 99, // Invalid position
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
        ];

        // Filter with position filter should exclude invalid position player
        let position_filter = Some(vec![Position::QB]);
        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players, None, position_filter).collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].original_player.default_position_id, 0);
    }

    #[test]
    fn test_matches_injury_filter_edge_cases() {
        // Test player with no injury status and no injured field
        let unknown_player = create_test_player("Unknown Status", None, None, None);

        // Should match Active filter (no injury status = assumed active)
        assert!(matches_injury_filter(
            &unknown_player,
            &InjuryStatusFilter::Active
        ));
        // Should not match Injured filter
        assert!(!matches_injury_filter(
            &unknown_player,
            &InjuryStatusFilter::Injured
        ));

        // Test player with injured=false but specific injury status
        let contradictory_player = create_test_player(
            "Contradictory Status",
            Some(false),
            Some(InjuryStatus::Questionable),
            None,
        );

        // Should match Injured filter (injury_status takes precedence)
        assert!(matches_injury_filter(
            &contradictory_player,
            &InjuryStatusFilter::Injured
        ));
        assert!(matches_injury_filter(
            &contradictory_player,
            &InjuryStatusFilter::Questionable
        ));
    }

    #[test]
    fn test_matches_injury_filter_all_statuses() {
        // Test all specific injury statuses
        let statuses = [
            (InjuryStatus::Out, InjuryStatusFilter::Out),
            (InjuryStatus::Doubtful, InjuryStatusFilter::Doubtful),
            (InjuryStatus::Questionable, InjuryStatusFilter::Questionable),
            (InjuryStatus::Probable, InjuryStatusFilter::Probable),
            (InjuryStatus::DayToDay, InjuryStatusFilter::DayToDay),
            (InjuryStatus::InjuryReserve, InjuryStatusFilter::IR),
        ];

        for (injury_status, filter) in statuses {
            let player =
                create_test_player("Test Player", Some(true), Some(injury_status.clone()), None);
            assert!(matches_injury_filter(&player, &filter));

            // Should not match Active filter
            assert!(!matches_injury_filter(&player, &InjuryStatusFilter::Active));

            // Should match Injured filter (except Active)
            if injury_status != InjuryStatus::Active {
                assert!(matches_injury_filter(&player, &InjuryStatusFilter::Injured));
            }
        }
    }

    #[test]
    fn test_matches_roster_filter_edge_cases() {
        // Test player with unknown roster status (None)
        let unknown_roster_player = create_test_player("Unknown Roster", None, None, None);

        // Should not match Rostered (None defaults to false)
        assert!(!matches_roster_filter(
            &unknown_roster_player,
            &RosterStatusFilter::Rostered
        ));
        // Should not match FA (None defaults to true, then negated to false)
        assert!(!matches_roster_filter(
            &unknown_roster_player,
            &RosterStatusFilter::FA
        ));
    }

    #[test]
    fn test_apply_injury_filter_empty_list() {
        let mut players: Vec<PlayerPoints> = vec![];
        apply_injury_filter(&mut players, &InjuryStatusFilter::Active);
        assert_eq!(players.len(), 0);
    }

    #[test]
    fn test_apply_roster_filter_empty_list() {
        let mut players: Vec<PlayerPoints> = vec![];
        apply_roster_filter(&mut players, &RosterStatusFilter::Rostered);
        assert_eq!(players.len(), 0);
    }

    #[test]
    fn test_apply_status_filters_no_filters() {
        let mut players = vec![
            create_test_player(
                "Player 1",
                Some(false),
                Some(InjuryStatus::Active),
                Some(true),
            ),
            create_test_player("Player 2", Some(true), Some(InjuryStatus::Out), Some(false)),
        ];

        // Apply no filters
        apply_status_filters(&mut players, None, None, None);

        // Should retain all players
        assert_eq!(players.len(), 2);
    }

    #[test]
    fn test_apply_status_filters_injury_only() {
        let mut players = vec![
            create_test_player(
                "Active Player",
                Some(false),
                Some(InjuryStatus::Active),
                Some(true),
            ),
            create_test_player(
                "Injured Player",
                Some(true),
                Some(InjuryStatus::Out),
                Some(true),
            ),
        ];

        // Apply only injury filter
        apply_status_filters(&mut players, Some(&InjuryStatusFilter::Active), None, None);

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "Active Player");
    }

    #[test]
    fn test_apply_status_filters_roster_only() {
        let mut players = vec![
            create_test_player(
                "Rostered Player",
                Some(false),
                Some(InjuryStatus::Active),
                Some(true),
            ),
            create_test_player(
                "FA Player",
                Some(false),
                Some(InjuryStatus::Active),
                Some(false),
            ),
        ];

        // Apply only roster filter
        apply_status_filters(&mut players, None, Some(&RosterStatusFilter::FA), None);

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "FA Player");
    }

    #[test]
    fn test_filtered_player_structure() {
        use crate::espn::types::Player;

        let player = Player {
            id: 12345,
            full_name: Some("Test Player".to_string()),
            default_position_id: 0,
            stats: vec![],
            active: Some(true),
            injured: Some(false),
            injury_status: Some(InjuryStatus::Active),
        };

        let players = vec![player.clone()];
        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players, None, None).collect();

        assert_eq!(filtered.len(), 1);
        let filtered_player = &filtered[0];

        // Check structure integrity
        assert_eq!(filtered_player.player_id.as_u64(), 12345);
        assert_eq!(filtered_player.original_player.id, player.id);
        assert_eq!(filtered_player.original_player.full_name, player.full_name);
        assert_eq!(
            filtered_player.original_player.default_position_id,
            player.default_position_id
        );
    }

    #[test]
    fn test_matches_team_filter() {
        // Test team ID filter
        let player_with_team = create_test_player("Player", None, None, Some(true));
        let mut player_with_team = player_with_team;
        player_with_team.team_id = Some(123);
        player_with_team.team_name = Some("Kenny Rogers' Toasters".to_string());

        let team_id_filter = TeamFilter::TeamId(123);
        assert!(matches_team_filter(&player_with_team, &team_id_filter));

        let wrong_team_id_filter = TeamFilter::TeamId(456);
        assert!(!matches_team_filter(
            &player_with_team,
            &wrong_team_id_filter
        ));

        // Test team name filter with partial matching
        let team_name_filter = TeamFilter::TeamName("kenny".to_string());
        assert!(matches_team_filter(&player_with_team, &team_name_filter));

        let team_name_filter_caps = TeamFilter::TeamName("ROGERS".to_string());
        assert!(matches_team_filter(
            &player_with_team,
            &team_name_filter_caps
        ));

        let no_match_filter = TeamFilter::TeamName("nomatch".to_string());
        assert!(!matches_team_filter(&player_with_team, &no_match_filter));

        // Test with no team info
        let player_no_team = create_test_player("FA Player", None, None, Some(false));
        assert!(!matches_team_filter(&player_no_team, &team_id_filter));
        assert!(!matches_team_filter(&player_no_team, &team_name_filter));
    }

    #[test]
    fn test_apply_team_filter() {
        let mut players = vec![
            {
                let mut p = create_test_player("Player 1", None, None, Some(true));
                p.team_id = Some(123);
                p.team_name = Some("Kenny Rogers' Toasters".to_string());
                p
            },
            {
                let mut p = create_test_player("Player 2", None, None, Some(true));
                p.team_id = Some(456);
                p.team_name = Some("Mike's Misfits".to_string());
                p
            },
            create_test_player("FA Player", None, None, Some(false)),
        ];

        // Filter by team name
        apply_team_filter(&mut players, &TeamFilter::TeamName("kenny".to_string()));
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "Player 1");

        // Reset and filter by team ID
        let mut players = vec![
            {
                let mut p = create_test_player("Player 1", None, None, Some(true));
                p.team_id = Some(123);
                p.team_name = Some("Kenny Rogers' Toasters".to_string());
                p
            },
            {
                let mut p = create_test_player("Player 2", None, None, Some(true));
                p.team_id = Some(456);
                p.team_name = Some("Mike's Misfits".to_string());
                p
            },
        ];

        apply_team_filter(&mut players, &TeamFilter::TeamId(456));
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "Player 2");
    }

    #[test]
    fn test_apply_status_filters_with_team() {
        let mut players = vec![
            {
                let mut p = create_test_player(
                    "Active Kenny Player",
                    Some(false),
                    Some(InjuryStatus::Active),
                    Some(true),
                );
                p.team_id = Some(123);
                p.team_name = Some("Kenny Rogers' Toasters".to_string());
                p
            },
            {
                let mut p = create_test_player(
                    "Injured Kenny Player",
                    Some(true),
                    Some(InjuryStatus::Out),
                    Some(true),
                );
                p.team_id = Some(123);
                p.team_name = Some("Kenny Rogers' Toasters".to_string());
                p
            },
            {
                let mut p = create_test_player(
                    "Active Mike Player",
                    Some(false),
                    Some(InjuryStatus::Active),
                    Some(true),
                );
                p.team_id = Some(456);
                p.team_name = Some("Mike's Misfits".to_string());
                p
            },
        ];

        // Filter for active players on Kenny's team
        apply_status_filters(
            &mut players,
            Some(&InjuryStatusFilter::Active),
            None,
            Some(&TeamFilter::TeamName("kenny".to_string())),
        );

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].name, "Active Kenny Player");
    }

    #[test]
    fn test_position_filtering_comprehensive() {
        use crate::espn::types::Player;

        // Create players with different positions
        let players = vec![
            Player {
                id: 1,
                full_name: Some("QB Player".to_string()),
                default_position_id: 0, // QB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 2,
                full_name: Some("RB Player".to_string()),
                default_position_id: 2, // RB
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 3,
                full_name: Some("WR Player".to_string()),
                default_position_id: 3, // WR
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 4,
                full_name: Some("TE Player".to_string()),
                default_position_id: 4, // TE
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
            Player {
                id: 5,
                full_name: Some("K Player".to_string()),
                default_position_id: 5, // K
                stats: vec![],
                active: Some(true),
                injured: Some(false),
                injury_status: Some(InjuryStatus::Active),
            },
        ];

        // Test multiple position filters
        let position_filter = Some(vec![Position::QB, Position::K]);
        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players.clone(), None, position_filter).collect();
        assert_eq!(filtered.len(), 2); // QB and K

        // Test FLEX filter (should include RB, WR, TE)
        let position_filter = Some(vec![Position::FLEX]);
        let filtered: Vec<FilteredPlayer> =
            filter_and_convert_players(players, None, position_filter).collect();
        assert_eq!(filtered.len(), 3); // RB, WR, TE
    }
}
