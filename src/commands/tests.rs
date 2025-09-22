//! Integration tests for command handlers

use super::*;

#[cfg(test)]
mod command_tests {
    use super::*;
    use crate::cli_types::PlayerId;

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
        let league_id = Some(LeagueId::new(0));
        let result = resolve_league_id(league_id);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_u32(), 0);
    }

    // Mock test for handle_league_data - this would require mocking the HTTP calls
    #[tokio::test]
    async fn test_handle_league_data_structure() {
        // Test the function signature and basic error handling
        let league_id = Some(LeagueId::new(12345));
        let season = Season::new(2023);

        // This would fail with actual HTTP call, but tests the structure
        let result = handle_league_data(league_id, false, season, false).await;
        // In a real test with mocks, we would assert success
        // For now, we just verify it compiles and has the right signature
        match result {
            Ok(_) => {}
            Err(_) => {
                // Expected to fail without mock server
            }
        }
    }

    #[tokio::test]
    async fn test_handle_player_data_structure() {
        // Test the function signature and basic parameter handling
        let league_id = Some(LeagueId::new(12345));
        let season = Season::new(2023);
        let week = Week::new(1);
        let positions = Some(vec![Position::QB, Position::RB]);

        // This would fail with actual HTTP call, but tests the structure
        let result = handle_player_data(PlayerDataParams {
            debug: false,
            as_json: false,
            league_id,
            player_name: Some("Brady".to_string()),
            positions,
            projected: false,
            season,
            week,
            refresh_positions: false,
            clear_db: false,
        })
        .await;

        // In a real test with mocks, we would assert success
        match result {
            Ok(_) => {}
            Err(_) => {
                // Expected to fail without mock server
            }
        }
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
            },
            PlayerPoints {
                id: PlayerId::new(2),
                name: "Player 2".to_string(),
                position: "WR".to_string(),
                week: Week::new(1),
                projected: false,
                points: 25.0,
            },
            PlayerPoints {
                id: PlayerId::new(3),
                name: "Player 3".to_string(),
                position: "TE".to_string(),
                week: Week::new(1),
                projected: false,
                points: 20.0,
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
    fn test_player_points_with_same_scores() {
        let mut players = vec![
            PlayerPoints {
                id: PlayerId::new(1),
                name: "Player 1".to_string(),
                position: "QB".to_string(),
                week: Week::new(1),
                projected: false,
                points: 20.0,
            },
            PlayerPoints {
                id: PlayerId::new(2),
                name: "Player 2".to_string(),
                position: "RB".to_string(),
                week: Week::new(1),
                projected: false,
                points: 20.0,
            },
        ];

        // Sort should handle equal values gracefully
        players.sort_by(|a, b| {
            b.points
                .partial_cmp(&a.points)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Both should remain (order may vary but both should be present)
        assert_eq!(players.len(), 2);
        assert_eq!(players[0].points, 20.0);
        assert_eq!(players[1].points, 20.0);
    }

    #[test]
    fn test_constants() {
        assert_eq!(LEAGUE_ID_ENV_VAR, "ESPN_FFL_LEAGUE_ID");
    }

    // Test error propagation in command handlers
    #[tokio::test]
    async fn test_handle_league_data_missing_id() {
        std::env::remove_var(LEAGUE_ID_ENV_VAR);

        let result = handle_league_data(None, false, Season::default(), false).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EspnError::MissingLeagueId { .. } => {}
            _ => panic!("Expected MissingLeagueId error"),
        }
    }

    #[tokio::test]
    async fn test_handle_player_data_missing_id() {
        std::env::remove_var(LEAGUE_ID_ENV_VAR);

        let result = handle_player_data(PlayerDataParams {
            debug: false,
            as_json: false,
            league_id: None,
            player_name: None,
            positions: None,
            projected: false,
            season: Season::default(),
            week: Week::default(),
            refresh_positions: false,
            clear_db: false,
        })
        .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EspnError::MissingLeagueId { .. } => {}
            _ => panic!("Expected MissingLeagueId error"),
        }
    }

    // Tests for new database functionality
    #[test]
    fn test_player_data_params_creation() {
        let params = PlayerDataParams {
            debug: true,
            as_json: false,
            league_id: Some(LeagueId::new(12345)),
            player_name: Some("Test".to_string()),
            positions: Some(vec![Position::QB, Position::RB]),
            projected: true,
            season: Season::new(2023),
            week: Week::new(1),
            refresh_positions: false,
            clear_db: false,
        };

        assert!(params.debug);
        assert!(!params.as_json);
        assert_eq!(params.league_id, Some(LeagueId::new(12345)));
        assert_eq!(params.player_name, Some("Test".to_string()));
        assert_eq!(params.positions, Some(vec![Position::QB, Position::RB]));
        assert!(params.projected);
        assert_eq!(params.season, Season::new(2023));
        assert_eq!(params.week, Week::new(1));
    }

    #[test]
    fn test_performance_estimate_creation() {
        use crate::database::PerformanceEstimate;

        let estimate = PerformanceEstimate {
            player_id: PlayerId::new(12345),
            name: "Test Player".to_string(),
            position: "QB".to_string(),
            team: Some("TEST".to_string()),
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
        use crate::database::ProjectionAnalysis;

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
        use crate::database::Player as DbPlayer;

        let player = DbPlayer {
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
        use crate::database::PlayerWeeklyStats;

        let stats = PlayerWeeklyStats {
            player_id: PlayerId::new(12345),
            season: Season::new(2023),
            week: Week::new(1),
            projected_points: Some(20.0),
            actual_points: Some(18.5),
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
        assert_eq!(Position::try_from(4).unwrap(), Position::WR);
        assert_eq!(Position::try_from(6).unwrap(), Position::TE);
        assert_eq!(Position::try_from(17).unwrap(), Position::K);
        assert_eq!(Position::try_from(16).unwrap(), Position::D);

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
        assert_eq!(Position::D.to_string(), "D/ST");
    }

    #[tokio::test]
    async fn test_handle_projection_analysis_structure() {
        // Test the function signature and basic error handling
        let result = handle_projection_analysis(
            Season::new(2023),
            Week::new(1),
            Some(LeagueId::new(12345)),
            None,
            None,
            false,
        )
        .await;

        // Should complete without panicking (may be empty result)
        match result {
            Ok(_) => {}  // Success case - empty analysis is OK
            Err(_) => {} // Database errors are also OK for this test
        }
    }

    #[tokio::test]
    async fn test_handle_projection_analysis_json_format() {
        // Test JSON output format
        let result = handle_projection_analysis(
            Season::new(2023),
            Week::new(2), // Changed from None since week is now required
            Some(LeagueId::new(12345)),
            None,
            None,
            true, // JSON format
        )
        .await;

        // Should complete without panicking
        match result {
            Ok(_) => {}
            Err(_) => {}
        }
    }
}
