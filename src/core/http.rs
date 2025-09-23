//! HTTP utilities for ESPN API communication

use crate::Result;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE};

/// Build cookie headers from `ESPN_SWID` and `ESPN_S2`, if present.
///
/// Returns `Ok(None)` when either env var is missing (public leagues).
pub fn maybe_cookie_header_map() -> Result<Option<HeaderMap>> {
    let swid = std::env::var("ESPN_SWID").ok();
    let s2 = std::env::var("ESPN_S2").ok();
    if let (Some(swid), Some(s2)) = (swid, s2) {
        let mut h = HeaderMap::new();
        h.insert(ACCEPT, HeaderValue::from_static("application/json"));
        // Remove curly braces from SWID if present
        let cookie = format!("SWID={}; espn_s2={}", swid, s2);
        h.insert(COOKIE, HeaderValue::from_str(&cookie)?);
        Ok(Some(h))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maybe_cookie_header_map_with_env_vars() {
        std::env::set_var("ESPN_SWID", "test_swid");
        std::env::set_var("ESPN_S2", "test_s2");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_some());

        let headers = result.unwrap();
        assert!(headers.contains_key(ACCEPT));
        assert!(headers.contains_key(COOKIE));

        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");
    }

    #[test]
    fn test_maybe_cookie_header_map_without_env_vars() {
        std::env::remove_var("ESPN_SWID");
        std::env::remove_var("ESPN_S2");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_maybe_cookie_header_map_partial_env_vars() {
        std::env::set_var("ESPN_SWID", "test_swid");
        std::env::remove_var("ESPN_S2");

        let result = maybe_cookie_header_map().unwrap();
        assert!(result.is_none());

        std::env::remove_var("ESPN_SWID");
    }
}