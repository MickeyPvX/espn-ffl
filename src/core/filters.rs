//! ESPN API filter utilities for efficient player data retrieval.
//!
//! This module provides structures and functions for building ESPN API filters
//! that reduce the amount of data transferred by filtering players server-side.
//!
//! # Supported Filters
//!
//! ESPN's API supports limited server-side filtering. Through systematic testing,
//! we've identified which filters actually work:
//!
//! - **filterActive**: Filter by player activity status (works)
//! - **filterInjured**: Filter by injury status (works)
//! - **filterName**: Filter by player name (works for single names)
//! - **filterSlotIds**: Filter by position IDs (works)
//!
//! Other filters like `filterHealthy`, `filterFreeAgent`, etc. are ignored by ESPN's API.

use crate::Result;
use reqwest::header::HeaderValue;
use serde::Serialize;

/// Wrapper for ESPN-style filter values.
///
/// ESPN API expects filter values to be wrapped in objects with a "value" field.
/// For example: `{"filterActive": {"value": true}}`
#[derive(Debug, Serialize)]
pub struct Val<T> {
    pub value: T,
}

/// Filter parameters for ESPN's `/players` endpoint.
///
/// Only includes filters that have been verified to work with ESPN's API.
/// Unused optional fields are automatically excluded from serialization.
///
/// # Examples
///
/// ```rust
/// use espn_ffl::core::filters::{PlayersFilter, Val, IntoHeaderValue};
///
/// let mut filter = PlayersFilter::default();
/// filter.filter_active = Some(Val { value: true });
/// filter.filter_injured = Some(Val { value: false });
///
/// // Convert to HTTP header for API request
/// let header_value = filter.to_header_value().unwrap();
/// ```
#[derive(Debug, Default, Serialize)]
pub struct PlayersFilter {
    #[serde(rename = "filterActive", skip_serializing_if = "Option::is_none")]
    pub filter_active: Option<Val<bool>>,

    #[serde(rename = "filterName", skip_serializing_if = "Option::is_none")]
    pub filter_name: Option<Val<String>>,

    #[serde(rename = "filterSlotIds", skip_serializing_if = "Option::is_none")]
    pub filter_slot_ids: Option<Val<Vec<u8>>>,

    // Working injury status filters (confirmed to work with ESPN API)
    #[serde(rename = "filterInjured", skip_serializing_if = "Option::is_none")]
    pub filter_injured: Option<Val<bool>>,
    // Note: filterHealthy, filterFreeAgent, filterAvailable, etc. don't seem to work as server-side filters
    // We'll handle roster filtering client-side after getting the data
}

/// General-purpose helper: any Serialize → JSON → HeaderValue
pub trait IntoHeaderValue {
    fn to_header_value(&self) -> Result<HeaderValue>;
}

impl<T> IntoHeaderValue for T
where
    T: Serialize,
{
    fn to_header_value(&self) -> Result<HeaderValue> {
        let s = serde_json::to_string(self)?;
        Ok(HeaderValue::from_str(&s)?)
    }
}

/// Convenience constructor used by main from CLI args.
pub fn build_players_filter(
    player_names: Option<Vec<String>>,
    slots: Option<Vec<u8>>,
    include_active: Option<bool>,
    injury_status_filter: Option<&crate::cli::types::InjuryStatusFilter>,
    _roster_status_filter: Option<&crate::cli::types::RosterStatusFilter>,
) -> PlayersFilter {
    use crate::cli::types::InjuryStatusFilter;

    let mut f = PlayersFilter::default();

    if let Some(names) = player_names {
        // If only one name, use ESPN filter for efficiency
        if names.len() == 1 {
            f.filter_name = Some(Val {
                value: names[0].clone(),
            });
        }
        // If multiple names, we'll filter locally after fetching all players
        // (don't set filter_name so ESPN returns all players)
    }
    if let Some(slot_ids) = slots {
        f.filter_slot_ids = Some(Val { value: slot_ids });
    }
    if let Some(active) = include_active {
        f.filter_active = Some(Val { value: active });
    }

    // Add injury status filters (only server-side ones that actually work)
    if let Some(injury_filter) = injury_status_filter {
        match injury_filter {
            InjuryStatusFilter::Active => {
                // Use filterActive=true to get only active players
                f.filter_active = Some(Val { value: true });
            }
            InjuryStatusFilter::Injured => {
                // Use filterInjured=true to get only injured players
                f.filter_injured = Some(Val { value: true });
            }
            // For specific injury statuses (Out, Doubtful, etc.), we'll filter client-side
            // since ESPN doesn't support granular injury status filtering
            _ => {
                // Don't set any server-side filter, we'll filter client-side
            }
        }
    }

    // Roster status filters don't work server-side, so we handle them client-side
    // (roster_status_filter parameter is kept for client-side filtering)

    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_val_creation() {
        let val = Val { value: 42 };
        assert_eq!(val.value, 42);
    }

    #[test]
    fn test_players_filter_default() {
        let filter = PlayersFilter::default();
        assert!(filter.filter_active.is_none());
        assert!(filter.filter_name.is_none());
        assert!(filter.filter_slot_ids.is_none());
        assert!(filter.filter_injured.is_none());
    }

    #[test]
    fn test_build_players_filter_with_name() {
        let filter = build_players_filter(Some(vec!["Brady".to_string()]), None, None, None, None);
        assert!(filter.filter_name.is_some());
        assert_eq!(filter.filter_name.unwrap().value, "Brady");
    }

    #[test]
    fn test_build_players_filter_with_slots() {
        let filter = build_players_filter(None, Some(vec![0, 2, 4]), None, None, None);
        assert!(filter.filter_slot_ids.is_some());
        assert_eq!(filter.filter_slot_ids.unwrap().value, vec![0, 2, 4]);
    }

    #[test]
    fn test_build_players_filter_with_active() {
        let filter = build_players_filter(None, None, Some(true), None, None);
        assert!(filter.filter_active.is_some());
        assert_eq!(filter.filter_active.unwrap().value, true);
    }

    #[test]
    fn test_into_header_value() {
        let val = Val { value: "test" };
        let header_value = val.to_header_value().unwrap();
        assert_eq!(header_value.to_str().unwrap(), r#"{"value":"test"}"#);
    }

    #[test]
    fn test_players_filter_serialization() {
        let filter = build_players_filter(
            Some(vec!["Test".to_string()]),
            Some(vec![0, 2]),
            Some(true),
            None,
            None,
        );

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("\"filterName\":{\"value\":\"Test\"}"));
        assert!(json.contains("\"filterSlotIds\":{\"value\":[0,2]}"));
        assert!(json.contains("\"filterActive\":{\"value\":true}"));
    }
}
