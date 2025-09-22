//! Unit tests for ESPN types and data structures

use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[cfg(test)]
mod types_tests {
    use super::*;

    #[test]
    fn test_scoring_item_deserialization() {
        let json = json!({
            "statId": 53,
            "points": 0.04,
            "pointsOverrides": {
                "0": 0.02,
                "2": 0.05
            }
        });

        let item: ScoringItem = serde_json::from_value(json).unwrap();
        assert_eq!(item.stat_id, 53);
        assert_eq!(item.points, 0.04);
        assert_eq!(item.points_overrides.get(&0), Some(&0.02));
        assert_eq!(item.points_overrides.get(&2), Some(&0.05));
    }

    #[test]
    fn test_scoring_item_deserialization_no_overrides() {
        let json = json!({
            "statId": 1,
            "points": 4.0
        });

        let item: ScoringItem = serde_json::from_value(json).unwrap();
        assert_eq!(item.stat_id, 1);
        assert_eq!(item.points, 4.0);
        assert!(item.points_overrides.is_empty());
    }

    #[test]
    fn test_scoring_item_serialization() {
        let mut overrides = BTreeMap::new();
        overrides.insert(0, 0.05);
        overrides.insert(2, 0.1);

        let item = ScoringItem {
            stat_id: 24,
            points: 0.1,
            points_overrides: overrides,
        };

        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["statId"], 24);
        assert_eq!(json["points"], 0.1);
        assert_eq!(json["pointsOverrides"]["0"], 0.05);
        assert_eq!(json["pointsOverrides"]["2"], 0.1);
    }

    #[test]
    fn test_scoring_settings_deserialization() {
        let json = json!({
            "scoringItems": [
                {
                    "statId": 53,
                    "points": 0.04,
                    "pointsOverrides": {}
                },
                {
                    "statId": 1,
                    "points": 4.0,
                    "pointsOverrides": {
                        "0": 6.0
                    }
                }
            ]
        });

        let settings: ScoringSettings = serde_json::from_value(json).unwrap();
        assert_eq!(settings.scoring_items.len(), 2);
        assert_eq!(settings.scoring_items[0].stat_id, 53);
        assert_eq!(settings.scoring_items[1].stat_id, 1);
        assert_eq!(
            settings.scoring_items[1].points_overrides.get(&0),
            Some(&6.0)
        );
    }

    #[test]
    fn test_league_settings_deserialization() {
        let json = json!({
            "scoringSettings": {
                "scoringItems": [
                    {
                        "statId": 20,
                        "points": -2.0,
                        "pointsOverrides": {}
                    }
                ]
            }
        });

        let settings: LeagueSettings = serde_json::from_value(json).unwrap();
        assert_eq!(settings.scoring_settings.scoring_items.len(), 1);
        assert_eq!(settings.scoring_settings.scoring_items[0].stat_id, 20);
        assert_eq!(settings.scoring_settings.scoring_items[0].points, -2.0);
    }

    #[test]
    fn test_league_envelope_deserialization() {
        let json = json!({
            "settings": {
                "scoringSettings": {
                    "scoringItems": [
                        {
                            "statId": 25,
                            "points": 6.0,
                            "pointsOverrides": {}
                        }
                    ]
                }
            }
        });

        let envelope: LeagueEnvelope = serde_json::from_value(json).unwrap();
        assert_eq!(envelope.settings.scoring_settings.scoring_items.len(), 1);
        assert_eq!(
            envelope.settings.scoring_settings.scoring_items[0].stat_id,
            25
        );
    }

    #[test]
    fn test_player_deserialization() {
        let json = json!({
            "id": 123456,
            "fullName": "Tom Brady",
            "defaultPositionId": 0,
            "stats": [
                {
                    "seasonId": 2023,
                    "scoringPeriodId": 1,
                    "statSourceId": 0,
                    "statSplitTypeId": 1,
                    "stats": {
                        "53": 350.0,
                        "1": 2.0
                    }
                }
            ]
        });

        let player: Player = serde_json::from_value(json).unwrap();
        assert_eq!(player.id.as_u64(), 123456);
        assert_eq!(player.full_name, Some("Tom Brady".to_string()));
        assert_eq!(player.default_position_id, 0);
        assert_eq!(player.stats.len(), 1);
    }

    #[test]
    fn test_player_deserialization_no_stats() {
        let json = json!({
            "id": 789012,
            "fullName": "Unknown Player",
            "defaultPositionId": 2
        });

        let player: Player = serde_json::from_value(json).unwrap();
        assert_eq!(player.id.as_u64(), 789012);
        assert_eq!(player.full_name, Some("Unknown Player".to_string()));
        assert_eq!(player.default_position_id, 2);
        assert!(player.stats.is_empty());
    }

    #[test]
    fn test_player_stats_deserialization() {
        let json = json!({
            "seasonId": 2023,
            "scoringPeriodId": 15,
            "statSourceId": 1,
            "statSplitTypeId": 1,
            "stats": {
                "53": 275.5,
                "1": 1.0,
                "20": 0.0,
                "24": 25.0
            }
        });

        let stats: PlayerStats = serde_json::from_value(json).unwrap();
        assert_eq!(stats.season_id.as_u16(), 2023);
        assert_eq!(stats.scoring_period_id.as_u16(), 15);
        assert_eq!(stats.stat_source_id, 1);
        assert_eq!(stats.stat_split_type_id, 1);
        assert_eq!(stats.stats.get("53"), Some(&275.5));
        assert_eq!(stats.stats.get("1"), Some(&1.0));
        assert_eq!(stats.stats.get("20"), Some(&0.0));
        assert_eq!(stats.stats.get("24"), Some(&25.0));
    }

    #[test]
    fn test_player_stats_empty_stats() {
        let json = json!({
            "seasonId": 2023,
            "scoringPeriodId": 1,
            "statSourceId": 0,
            "statSplitTypeId": 1
        });

        let stats: PlayerStats = serde_json::from_value(json).unwrap();
        assert_eq!(stats.season_id.as_u16(), 2023);
        assert!(stats.stats.is_empty());
    }

    #[test]
    fn test_player_points_serialization() {
        let player_points = PlayerPoints {
            id: PlayerId::new(456789),
            name: "Patrick Mahomes".to_string(),
            position: "QB".to_string(),
            week: Week::new(8),
            projected: true,
            points: 28.75,
        };

        let json = serde_json::to_value(&player_points).unwrap();
        assert_eq!(json["id"], 456789);
        assert_eq!(json["name"], "Patrick Mahomes");
        assert_eq!(json["week"], 8);
        assert_eq!(json["projected"], true);
        assert_eq!(json["points"], 28.75);
    }

    #[test]
    fn test_de_str_key_map_u8_f64() {
        // Test the custom deserializer for points overrides
        let json = json!({
            "0": 1.5,
            "2": 2.0,
            "16": 0.5
        });

        let map: BTreeMap<String, f64> = serde_json::from_value(json).unwrap();

        // Convert using our deserializer logic
        let converted: BTreeMap<u8, f64> = map
            .into_iter()
            .map(|(k, v)| (k.parse::<u8>().unwrap(), v))
            .collect();

        assert_eq!(converted.get(&0), Some(&1.5));
        assert_eq!(converted.get(&2), Some(&2.0));
        assert_eq!(converted.get(&16), Some(&0.5));
    }

    #[test]
    fn test_scoring_item_with_invalid_override_key() {
        // Test that invalid keys in pointsOverrides are handled
        let json = json!({
            "statId": 53,
            "points": 0.04,
            "pointsOverrides": {
                "invalid": 0.02,
                "2": 0.05
            }
        });

        // This should fail during deserialization due to invalid key
        let result = serde_json::from_value::<ScoringItem>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_player_data() {
        let json = json!({
            "id": 987654,
            "fullName": "Aaron Rodgers",
            "defaultPositionId": 0,
            "stats": [
                {
                    "seasonId": 2023,
                    "scoringPeriodId": 1,
                    "statSourceId": 0,
                    "statSplitTypeId": 1,
                    "stats": {
                        "53": 325.0,
                        "1": 3.0,
                        "20": 1.0,
                        "24": 15.0
                    }
                },
                {
                    "seasonId": 2023,
                    "scoringPeriodId": 1,
                    "statSourceId": 1,
                    "statSplitTypeId": 1,
                    "stats": {
                        "53": 300.0,
                        "1": 2.0,
                        "20": 0.0,
                        "24": 10.0
                    }
                }
            ]
        });

        let player: Player = serde_json::from_value(json).unwrap();
        assert_eq!(player.id.as_u64(), 987654);
        assert_eq!(player.full_name, Some("Aaron Rodgers".to_string()));
        assert_eq!(player.stats.len(), 2);

        // First stat entry (actual)
        assert_eq!(player.stats[0].stat_source_id, 0);
        assert_eq!(player.stats[0].stats.get("53"), Some(&325.0));

        // Second stat entry (projected)
        assert_eq!(player.stats[1].stat_source_id, 1);
        assert_eq!(player.stats[1].stats.get("53"), Some(&300.0));
    }

    #[test]
    fn test_roundtrip_serialization() {
        // Test that we can serialize and deserialize without data loss
        let original = LeagueSettings {
            scoring_settings: ScoringSettings {
                scoring_items: vec![
                    ScoringItem {
                        stat_id: 53,
                        points: 0.04,
                        points_overrides: {
                            let mut map = BTreeMap::new();
                            map.insert(0, 0.02);
                            map.insert(2, 0.06);
                            map
                        },
                    },
                    ScoringItem {
                        stat_id: 1,
                        points: 4.0,
                        points_overrides: BTreeMap::new(),
                    },
                ],
            },
        };

        let json = serde_json::to_value(&original).unwrap();
        let deserialized: LeagueSettings = serde_json::from_value(json).unwrap();

        assert_eq!(
            original.scoring_settings.scoring_items.len(),
            deserialized.scoring_settings.scoring_items.len()
        );
        assert_eq!(
            original.scoring_settings.scoring_items[0].stat_id,
            deserialized.scoring_settings.scoring_items[0].stat_id
        );
        assert_eq!(
            original.scoring_settings.scoring_items[0].points_overrides,
            deserialized.scoring_settings.scoring_items[0].points_overrides
        );
    }

    #[test]
    fn test_de_str_key_map_u8_f64_with_invalid_keys() {
        // Test the deserializer with invalid key that can't be parsed as u8
        let json = json!({
            "invalid_key": 5.0,
            "also_invalid": 10.0,
            "42": 15.0  // This one should work
        });

        // This should fail because the keys can't be parsed as u8
        let result: Result<BTreeMap<u8, f64>, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }
}
