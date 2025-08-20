use reqwest::header::{ACCEPT, COOKIE, HeaderMap, HeaderValue};
use std::error::Error;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Returns cookie headers if ESPN_SWID + ESPN_S2 are set; otherwise Ok(None).
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
