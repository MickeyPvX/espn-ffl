//! Unit tests for error handling

use super::*;
use std::io;

#[cfg(test)]
mod espn_error_tests {
    use super::*;

    #[tokio::test]
    async fn test_http_error_conversion() {
        // Create a real HTTP error by making a request to an invalid URL
        let client = reqwest::Client::new();
        let result = client
            .get("http://invalid-url-that-does-not-exist.fake")
            .send()
            .await;
        let reqwest_error = result.unwrap_err();
        let espn_error = EspnError::from(reqwest_error);

        match espn_error {
            EspnError::Http(_) => (),
            _ => panic!("Expected Http error variant"),
        }
    }

    #[test]
    fn test_json_error_conversion() {
        // Create a JSON error by trying to parse invalid JSON
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let espn_error = EspnError::from(json_error);

        match espn_error {
            EspnError::Json(_) => (),
            _ => panic!("Expected Json error variant"),
        }
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let espn_error = EspnError::from(io_error);

        match espn_error {
            EspnError::Io(_) => (),
            _ => panic!("Expected Io error variant"),
        }
    }

    #[test]
    fn test_invalid_header_error_conversion() {
        // Create an invalid header by using invalid characters
        let header_error = reqwest::header::HeaderValue::from_str("invalid\nheader").unwrap_err();
        let espn_error = EspnError::from(header_error);

        match espn_error {
            EspnError::InvalidHeader(_) => (),
            _ => panic!("Expected InvalidHeader error variant"),
        }
    }

    #[test]
    fn test_parse_int_error_conversion() {
        let parse_error = "not_a_number".parse::<u32>().unwrap_err();
        let espn_error = EspnError::from(parse_error);

        match espn_error {
            EspnError::InvalidLeagueId(_) => (),
            _ => panic!("Expected InvalidLeagueId error variant"),
        }
    }

    #[test]
    fn test_missing_league_id_error() {
        let error = EspnError::MissingLeagueId {
            env_var: "ESPN_FFL_LEAGUE_ID".to_string(),
        };

        let error_string = error.to_string();
        assert!(error_string.contains("League ID not provided"));
        assert!(error_string.contains("ESPN_FFL_LEAGUE_ID"));
    }

    #[test]
    fn test_cache_error() {
        let error = EspnError::Cache {
            message: "Failed to write cache".to_string(),
        };

        let error_string = error.to_string();
        assert!(error_string.contains("Cache error"));
        assert!(error_string.contains("Failed to write cache"));
    }

    #[test]
    fn test_no_data_error() {
        let error = EspnError::NoData;
        let error_string = error.to_string();
        assert_eq!(error_string, "ESPN API returned no data");
    }

    #[test]
    fn test_invalid_position_error() {
        let error = EspnError::InvalidPosition {
            position: "INVALID_POS".to_string(),
        };

        let error_string = error.to_string();
        assert!(error_string.contains("Invalid position"));
        assert!(error_string.contains("INVALID_POS"));
    }

    #[test]
    fn test_player_not_found_error() {
        let error = EspnError::PlayerNotFound {
            name: "John Doe".to_string(),
        };

        let error_string = error.to_string();
        assert!(error_string.contains("Player not found"));
        assert!(error_string.contains("John Doe"));
    }

    #[test]
    fn test_invalid_scoring_error() {
        let error = EspnError::InvalidScoring;
        let error_string = error.to_string();
        assert_eq!(error_string, "Invalid scoring configuration");
    }

    #[test]
    fn test_box_error_conversion() {
        let box_error: Box<dyn std::error::Error + Send + Sync> = Box::new(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Access denied",
        ));
        let espn_error = EspnError::from(box_error);

        match espn_error {
            EspnError::Cache { message } => {
                assert!(message.contains("Access denied"));
            }
            _ => panic!("Expected Cache error variant"),
        }
    }

    #[test]
    fn test_error_source_chain() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let espn_error = EspnError::from(io_error);

        // Test that the error implements std::error::Error properly
        let error_trait: &dyn std::error::Error = &espn_error;
        assert!(error_trait.source().is_some());
    }

    #[test]
    fn test_error_debug_formatting() {
        let error = EspnError::NoData;
        let debug_string = format!("{:?}", error);
        assert_eq!(debug_string, "NoData");
    }

    #[test]
    fn test_result_type_alias() {
        fn test_function() -> Result<String> {
            Ok("success".to_string())
        }

        let result = test_function();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn test_result_type_alias_error() {
        fn test_function() -> Result<String> {
            Err(EspnError::NoData)
        }

        let result = test_function();
        assert!(result.is_err());
        match result.unwrap_err() {
            EspnError::NoData => (),
            _ => panic!("Expected NoData error"),
        }
    }

    #[test]
    fn test_anyhow_error_conversion() {
        // Test From<anyhow::Error> implementation
        let anyhow_error = anyhow::anyhow!("Test anyhow error message");
        let espn_error = EspnError::from(anyhow_error);

        match espn_error {
            EspnError::Cache { message } => {
                assert!(message.contains("Test anyhow error message"));
            }
            _ => panic!("Expected Cache error variant"),
        }
    }

    #[test]
    fn test_database_error_conversion() {
        // Test From<rusqlite::Error> implementation
        let db_error = rusqlite::Error::InvalidColumnType(
            0,
            "test_column".to_string(),
            rusqlite::types::Type::Null,
        );
        let espn_error = EspnError::from(db_error);

        match espn_error {
            EspnError::Database(_) => (),
            _ => panic!("Expected Database error variant"),
        }
    }

    #[test]
    fn test_system_time_error_conversion() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        // Create a SystemTimeError by trying to get a duration from a future time
        let future_time = SystemTime::now() + Duration::from_secs(100);
        let system_time_error = UNIX_EPOCH.duration_since(future_time).unwrap_err();
        let espn_error = EspnError::from(system_time_error);

        match espn_error {
            EspnError::SystemTime(_) => (),
            _ => panic!("Expected SystemTime error variant"),
        }
    }
}
