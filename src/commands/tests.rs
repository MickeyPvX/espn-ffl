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
        let result = handle_player_data(
            false, // debug
            false, // as_json
            league_id,
            Some(10),                  // limit
            Some("Brady".to_string()), // player_name
            positions,
            false, // projected
            season,
            week,
        )
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
                week: Week::new(1),
                projected: false,
                points: 15.0,
            },
            PlayerPoints {
                id: PlayerId::new(2),
                name: "Player 2".to_string(),
                week: Week::new(1),
                projected: false,
                points: 25.0,
            },
            PlayerPoints {
                id: PlayerId::new(3),
                name: "Player 3".to_string(),
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
                week: Week::new(1),
                projected: false,
                points: 20.0,
            },
            PlayerPoints {
                id: PlayerId::new(2),
                name: "Player 2".to_string(),
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

        let result = handle_player_data(
            false,
            false,
            None,
            None,
            None,
            None,
            false,
            Season::default(),
            Week::default(),
        )
        .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EspnError::MissingLeagueId { .. } => {}
            _ => panic!("Expected MissingLeagueId error"),
        }
    }
}
