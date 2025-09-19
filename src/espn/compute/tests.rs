//! Unit tests for scoring computation logic

use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[cfg(test)]
mod scoring_tests {
    use super::*;

    fn create_test_scoring_items() -> Vec<ScoringItem> {
        vec![
            ScoringItem {
                stat_id: 53,  // Passing yards
                points: 0.04, // 1 point per 25 yards
                points_overrides: BTreeMap::new(),
            },
            ScoringItem {
                stat_id: 1, // Passing TDs
                points: 4.0,
                points_overrides: BTreeMap::new(),
            },
            ScoringItem {
                stat_id: 20, // Interceptions
                points: -2.0,
                points_overrides: BTreeMap::new(),
            },
            ScoringItem {
                stat_id: 24, // Rushing yards
                points: 0.1, // 1 point per 10 yards
                points_overrides: {
                    let mut map = BTreeMap::new();
                    map.insert(2, 0.1); // RB slot gets 0.1 per yard
                    map.insert(0, 0.05); // QB slot gets 0.05 per yard
                    map
                },
            },
            ScoringItem {
                stat_id: 25, // Rushing TDs
                points: 6.0,
                points_overrides: BTreeMap::new(),
            },
        ]
    }

    #[test]
    fn test_build_scoring_index() {
        let items = create_test_scoring_items();
        let index = build_scoring_index(&items);

        assert_eq!(index.len(), 5);

        // Test basic scoring
        let (points, overrides) = index.get(&53).unwrap();
        assert_eq!(*points, 0.04);
        assert!(overrides.is_empty());

        // Test with overrides
        let (points, overrides) = index.get(&24).unwrap();
        assert_eq!(*points, 0.1);
        assert_eq!(overrides.get(&2), Some(&0.1));
        assert_eq!(overrides.get(&0), Some(&0.05));
    }

    #[test]
    fn test_build_scoring_index_empty() {
        let items = vec![];
        let index = build_scoring_index(&items);
        assert!(index.is_empty());
    }

    #[test]
    fn test_select_weekly_stats_found() {
        let player_data = json!({
            "id": 12345,
            "fullName": "Test Player",
            "stats": [
                {
                    "seasonId": 2023,
                    "scoringPeriodId": 1,
                    "statSourceId": 0,
                    "statSplitTypeId": 1,
                    "stats": {
                        "53": 350.0,
                        "1": 2.0,
                        "20": 1.0
                    }
                },
                {
                    "seasonId": 2023,
                    "scoringPeriodId": 2,
                    "statSourceId": 0,
                    "statSplitTypeId": 1,
                    "stats": {
                        "53": 275.0,
                        "1": 1.0
                    }
                }
            ]
        });

        let stats = select_weekly_stats(&player_data, 2023, 1, 0);
        assert!(stats.is_some());

        let stats_obj = stats.unwrap().as_object().unwrap();
        assert_eq!(stats_obj.get("53").unwrap().as_f64().unwrap(), 350.0);
        assert_eq!(stats_obj.get("1").unwrap().as_f64().unwrap(), 2.0);
        assert_eq!(stats_obj.get("20").unwrap().as_f64().unwrap(), 1.0);
    }

    #[test]
    fn test_select_weekly_stats_projected() {
        let player_data = json!({
            "stats": [
                {
                    "seasonId": 2023,
                    "scoringPeriodId": 1,
                    "statSourceId": 1, // Projected
                    "statSplitTypeId": 1,
                    "stats": {
                        "53": 300.0,
                        "1": 2.0
                    }
                }
            ]
        });

        let stats = select_weekly_stats(&player_data, 2023, 1, 1);
        assert!(stats.is_some());

        let stats_obj = stats.unwrap().as_object().unwrap();
        assert_eq!(stats_obj.get("53").unwrap().as_f64().unwrap(), 300.0);
    }

    #[test]
    fn test_select_weekly_stats_not_found() {
        let player_data = json!({
            "stats": [
                {
                    "seasonId": 2023,
                    "scoringPeriodId": 2, // Different week
                    "statSourceId": 0,
                    "statSplitTypeId": 1,
                    "stats": {}
                }
            ]
        });

        let stats = select_weekly_stats(&player_data, 2023, 1, 0);
        assert!(stats.is_none());
    }

    #[test]
    fn test_select_weekly_stats_no_stats_array() {
        let player_data = json!({
            "id": 12345,
            "fullName": "Test Player"
        });

        let stats = select_weekly_stats(&player_data, 2023, 1, 0);
        assert!(stats.is_none());
    }

    #[test]
    fn test_select_weekly_stats_wrong_split_type() {
        let player_data = json!({
            "stats": [
                {
                    "seasonId": 2023,
                    "scoringPeriodId": 1,
                    "statSourceId": 0,
                    "statSplitTypeId": 0, // Season total, not weekly
                    "stats": {}
                }
            ]
        });

        let stats = select_weekly_stats(&player_data, 2023, 1, 0);
        assert!(stats.is_none());
    }

    #[test]
    fn test_compute_points_for_week_basic() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!({
            "53": 300.0, // 300 passing yards = 300 * 0.04 = 12 points
            "1": 2.0,    // 2 passing TDs = 2 * 4 = 8 points
            "20": 1.0    // 1 INT = 1 * -2 = -2 points
        });

        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, 18.0); // 12 + 8 - 2 = 18
    }

    #[test]
    fn test_compute_points_for_week_with_overrides() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!({
            "24": 100.0, // 100 rushing yards
            "25": 1.0    // 1 rushing TD = 6 points
        });

        // Test QB slot (slot 0) - should use override 0.05 per yard
        let qb_points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(qb_points, 11.0); // 100 * 0.05 + 1 * 6 = 5 + 6 = 11

        // Test RB slot (slot 2) - should use override 0.1 per yard
        let rb_points = compute_points_for_week(&weekly_stats, 2, &scoring_index);
        assert_eq!(rb_points, 16.0); // 100 * 0.1 + 1 * 6 = 10 + 6 = 16

        // Test WR slot (slot 4) - should use base points 0.1 per yard
        let wr_points = compute_points_for_week(&weekly_stats, 4, &scoring_index);
        assert_eq!(rb_points, wr_points); // Should be same as RB since base is 0.1
    }

    #[test]
    fn test_compute_points_for_week_unknown_stats() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!({
            "999": 100.0, // Unknown stat ID
            "1": 1.0      // Known stat
        });

        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, 4.0); // Only the passing TD counts
    }

    #[test]
    fn test_compute_points_for_week_invalid_stats() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!({
            "53": "not_a_number",
            "1": 1.0
        });

        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, 4.0); // Only the valid passing TD counts
    }

    #[test]
    fn test_compute_points_for_week_non_object() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!("not an object");
        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, 0.0);
    }

    #[test]
    fn test_compute_points_for_week_empty_stats() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!({});
        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, 0.0);
    }

    #[test]
    fn test_compute_points_for_week_zero_values() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!({
            "53": 0.0,
            "1": 0.0,
            "20": 0.0
        });

        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, 0.0);
    }

    #[test]
    fn test_compute_points_for_week_negative_values() {
        let items = vec![ScoringItem {
            stat_id: 999,
            points: -1.0, // Negative points per unit
            points_overrides: BTreeMap::new(),
        }];
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!({
            "999": 5.0
        });

        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, -5.0);
    }

    #[test]
    fn test_complex_scoring_scenario() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        // QB with 325 passing yards, 3 TDs, 2 INTs, 50 rushing yards, 1 rushing TD
        let weekly_stats = json!({
            "53": 325.0, // Passing yards: 325 * 0.04 = 13 points
            "1": 3.0,    // Passing TDs: 3 * 4 = 12 points
            "20": 2.0,   // INTs: 2 * -2 = -4 points
            "24": 50.0,  // Rushing yards (QB): 50 * 0.05 = 2.5 points
            "25": 1.0    // Rushing TDs: 1 * 6 = 6 points
        });

        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, 29.5); // 13 + 12 - 4 + 2.5 + 6 = 29.5
    }

    #[test]
    fn test_compute_points_for_week_invalid_stat_ids() {
        let items = create_test_scoring_items();
        let scoring_index = build_scoring_index(&items);

        let weekly_stats = json!({
            "not_a_number": 5.0,     // Invalid stat ID - should be skipped
            "also_invalid": 10.0,    // Invalid stat ID - should be skipped
            "1": 2.0                 // Valid passing TD = 2 * 4 = 8 points
        });

        // Should skip invalid stat IDs and only count valid ones
        let points = compute_points_for_week(&weekly_stats, 0, &scoring_index);
        assert_eq!(points, 8.0); // Only the valid passing TD should count
    }
}
