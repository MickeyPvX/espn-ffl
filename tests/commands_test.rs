//! Integration tests for command handlers

use espn_ffl::{
    cli::types::position::Position,
    commands::{player_data::PlayerDataParams, resolve_league_id},
    espn::types::PlayerPoints,
    storage::*,
    EspnError, LeagueId, PlayerId, Season, Week, LEAGUE_ID_ENV_VAR,
};

#[test]
fn test_resolve_league_id_from_option() {
    let league_id = Some(LeagueId::new(12345));
    let result = resolve_league_id(league_id);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_u32(), 12345);
}

#[test]
fn test_resolve_league_id_from_env() {
    // Clear any existing env var
    std::env::remove_var(LEAGUE_ID_ENV_VAR);

    // Set test env var
    std::env::set_var(LEAGUE_ID_ENV_VAR, "54321");

    let result = resolve_league_id(None);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_u32(), 54321);

    // Clean up
    std::env::remove_var(LEAGUE_ID_ENV_VAR);
}

#[test]
fn test_resolve_league_id_missing() {
    // Clear env var
    std::env::remove_var(LEAGUE_ID_ENV_VAR);

    let result = resolve_league_id(None);
    assert!(result.is_err());
    match result.unwrap_err() {
        EspnError::MissingLeagueId { env_var } => {
            assert_eq!(env_var, LEAGUE_ID_ENV_VAR);
        }
        _ => panic!("Expected MissingLeagueId error"),
    }
}

#[test]
fn test_resolve_league_id_invalid_env() {
    // Set invalid env var
    std::env::set_var(LEAGUE_ID_ENV_VAR, "not_a_number");

    let result = resolve_league_id(None);
    assert!(result.is_err());

    // Clean up
    std::env::remove_var(LEAGUE_ID_ENV_VAR);
}

#[test]
fn test_resolve_league_id_option_overrides_env() {
    // Set env var
    std::env::set_var(LEAGUE_ID_ENV_VAR, "99999");

    // Option should take precedence
    let league_id = Some(LeagueId::new(12345));
    let result = resolve_league_id(league_id);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_u32(), 12345);

    // Clean up
    std::env::remove_var(LEAGUE_ID_ENV_VAR);
}

#[test]
fn test_resolve_league_id_zero_value() {
    // Test zero value from environment variable (should fail)
    std::env::set_var(LEAGUE_ID_ENV_VAR, "0");
    let result = resolve_league_id(None);
    assert!(result.is_err());
    std::env::remove_var(LEAGUE_ID_ENV_VAR);

    // Test that zero value as direct input is accepted (Some overrides validation)
    let league_id = Some(LeagueId::new(0));
    let result = resolve_league_id(league_id);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_u32(), 0);
}

// Test helper functions and data structures used in commands
#[test]
fn test_player_points_serialization() {
    let player_points = PlayerPoints {
        id: PlayerId::new(123456),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        week: Week::new(1),
        projected: false,
        points: 25.5,
        active: Some(true),
        injured: Some(false),
        injury_status: None,
        is_rostered: Some(true),
        team_id: Some(1),
        team_name: Some("Test Team".to_string()),
    };

    let json = serde_json::to_string(&player_points).unwrap();
    assert!(json.contains("123456"));
    assert!(json.contains("Test Player"));
    assert!(json.contains("25.5"));
    assert!(json.contains("false"));
}

#[test]
fn test_player_points_ordering() {
    let mut players = vec![
        PlayerPoints {
            id: PlayerId::new(1),
            name: "Player 1".to_string(),
            position: "RB".to_string(),
            week: Week::new(1),
            projected: false,
            points: 15.0,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(true),
            team_id: Some(1),
            team_name: Some("Team A".to_string()),
        },
        PlayerPoints {
            id: PlayerId::new(2),
            name: "Player 2".to_string(),
            position: "WR".to_string(),
            week: Week::new(1),
            projected: false,
            points: 25.0,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(true),
            team_id: Some(2),
            team_name: Some("Team B".to_string()),
        },
        PlayerPoints {
            id: PlayerId::new(3),
            name: "Player 3".to_string(),
            position: "TE".to_string(),
            week: Week::new(1),
            projected: false,
            points: 20.0,
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            is_rostered: Some(false),
            team_id: None,
            team_name: None,
        },
    ];

    // Sort by points descending (like in the actual command)
    players.sort_by(|a, b| {
        b.points
            .partial_cmp(&a.points)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    assert_eq!(players[0].points, 25.0);
    assert_eq!(players[1].points, 20.0);
    assert_eq!(players[2].points, 15.0);
}

#[test]
fn test_constants() {
    assert_eq!(LEAGUE_ID_ENV_VAR, "ESPN_FFL_LEAGUE_ID");
}

// Tests for new database functionality
#[test]
fn test_player_data_params_creation() {
    let params = PlayerDataParams::new(Season::new(2023), Week::new(1), true)
        .with_league_id(LeagueId::new(12345))
        .with_player_names(vec!["Test".to_string()])
        .with_positions(vec![Position::QB, Position::RB])
        .with_debug();

    assert!(params.debug);
    assert!(!params.as_json);
    assert_eq!(params.league_id, Some(LeagueId::new(12345)));
    assert_eq!(params.player_name, Some(vec!["Test".to_string()]));
    assert_eq!(params.positions, Some(vec![Position::QB, Position::RB]));
    assert!(params.projected);
    assert_eq!(params.season, Season::new(2023));
    assert_eq!(params.week, Week::new(1));
}

#[test]
fn test_performance_estimate_creation() {
    let estimate = PerformanceEstimate {
        player_id: PlayerId::new(12345),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: Some("TEST".to_string()),
        espn_projection: 17.0,
        bias_adjustment: 1.5,
        estimated_points: 18.5,
        confidence: 0.75,
        reasoning: "Based on historical data".to_string(),
    };

    assert_eq!(estimate.player_id, PlayerId::new(12345));
    assert_eq!(estimate.name, "Test Player");
    assert_eq!(estimate.position, "QB");
    assert_eq!(estimate.team, Some("TEST".to_string()));
    assert!((estimate.estimated_points - 18.5).abs() < 0.01);
    assert!((estimate.confidence - 0.75).abs() < 0.01);
    assert_eq!(estimate.reasoning, "Based on historical data");
}

#[test]
fn test_projection_analysis_creation() {
    let analysis = ProjectionAnalysis {
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: Some("TEST".to_string()),
        avg_error: 2.5,
        games_count: 10,
    };

    assert_eq!(analysis.name, "Test Player");
    assert_eq!(analysis.position, "QB");
    assert_eq!(analysis.team, Some("TEST".to_string()));
    assert!((analysis.avg_error - 2.5).abs() < 0.01);
    assert_eq!(analysis.games_count, 10);
}

#[test]
fn test_database_player_creation() {
    let player = Player {
        player_id: PlayerId::new(12345),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: Some("TEST".to_string()),
    };

    assert_eq!(player.player_id, PlayerId::new(12345));
    assert_eq!(player.name, "Test Player");
    assert_eq!(player.position, "QB");
    assert_eq!(player.team, Some("TEST".to_string()));
}

#[test]
fn test_player_weekly_stats_creation() {
    let stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12345),
        season: Season::new(2023),
        week: Week::new(1),
        projected_points: Some(20.0),
        actual_points: Some(18.5),
        active: Some(true),
        injured: Some(false),
        injury_status: None,
        is_rostered: Some(true),
        fantasy_team_id: Some(1),
        fantasy_team_name: Some("Test Team".to_string()),
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    assert_eq!(stats.player_id, PlayerId::new(12345));
    assert_eq!(stats.season, Season::new(2023));
    assert_eq!(stats.week, Week::new(1));
    assert_eq!(stats.projected_points, Some(20.0));
    assert_eq!(stats.actual_points, Some(18.5));
    assert_eq!(stats.created_at, 1234567890);
    assert_eq!(stats.updated_at, 1234567890);
}

#[test]
fn test_position_conversion_in_player_data() {
    // Test that Position::try_from works correctly for common position IDs
    assert_eq!(Position::try_from(0).unwrap(), Position::QB);
    assert_eq!(Position::try_from(2).unwrap(), Position::RB);
    assert_eq!(Position::try_from(3).unwrap(), Position::WR);
    assert_eq!(Position::try_from(4).unwrap(), Position::TE);
    assert_eq!(Position::try_from(5).unwrap(), Position::K);
    assert_eq!(Position::try_from(6).unwrap(), Position::TE);
    assert_eq!(Position::try_from(17).unwrap(), Position::K);
    assert_eq!(Position::try_from(16).unwrap(), Position::DEF);

    // Test unknown position
    assert!(Position::try_from(99).is_err());
}

#[test]
fn test_position_to_string() {
    assert_eq!(Position::QB.to_string(), "QB");
    assert_eq!(Position::RB.to_string(), "RB");
    assert_eq!(Position::WR.to_string(), "WR");
    assert_eq!(Position::TE.to_string(), "TE");
    assert_eq!(Position::K.to_string(), "K");
    assert_eq!(Position::DEF.to_string(), "D/ST");
}

#[test]
fn test_cached_data_includes_injury_and_roster_status() {
    use espn_ffl::espn::types::{CachedPlayerData, InjuryStatus, PlayerPoints};

    // This is a unit test to ensure cached PlayerPoints include status fields
    // This would catch the bug where cached data returned None for all status fields

    let player_points = PlayerPoints::from_cached_data(CachedPlayerData {
        player_id: PlayerId::new(999),
        name: "Test Player".to_string(),
        position: "RB".to_string(),
        points: 15.5,
        week: Week::new(1),
        projected: false,
        active: Some(false), // not active
        injured: Some(true), // injured
        injury_status: Some(InjuryStatus::Questionable),
        is_rostered: Some(true), // rostered
        team_id: Some(123),      // team_id
        team_name: Some("My Team".to_string()),
    });

    // These assertions would fail if cached data constructor ignores status fields
    assert_eq!(
        player_points.active,
        Some(false),
        "Cached data should preserve active status"
    );
    assert_eq!(
        player_points.injured,
        Some(true),
        "Cached data should preserve injured status"
    );
    assert_eq!(
        player_points.injury_status,
        Some(InjuryStatus::Questionable),
        "Cached data should preserve injury status"
    );
    assert_eq!(
        player_points.is_rostered,
        Some(true),
        "Cached data should preserve roster status"
    );
    assert_eq!(
        player_points.team_id,
        Some(123),
        "Cached data should preserve team ID"
    );
    assert_eq!(
        player_points.team_name,
        Some("My Team".to_string()),
        "Cached data should preserve team name"
    );
}

#[test]
fn test_cached_vs_fresh_data_status_consistency() {
    // This test would catch issues where cached and fresh data return different status info
    // Note: This is more of a conceptual test since we can't easily mock ESPN API calls

    use espn_ffl::espn::types::{
        CachedPlayerData, InjuryStatus, Player as EspnPlayer, PlayerPoints,
    };

    // Simulate what fresh data might look like
    let fresh_data = PlayerPoints::from_espn_player(
        PlayerId::new(12345),
        &EspnPlayer {
            id: 12345,
            full_name: Some("Josh Allen".to_string()),
            default_position_id: 0, // QB
            stats: vec![],          // Empty stats vec
            active: Some(true),
            injured: Some(false),
            injury_status: Some(InjuryStatus::Active),
        },
        "QB".to_string(),
        25.0,
        Week::new(1),
        false,
    );

    // Simulate corresponding cached data (this should match fresh data)
    let cached_data = PlayerPoints::from_cached_data(CachedPlayerData {
        player_id: PlayerId::new(12345),
        name: "Josh Allen".to_string(),
        position: "QB".to_string(),
        points: 25.0,
        week: Week::new(1),
        projected: false,
        active: Some(true),                        // active - should match fresh
        injured: Some(false),                      // injured - should match fresh
        injury_status: Some(InjuryStatus::Active), // injury_status - should match fresh
        is_rostered: Some(true),                   // rostered (example)
        team_id: Some(42),                         // team_id (example)
        team_name: Some("Test Team".to_string()),  // team_name (example)
    });

    // Status fields should match between fresh and cached data
    assert_eq!(
        fresh_data.active, cached_data.active,
        "Active status should match between fresh and cached data"
    );
    assert_eq!(
        fresh_data.injured, cached_data.injured,
        "Injured status should match between fresh and cached data"
    );
    assert_eq!(
        fresh_data.injury_status, cached_data.injury_status,
        "Injury status should match between fresh and cached data"
    );

    // Basic fields should also match
    assert_eq!(fresh_data.id, cached_data.id);
    assert_eq!(fresh_data.points, cached_data.points);
    assert_eq!(fresh_data.projected, cached_data.projected);
}

#[cfg(test)]
mod projection_analysis_filtering_tests {
    use super::*;
    use espn_ffl::{
        cli::types::filters::{InjuryStatusFilter, RosterStatusFilter},
        commands::player_filters::{matches_injury_filter, matches_roster_filter},
        espn::types::{InjuryStatus, PlayerPoints},
    };

    fn create_test_player_points(
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
    fn test_projection_analysis_roster_status_filtering_works() {
        // This test ensures that roster status filtering works correctly for projection analysis
        // and would catch the bug where filtering returned no results for future weeks

        let rostered_player = create_test_player_points("Rostered QB", None, None, Some(true));
        let fa_player = create_test_player_points("FA QB", None, None, Some(false));
        let unknown_player = create_test_player_points("Unknown QB", None, None, None);

        // Test rostered filter
        assert!(matches_roster_filter(
            &rostered_player,
            &RosterStatusFilter::Rostered
        ));
        assert!(!matches_roster_filter(
            &fa_player,
            &RosterStatusFilter::Rostered
        ));
        assert!(!matches_roster_filter(
            &unknown_player,
            &RosterStatusFilter::Rostered
        ));

        // Test free agent filter
        assert!(!matches_roster_filter(
            &rostered_player,
            &RosterStatusFilter::FA
        ));
        assert!(matches_roster_filter(&fa_player, &RosterStatusFilter::FA));
        assert!(!matches_roster_filter(
            &unknown_player,
            &RosterStatusFilter::FA
        )); // None defaults to NOT FA
    }

    #[test]
    fn test_projection_analysis_injury_status_filtering_works() {
        // This test ensures that injury status filtering works correctly for projection analysis

        let active_player =
            create_test_player_points("Active QB", Some(false), Some(InjuryStatus::Active), None);
        let injured_player =
            create_test_player_points("Injured QB", Some(true), Some(InjuryStatus::Out), None);
        let questionable_player = create_test_player_points(
            "Questionable QB",
            Some(true),
            Some(InjuryStatus::Questionable),
            None,
        );

        // Test active filter
        assert!(matches_injury_filter(
            &active_player,
            &InjuryStatusFilter::Active
        ));
        assert!(!matches_injury_filter(
            &injured_player,
            &InjuryStatusFilter::Active
        ));
        assert!(!matches_injury_filter(
            &questionable_player,
            &InjuryStatusFilter::Active
        ));

        // Test injured filter
        assert!(!matches_injury_filter(
            &active_player,
            &InjuryStatusFilter::Injured
        ));
        assert!(matches_injury_filter(
            &injured_player,
            &InjuryStatusFilter::Injured
        ));
        assert!(matches_injury_filter(
            &questionable_player,
            &InjuryStatusFilter::Injured
        ));

        // Test specific status filters
        assert!(matches_injury_filter(
            &injured_player,
            &InjuryStatusFilter::Out
        ));
        assert!(!matches_injury_filter(
            &questionable_player,
            &InjuryStatusFilter::Out
        ));
        assert!(matches_injury_filter(
            &questionable_player,
            &InjuryStatusFilter::Questionable
        ));
        assert!(!matches_injury_filter(
            &injured_player,
            &InjuryStatusFilter::Questionable
        ));
    }

    #[test]
    fn test_projection_analysis_combined_filtering() {
        // This test ensures that both injury and roster status filters work together
        // This would catch bugs where one filter interferes with another

        let active_rostered = create_test_player_points(
            "Active Rostered",
            Some(false),
            Some(InjuryStatus::Active),
            Some(true),
        );
        let active_fa = create_test_player_points(
            "Active FA",
            Some(false),
            Some(InjuryStatus::Active),
            Some(false),
        );
        let injured_rostered = create_test_player_points(
            "Injured Rostered",
            Some(true),
            Some(InjuryStatus::Out),
            Some(true),
        );
        let injured_fa = create_test_player_points(
            "Injured FA",
            Some(true),
            Some(InjuryStatus::Out),
            Some(false),
        );

        // Test combined filters: active + FA
        assert!(
            !matches_injury_filter(&active_rostered, &InjuryStatusFilter::Active)
                || !matches_roster_filter(&active_rostered, &RosterStatusFilter::FA)
        );
        assert!(
            matches_injury_filter(&active_fa, &InjuryStatusFilter::Active)
                && matches_roster_filter(&active_fa, &RosterStatusFilter::FA)
        );
        assert!(
            !matches_injury_filter(&injured_rostered, &InjuryStatusFilter::Active)
                || !matches_roster_filter(&injured_rostered, &RosterStatusFilter::FA)
        );
        assert!(
            !matches_injury_filter(&injured_fa, &InjuryStatusFilter::Active)
                || !matches_roster_filter(&injured_fa, &RosterStatusFilter::FA)
        );

        // Test combined filters: injured + rostered
        assert!(
            !matches_injury_filter(&active_rostered, &InjuryStatusFilter::Injured)
                || !matches_roster_filter(&active_rostered, &RosterStatusFilter::Rostered)
        );
        assert!(
            !matches_injury_filter(&active_fa, &InjuryStatusFilter::Injured)
                || !matches_roster_filter(&active_fa, &RosterStatusFilter::Rostered)
        );
        assert!(
            matches_injury_filter(&injured_rostered, &InjuryStatusFilter::Injured)
                && matches_roster_filter(&injured_rostered, &RosterStatusFilter::Rostered)
        );
        assert!(
            !matches_injury_filter(&injured_fa, &InjuryStatusFilter::Injured)
                || !matches_roster_filter(&injured_fa, &RosterStatusFilter::Rostered)
        );
    }

    #[test]
    fn test_filtering_with_missing_status_data() {
        // This test catches the specific bug we fixed where players without current status data
        // were incorrectly filtered out when they should have been included or excluded based on defaults

        let player_no_injury_data =
            create_test_player_points("No Injury Data", None, None, Some(true));
        let player_no_roster_data = create_test_player_points(
            "No Roster Data",
            Some(false),
            Some(InjuryStatus::Active),
            None,
        );
        let player_no_data = create_test_player_points("No Data", None, None, None);

        // Players with missing injury data should be treated as not injured for "Active" filter
        // This test would catch the bug where we required cached data to exist
        assert!(matches_injury_filter(
            &player_no_injury_data,
            &InjuryStatusFilter::Active
        ));
        assert!(!matches_injury_filter(
            &player_no_injury_data,
            &InjuryStatusFilter::Injured
        ));

        // Players with missing roster data should NOT be treated as free agents (conservative default)
        assert!(!matches_roster_filter(
            &player_no_roster_data,
            &RosterStatusFilter::Rostered
        ));
        assert!(!matches_roster_filter(
            &player_no_roster_data,
            &RosterStatusFilter::FA
        ));

        // Players with no data should have sensible defaults
        assert!(matches_injury_filter(
            &player_no_data,
            &InjuryStatusFilter::Active
        ));
        assert!(!matches_injury_filter(
            &player_no_data,
            &InjuryStatusFilter::Injured
        ));
        assert!(!matches_roster_filter(
            &player_no_data,
            &RosterStatusFilter::Rostered
        ));
        assert!(!matches_roster_filter(
            &player_no_data,
            &RosterStatusFilter::FA
        ));
    }
}
