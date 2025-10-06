//! Unit tests for HTTP client functionality

use super::*;

#[cfg(test)]
mod http_tests {
    use super::*;

    #[test]
    fn test_ffl_base_url_constant() {
        assert_eq!(
            FFL_BASE_URL,
            "https://lm-api-reads.fantasy.espn.com/apis/v3/games/ffl"
        );
    }

    #[test]
    fn test_client_initialization() {
        // Test that the static CLIENT is properly initialized
        let client = &*CLIENT;

        // We can't directly test the user agent, but we can verify the client exists
        assert!(std::ptr::addr_of!(*client) as usize != 0);
    }
}
