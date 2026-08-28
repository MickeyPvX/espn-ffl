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

    #[test]
    fn test_finished_seasons_never_expire() {
        let past = Season::new(Season::current().as_u16() - 1);
        assert_eq!(
            season_cache_max_age(past),
            None,
            "a completed season's numbers are final"
        );
    }

    #[test]
    fn test_live_season_expires() {
        assert_eq!(
            season_cache_max_age(Season::current()),
            Some(LIVE_SEASON_MAX_AGE)
        );
        // A season ESPN has published but not yet played is still in motion.
        let future = Season::new(Season::current().as_u16() + 1);
        assert_eq!(season_cache_max_age(future), Some(LIVE_SEASON_MAX_AGE));
    }

    #[test]
    fn test_live_season_ttl_is_short_enough_to_track_a_game() {
        assert!(
            LIVE_SEASON_MAX_AGE <= std::time::Duration::from_secs(60 * 60),
            "a live week must not go stale for an hour"
        );
    }
}
