//! Common helpers: cookie header builder and week-spec parsing.

use reqwest::header::{ACCEPT, COOKIE, HeaderMap, HeaderValue};
use std::error::Error;

/// Project-standard Result with Send+Sync error.
pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Build cookie headers from `ESPN_SWID` and `ESPN_S2`, if present.
///
/// Returns `Ok(None)` when either env var is missing (public leagues).
pub fn maybe_cookie_header_map() -> Result<Option<HeaderMap>> {
    let swid = std::env::var("ESPN_SWID").ok();
    let s2 = std::env::var("ESPN_S2").ok();
    if let (Some(swid), Some(s2)) = (swid, s2) {
        let mut h = HeaderMap::new();
        h.insert(ACCEPT, HeaderValue::from_static("application/json"));
        let cookie = format!("SWID={}; espn_s2={}", swid, s2);
        h.insert(COOKIE, HeaderValue::from_str(&cookie)?);
        Ok(Some(h))
    } else {
        Ok(None)
    }
}
