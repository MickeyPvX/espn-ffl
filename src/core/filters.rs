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

    #[test]
    fn test_build_players_filter_multiple_names() {
        // When multiple names are provided, no filter_name should be set (client-side filtering)
        let filter = build_players_filter(
            Some(vec!["Brady".to_string(), "Rodgers".to_string()]),
            None,
            None,
            None,
            None,
        );
        assert!(
            filter.filter_name.is_none(),
            "Multiple names should not set server-side filter"
        );
    }

    #[test]
    fn test_build_players_filter_injury_active() {
        use crate::cli::types::InjuryStatusFilter;

        let filter =
            build_players_filter(None, None, None, Some(&InjuryStatusFilter::Active), None);

        assert!(filter.filter_active.is_some());
        assert_eq!(filter.filter_active.unwrap().value, true);
        assert!(filter.filter_injured.is_none());
    }

    #[test]
    fn test_build_players_filter_injury_injured() {
        use crate::cli::types::InjuryStatusFilter;

        let filter =
            build_players_filter(None, None, None, Some(&InjuryStatusFilter::Injured), None);

        assert!(filter.filter_injured.is_some());
        assert_eq!(filter.filter_injured.unwrap().value, true);
        assert!(filter.filter_active.is_none());
    }

    #[test]
    fn test_build_players_filter_injury_specific_statuses() {
        use crate::cli::types::InjuryStatusFilter;

        // Test specific injury statuses that should not set server-side filters
        let specific_statuses = [
            InjuryStatusFilter::Out,
            InjuryStatusFilter::Doubtful,
            InjuryStatusFilter::Questionable,
            InjuryStatusFilter::Probable,
            InjuryStatusFilter::DayToDay,
            InjuryStatusFilter::IR,
        ];

        for status in specific_statuses {
            let filter = build_players_filter(None, None, None, Some(&status), None);

            // Should not set any server-side filters for specific statuses
            assert!(
                filter.filter_active.is_none(),
                "Specific status {:?} should not set filter_active",
                status
            );
            assert!(
                filter.filter_injured.is_none(),
                "Specific status {:?} should not set filter_injured",
                status
            );
        }
    }

    #[test]
    fn test_build_players_filter_roster_status_ignored() {
        use crate::cli::types::RosterStatusFilter;

        // Roster status filters should not affect the server-side filter
        let filter =
            build_players_filter(None, None, None, None, Some(&RosterStatusFilter::Rostered));

        // No server-side filters should be set for roster status
        assert!(filter.filter_active.is_none());
        assert!(filter.filter_injured.is_none());
        assert!(filter.filter_name.is_none());
        assert!(filter.filter_slot_ids.is_none());
    }

    #[test]
    fn test_build_players_filter_comprehensive() {
        use crate::cli::types::{InjuryStatusFilter, RosterStatusFilter};

        let filter = build_players_filter(
            Some(vec!["Test Player".to_string()]),
            Some(vec![0, 2, 4]), // QB, RB, TE
            Some(false),         // Include inactive players
            Some(&InjuryStatusFilter::Active),
            Some(&RosterStatusFilter::FA),
        );

        // Check all set filters
        assert!(filter.filter_name.is_some());
        assert_eq!(filter.filter_name.unwrap().value, "Test Player");

        assert!(filter.filter_slot_ids.is_some());
        assert_eq!(filter.filter_slot_ids.unwrap().value, vec![0, 2, 4]);

        // The injury filter should override the include_active parameter
        assert!(filter.filter_active.is_some());
        assert_eq!(filter.filter_active.unwrap().value, true);

        assert!(filter.filter_injured.is_none());
    }

    #[test]
    fn test_val_with_different_types() {
        let bool_val = Val { value: true };
        let string_val = Val {
            value: "test".to_string(),
        };
        let vec_val = Val {
            value: vec![1, 2, 3],
        };
        let int_val = Val { value: 42 };

        assert_eq!(bool_val.value, true);
        assert_eq!(string_val.value, "test");
        assert_eq!(vec_val.value, vec![1, 2, 3]);
        assert_eq!(int_val.value, 42);
    }

    #[test]
    fn test_val_serialization() {
        let val = Val {
            value: "test_value",
        };
        let json = serde_json::to_string(&val).unwrap();
        assert_eq!(json, r#"{"value":"test_value"}"#);

        let bool_val = Val { value: false };
        let bool_json = serde_json::to_string(&bool_val).unwrap();
        assert_eq!(bool_json, r#"{"value":false}"#);

        let vec_val = Val {
            value: vec![1, 2, 3],
        };
        let vec_json = serde_json::to_string(&vec_val).unwrap();
        assert_eq!(vec_json, r#"{"value":[1,2,3]}"#);
    }

    #[test]
    fn test_players_filter_field_skipping() {
        let mut filter = PlayersFilter::default();
        filter.filter_active = Some(Val { value: true });
        // Leave other fields as None

        let json = serde_json::to_string(&filter).unwrap();

        // Should only contain the active filter
        assert!(json.contains("\"filterActive\":{\"value\":true}"));
        assert!(!json.contains("\"filterName\""));
        assert!(!json.contains("\"filterSlotIds\""));
        assert!(!json.contains("\"filterInjured\""));
    }

    #[test]
    fn test_into_header_value_complex_structure() {
        let mut filter = PlayersFilter::default();
        filter.filter_name = Some(Val {
            value: "Complex Player Name".to_string(),
        });
        filter.filter_slot_ids = Some(Val {
            value: vec![0, 1, 2, 3, 4, 5],
        });
        filter.filter_active = Some(Val { value: true });
        filter.filter_injured = Some(Val { value: false });

        let header_value = filter.to_header_value().unwrap();
        let header_str = header_value.to_str().unwrap();

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(header_str).unwrap();
        assert!(parsed.is_object());

        // Verify structure
        assert!(header_str.contains("\"filterName\""));
        assert!(header_str.contains("\"filterSlotIds\""));
        assert!(header_str.contains("\"filterActive\""));
        assert!(header_str.contains("\"filterInjured\""));
    }

    #[test]
    fn test_into_header_value_empty_filter() {
        let filter = PlayersFilter::default();
        let header_value = filter.to_header_value().unwrap();
        let header_str = header_value.to_str().unwrap();

        // Should be an empty JSON object
        assert_eq!(header_str, "{}");
    }

    #[test]
    fn test_build_players_filter_edge_cases() {
        // Test with empty vectors
        let filter = build_players_filter(
            Some(vec![]), // Empty names
            Some(vec![]), // Empty slots
            None,
            None,
            None,
        );

        // Empty names should not set filter_name (will be filtered client-side)
        assert!(filter.filter_name.is_none());

        // Empty slots should still set filter_slot_ids (ESPN might handle this)
        assert!(filter.filter_slot_ids.is_some());
        assert_eq!(filter.filter_slot_ids.unwrap().value, Vec::<u8>::new());
    }

    #[test]
    fn test_build_players_filter_conflicting_parameters() {
        use crate::cli::types::InjuryStatusFilter;

        // Test when both include_active and injury filter are provided
        let filter = build_players_filter(
            None,
            None,
            Some(false),                       // Want inactive players
            Some(&InjuryStatusFilter::Active), // But also want active players
            None,
        );

        // Injury filter should take precedence
        assert!(filter.filter_active.is_some());
        assert_eq!(filter.filter_active.unwrap().value, true);
    }

    #[test]
    fn test_players_filter_field_independence() {
        // Test that each field can be set independently
        let mut filter = PlayersFilter::default();

        // Set each field one by one and verify others remain None
        filter.filter_active = Some(Val { value: true });
        assert!(filter.filter_name.is_none());
        assert!(filter.filter_slot_ids.is_none());
        assert!(filter.filter_injured.is_none());

        filter.filter_name = Some(Val {
            value: "Test".to_string(),
        });
        assert!(filter.filter_active.is_some());
        assert!(filter.filter_slot_ids.is_none());
        assert!(filter.filter_injured.is_none());

        filter.filter_slot_ids = Some(Val { value: vec![0] });
        assert!(filter.filter_active.is_some());
        assert!(filter.filter_name.is_some());
        assert!(filter.filter_injured.is_none());

        filter.filter_injured = Some(Val { value: false });
        assert!(filter.filter_active.is_some());
        assert!(filter.filter_name.is_some());
        assert!(filter.filter_slot_ids.is_some());
    }

    #[test]
    fn test_slot_ids_various_positions() {
        // Test with realistic position slot IDs
        let qb_rb_wr_slots = vec![0, 2, 3]; // QB, RB, WR
        let all_skill_slots = vec![0, 2, 3, 4]; // QB, RB, WR, TE
        let defense_kicker_slots = vec![5, 16]; // K, D/ST

        let filter1 = build_players_filter(None, Some(qb_rb_wr_slots.clone()), None, None, None);
        assert_eq!(filter1.filter_slot_ids.unwrap().value, qb_rb_wr_slots);

        let filter2 = build_players_filter(None, Some(all_skill_slots.clone()), None, None, None);
        assert_eq!(filter2.filter_slot_ids.unwrap().value, all_skill_slots);

        let filter3 =
            build_players_filter(None, Some(defense_kicker_slots.clone()), None, None, None);
        assert_eq!(filter3.filter_slot_ids.unwrap().value, defense_kicker_slots);
    }

    #[test]
    fn test_name_filter_special_characters() {
        // Test with names containing special characters
        let special_names = vec![
            "D'Angelo Russell".to_string(),
            "T.J. Watt".to_string(),
            "Geno Smith".to_string(),
        ];

        // Single name with special characters should work
        let filter =
            build_players_filter(Some(vec![special_names[0].clone()]), None, None, None, None);
        assert!(filter.filter_name.is_some());
        assert_eq!(filter.filter_name.unwrap().value, "D'Angelo Russell");

        // Multiple names should not set server-side filter
        let filter = build_players_filter(Some(special_names), None, None, None, None);
        assert!(filter.filter_name.is_none());
    }
}
