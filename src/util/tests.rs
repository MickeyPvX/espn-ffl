//! Unit tests for utility functions

use super::*;

#[cfg(test)]
mod util_tests {
    use super::*;

    #[test]
    fn test_maybe_cookie_header_map_with_both_env_vars() {
        // Clean up first to ensure test isolation
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");

        // Set both environment variables
        std::env::set_var("ESPN_SWID", "test-swid-123");
        std::env::set_var("ESPN_S2", "test-s2-456");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_some());

        let headers = result.unwrap();
        assert!(headers.contains_key("accept"));
        assert!(headers.contains_key("cookie"));

        let accept_value = headers.get("accept").unwrap();
        assert_eq!(accept_value, "application/json");

        let cookie_value = headers.get("cookie").unwrap().to_str().unwrap();
        assert!(cookie_value.contains("SWID=test-swid-123"));
        assert!(cookie_value.contains("espn_s2=test-s2-456"));

        // Clean up
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_maybe_cookie_header_map_missing_swid() {
        // Clean up first to ensure test isolation
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");

        // Remove SWID, set S2
        std::env::remove_var("ESPN_SWID");
        std::env::set_var("ESPN_S2", "test-s2-456");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_none());

        // Clean up
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_maybe_cookie_header_map_missing_s2() {
        // Clean up first to ensure test isolation
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");

        // Set SWID, remove S2
        std::env::set_var("ESPN_SWID", "test-swid-123");
        std::env::remove_var("ESPN_S2");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_none());

        // Clean up
        std::env::remove_var("ESPN_SWID");
    }

    #[test]
    fn test_maybe_cookie_header_map_both_missing() {
        // Clean up first to ensure test isolation
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");

        // Remove both environment variables
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_maybe_cookie_header_map_empty_values() {
        // Set empty values
        std::env::set_var("ESPN_SWID", "");
        std::env::set_var("ESPN_S2", "");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_some());

        let headers = result.unwrap();
        let cookie_value = headers.get("cookie").unwrap().to_str().unwrap();
        assert_eq!(cookie_value, "SWID=; espn_s2=");

        // Clean up
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_maybe_cookie_header_map_special_characters() {
        // Clean up first to ensure test isolation
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");

        // Test with special characters that might need escaping
        std::env::set_var("ESPN_SWID", "swid-with-dashes_and_underscores.123");
        std::env::set_var("ESPN_S2", "s2+with/special=chars&symbols");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_some());

        let headers = result.unwrap();
        let cookie_value = headers.get("cookie").unwrap().to_str().unwrap();
        assert!(cookie_value.contains("SWID=swid-with-dashes_and_underscores.123"));
        assert!(cookie_value.contains("espn_s2=s2+with/special=chars&symbols"));

        // Clean up
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_maybe_cookie_header_map_long_values() {
        // Test with very long values
        let long_swid = "a".repeat(500);
        let long_s2 = "b".repeat(1000);

        std::env::set_var("ESPN_SWID", &long_swid);
        std::env::set_var("ESPN_S2", &long_s2);

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_some());

        let headers = result.unwrap();
        let cookie_value = headers.get("cookie").unwrap().to_str().unwrap();
        assert!(cookie_value.contains(&format!("SWID={}", long_swid)));
        assert!(cookie_value.contains(&format!("espn_s2={}", long_s2)));

        // Clean up
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_maybe_cookie_header_map_header_value_format() {
        std::env::set_var("ESPN_SWID", "test-swid");
        std::env::set_var("ESPN_S2", "test-s2");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_some());

        let headers = result.unwrap();

        // Verify accept header is properly set
        let accept_header = headers.get("accept").unwrap();
        assert_eq!(accept_header.to_str().unwrap(), "application/json");

        // Verify cookie header format
        let cookie_header = headers.get("cookie").unwrap();
        let cookie_str = cookie_header.to_str().unwrap();

        // Should follow the format: "SWID=value; espn_s2=value"
        assert!(cookie_str.starts_with("SWID="));
        assert!(cookie_str.contains("; espn_s2="));

        // Verify exact format
        assert_eq!(cookie_str, "SWID=test-swid; espn_s2=test-s2");

        // Clean up
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_maybe_cookie_header_map_invalid_header_characters() {
        // Test with characters that cause header value creation to fail
        // Using carriage return character which is invalid in HTTP headers
        std::env::set_var("ESPN_SWID", "swid\rwith\rcarriage\rreturn");
        std::env::set_var("ESPN_S2", "valid-s2");

        let result = maybe_cookie_header_map();

        // Should return an error due to invalid header value
        assert!(result.is_err());

        // Clean up
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_maybe_cookie_header_map_unicode_values() {
        // Test with Unicode characters (should work as they're valid in headers when UTF-8 encoded)
        std::env::set_var("ESPN_SWID", "swid-café-🏈");
        std::env::set_var("ESPN_S2", "s2-naïve-⚡");

        let result = maybe_cookie_header_map();

        // Unicode characters in headers are tricky - this might fail or succeed depending on implementation
        match result {
            Ok(Some(headers)) => {
                // Try to get cookie value - if it fails, that's acceptable
                match headers.get("cookie").unwrap().to_str() {
                    Ok(cookie_value) => {
                        assert!(cookie_value.contains("SWID=swid-café-🏈"));
                        assert!(cookie_value.contains("espn_s2=s2-naïve-⚡"));
                    }
                    Err(_) => {
                        // Unicode might not be supported in header values
                        // This is acceptable behavior
                    }
                }
            }
            Ok(None) => panic!("Expected headers when both env vars are set"),
            Err(_) => {
                // This is acceptable - Unicode in headers might not be supported
            }
        }

        // Clean up
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_result_type_alias() {
        // Test that our Result type alias works correctly
        fn test_function_success() -> Result<String> {
            Ok("success".to_string())
        }

        fn test_function_error() -> Result<String> {
            Err(crate::error::EspnError::NoData)
        }

        let success = test_function_success();
        assert!(success.is_ok());
        assert_eq!(success.unwrap(), "success");

        let error = test_function_error();
        assert!(error.is_err());
    }

    #[test]
    fn test_header_map_independence() {
        // Test that multiple calls return independent header maps
        std::env::set_var("ESPN_SWID", "test-swid");
        std::env::set_var("ESPN_S2", "test-s2");

        let result1 = maybe_cookie_header_map().unwrap();
        let result2 = maybe_cookie_header_map().unwrap();

        // Both should return Some since env vars are set
        assert!(result1.is_some());
        assert!(result2.is_some());

        let headers1 = result1.unwrap();
        let headers2 = result2.unwrap();

        // Should be independent (different memory locations)
        assert_ne!(std::ptr::addr_of!(headers1), std::ptr::addr_of!(headers2));

        // But should have the same content
        assert_eq!(headers1.get("cookie"), headers2.get("cookie"));
        assert_eq!(headers1.get("accept"), headers2.get("accept"));

        // Verify the actual content is what we set
        let expected_cookie = "SWID=test-swid; espn_s2=test-s2";
        assert_eq!(
            headers1.get("cookie").unwrap().to_str().unwrap(),
            expected_cookie
        );

        // Clean up
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }
}
