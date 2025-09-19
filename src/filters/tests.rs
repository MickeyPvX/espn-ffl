//! Unit tests for filter functionality

use super::*;
use crate::cli_types::Position;

#[cfg(test)]
mod filter_tests {
    use super::*;

    #[test]
    fn test_val_serialization() {
        let val = Val { value: 42 };
        let json = serde_json::to_string(&val).unwrap();
        assert_eq!(json, r#"{"value":42}"#);

        let val_str = Val {
            value: "test".to_string(),
        };
        let json = serde_json::to_string(&val_str).unwrap();
        assert_eq!(json, r#"{"value":"test"}"#);
    }

    #[test]
    fn test_val_bool() {
        let val = Val { value: true };
        let json = serde_json::to_string(&val).unwrap();
        assert_eq!(json, r#"{"value":true}"#);
    }

    #[test]
    fn test_val_array() {
        let val = Val {
            value: vec![1, 2, 3],
        };
        let json = serde_json::to_string(&val).unwrap();
        assert_eq!(json, r#"{"value":[1,2,3]}"#);
    }

    #[test]
    fn test_players_filter_empty() {
        let filter = PlayersFilter::default();
        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_players_filter_with_limit() {
        let filter = PlayersFilter {
            limit: Some(50),
            ..PlayersFilter::default()
        };
        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(json, r#"{"limit":50}"#);
    }

    #[test]
    fn test_players_filter_with_name() {
        let filter = PlayersFilter {
            filter_name: Some(Val {
                value: "Brady".to_string(),
            }),
            ..PlayersFilter::default()
        };
        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(json, r#"{"filterName":{"value":"Brady"}}"#);
    }

    #[test]
    fn test_players_filter_with_slots() {
        let filter = PlayersFilter {
            filter_slot_ids: Some(Val {
                value: vec![0, 2, 4],
            }),
            ..PlayersFilter::default()
        };
        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(json, r#"{"filterSlotIds":{"value":[0,2,4]}}"#);
    }

    #[test]
    fn test_players_filter_with_active() {
        let filter = PlayersFilter {
            filter_active: Some(Val { value: true }),
            ..PlayersFilter::default()
        };
        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(json, r#"{"filterActive":{"value":true}}"#);
    }

    #[test]
    fn test_players_filter_all_fields() {
        let filter = PlayersFilter {
            filter_active: Some(Val { value: false }),
            filter_name: Some(Val {
                value: "Mahomes".to_string(),
            }),
            filter_slot_ids: Some(Val { value: vec![0] }),
            limit: Some(10),
        };
        let json = serde_json::to_string(&filter).unwrap();

        // JSON object order is not guaranteed, so we check for presence of each field
        assert!(json.contains(r#""filterActive":{"value":false}"#));
        assert!(json.contains(r#""filterName":{"value":"Mahomes"}"#));
        assert!(json.contains(r#""filterSlotIds":{"value":[0]}"#));
        assert!(json.contains(r#""limit":10"#));
    }

    #[test]
    fn test_into_header_value_trait() {
        let val = Val { value: "test" };
        let header_value = val.into_header_value().unwrap();
        assert_eq!(header_value.to_str().unwrap(), r#"{"value":"test"}"#);
    }

    #[test]
    fn test_into_header_value_complex() {
        let filter = PlayersFilter {
            limit: Some(25),
            filter_name: Some(Val {
                value: "Player".to_string(),
            }),
            ..PlayersFilter::default()
        };
        let header_value = filter.into_header_value().unwrap();
        let header_str = header_value.to_str().unwrap();

        // Should be valid JSON
        let _: serde_json::Value = serde_json::from_str(header_str).unwrap();
        assert!(header_str.contains("25"));
        assert!(header_str.contains("Player"));
    }

    #[test]
    fn test_build_players_filter_empty() {
        let filter = build_players_filter(None, None, None, None);

        assert!(filter.limit.is_none());
        assert!(filter.filter_name.is_none());
        assert!(filter.filter_slot_ids.is_none());
        assert!(filter.filter_active.is_none());
    }

    #[test]
    fn test_build_players_filter_with_limit() {
        let filter = build_players_filter(Some(100), None, None, None);

        assert_eq!(filter.limit, Some(100));
        assert!(filter.filter_name.is_none());
        assert!(filter.filter_slot_ids.is_none());
        assert!(filter.filter_active.is_none());
    }

    #[test]
    fn test_build_players_filter_with_name() {
        let filter = build_players_filter(None, Some("Wilson".to_string()), None, None);

        assert!(filter.limit.is_none());
        assert_eq!(filter.filter_name.as_ref().unwrap().value, "Wilson");
        assert!(filter.filter_slot_ids.is_none());
        assert!(filter.filter_active.is_none());
    }

    #[test]
    fn test_build_players_filter_with_slots() {
        let slots = vec![0, 2, 4]; // QB, RB, WR
        let filter = build_players_filter(None, None, Some(slots.clone()), None);

        assert!(filter.limit.is_none());
        assert!(filter.filter_name.is_none());
        assert_eq!(filter.filter_slot_ids.as_ref().unwrap().value, slots);
        assert!(filter.filter_active.is_none());
    }

    #[test]
    fn test_build_players_filter_with_active() {
        let filter = build_players_filter(None, None, None, Some(true));

        assert!(filter.limit.is_none());
        assert!(filter.filter_name.is_none());
        assert!(filter.filter_slot_ids.is_none());
        assert_eq!(filter.filter_active.as_ref().unwrap().value, true);
    }

    #[test]
    fn test_build_players_filter_all_parameters() {
        let slots = vec![0, 6]; // QB, TE
        let filter = build_players_filter(
            Some(50),
            Some("Kelce".to_string()),
            Some(slots.clone()),
            Some(false),
        );

        assert_eq!(filter.limit, Some(50));
        assert_eq!(filter.filter_name.as_ref().unwrap().value, "Kelce");
        assert_eq!(filter.filter_slot_ids.as_ref().unwrap().value, slots);
        assert_eq!(filter.filter_active.as_ref().unwrap().value, false);
    }

    #[test]
    fn test_build_players_filter_zero_limit() {
        let filter = build_players_filter(Some(0), None, None, None);
        assert_eq!(filter.limit, Some(0));
    }

    #[test]
    fn test_build_players_filter_empty_name() {
        let filter = build_players_filter(None, Some("".to_string()), None, None);
        assert_eq!(filter.filter_name.as_ref().unwrap().value, "");
    }

    #[test]
    fn test_build_players_filter_empty_slots() {
        let filter = build_players_filter(None, None, Some(vec![]), None);
        assert_eq!(
            filter.filter_slot_ids.as_ref().unwrap().value,
            Vec::<u8>::new()
        );
    }

    #[test]
    fn test_real_world_scenario() {
        // Simulate a real query: "Get top 20 QBs and RBs with 'Josh' in their name"
        let positions = vec![Position::QB, Position::RB];
        let slots: Vec<u8> = positions.into_iter().map(u8::from).collect();

        let filter =
            build_players_filter(Some(20), Some("Josh".to_string()), Some(slots), Some(true));

        // Convert to header value to ensure it works end-to-end
        let header_value = filter.into_header_value().unwrap();
        let header_str = header_value.to_str().unwrap();

        // Should be valid JSON containing our parameters
        let parsed: serde_json::Value = serde_json::from_str(header_str).unwrap();
        assert_eq!(parsed["limit"], 20);
        assert_eq!(parsed["filterName"]["value"], "Josh");
        assert_eq!(parsed["filterActive"]["value"], true);

        let slot_ids = parsed["filterSlotIds"]["value"].as_array().unwrap();
        assert!(slot_ids.contains(&serde_json::Value::from(0))); // QB
        assert!(slot_ids.contains(&serde_json::Value::from(2))); // RB
    }

    #[test]
    fn test_filter_serialization_deterministic() {
        // Test that serialization is consistent
        let filter = PlayersFilter {
            limit: Some(10),
            filter_name: Some(Val {
                value: "Test".to_string(),
            }),
            ..PlayersFilter::default()
        };

        let json1 = serde_json::to_string(&filter).unwrap();
        let json2 = serde_json::to_string(&filter).unwrap();
        assert_eq!(json1, json2);
    }
}
