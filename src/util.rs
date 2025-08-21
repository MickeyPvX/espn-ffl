//! Common helpers: cookie header builder and week-spec parsing.

use reqwest::header::{HeaderMap, HeaderValue, COOKIE, ACCEPT};
use std::collections::BTreeSet;
use std::error::Error;

/// Project-standard Result with Send+Sync error.
pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Build cookie headers from `ESPN_SWID` and `ESPN_S2`, if present.
///
/// Returns `Ok(None)` when either env var is missing (public leagues).
pub fn maybe_cookie_header_map() -> Result<Option<HeaderMap>> {
    let swid = std::env::var("ESPN_SWID").ok();
    let s2   = std::env::var("ESPN_S2").ok();
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

/// Parse a week spec like `1`, `1,3,5`, `2-6`, `1-4,6,8-10`.
///
/// - Returns a sorted, deduplicated `Vec<u16>`.
/// - Errors on invalid ranges (e.g., `6-2`) or non-numeric input.
pub fn parse_weeks_spec(spec: &str) -> Result<Vec<u16>> {
    let mut set = BTreeSet::new();
    for part in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if let Some((a, b)) = part.split_once('-') {
            let start: u16 = a.trim().parse()?;
            let end: u16 = b.trim().parse()?;
            if start > end {
                return Err(format!("invalid week range: {part}").into());
            }
            for w in start..=end {
                set.insert(w);
            }
        } else {
            set.insert(part.parse()?);
        }
    }
    Ok(set.into_iter().collect())
}
