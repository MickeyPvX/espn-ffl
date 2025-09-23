//! ESPN API filter utilities

use crate::Result;
use reqwest::header::HeaderValue;
use serde::Serialize;

/// Wraps ESPN-style `{ "value": ... }`
#[derive(Debug, Serialize)]
pub struct Val<T> {
    pub value: T,
}

/// Rootless filter object for `/players` endpoint.
/// Only set fields will be serialized (thanks to `skip_serializing_if`).
#[derive(Debug, Default, Serialize)]
pub struct PlayersFilter {
    #[serde(rename = "filterActive", skip_serializing_if = "Option::is_none")]
    pub filter_active: Option<Val<bool>>,

    #[serde(rename = "filterName", skip_serializing_if = "Option::is_none")]
    pub filter_name: Option<Val<String>>,

    #[serde(rename = "filterSlotIds", skip_serializing_if = "Option::is_none")]
    pub filter_slot_ids: Option<Val<Vec<u8>>>,

    /// Simple limit (server-side)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
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
    limit: Option<u32>,
    player_names: Option<Vec<String>>,
    slots: Option<Vec<u8>>,
    include_active: Option<bool>, // if you still want to set filterActive sometimes
) -> PlayersFilter {
    let mut f = PlayersFilter::default();

    if let Some(n) = limit {
        f.limit = Some(n);
    }
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
        assert!(filter.limit.is_none());
    }

    #[test]
    fn test_build_players_filter_with_limit() {
        let filter = build_players_filter(Some(100), None, None, None);
        assert_eq!(filter.limit, Some(100));
        assert!(filter.filter_name.is_none());
    }

    #[test]
    fn test_build_players_filter_with_name() {
        let filter = build_players_filter(None, Some(vec!["Brady".to_string()]), None, None);
        assert!(filter.filter_name.is_some());
        assert_eq!(filter.filter_name.unwrap().value, "Brady");
    }

    #[test]
    fn test_build_players_filter_with_slots() {
        let filter = build_players_filter(None, None, Some(vec![0, 2, 4]), None);
        assert!(filter.filter_slot_ids.is_some());
        assert_eq!(filter.filter_slot_ids.unwrap().value, vec![0, 2, 4]);
    }

    #[test]
    fn test_build_players_filter_with_active() {
        let filter = build_players_filter(None, None, None, Some(true));
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
            Some(50),
            Some(vec!["Test".to_string()]),
            Some(vec![0, 2]),
            Some(true),
        );

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("\"limit\":50"));
        assert!(json.contains("\"filterName\":{\"value\":\"Test\"}"));
        assert!(json.contains("\"filterSlotIds\":{\"value\":[0,2]}"));
        assert!(json.contains("\"filterActive\":{\"value\":true}"));
    }
}
