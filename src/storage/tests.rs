//! Unit tests for storage functionality

use super::*;
use crate::cli::types::{PlayerId, Season, Week};

fn create_test_db() -> PlayerDatabase {
    // Create in-memory database for testing
    let conn = rusqlite::Connection::open_in_memory().unwrap();

    // Enable foreign keys for testing
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    let mut db = PlayerDatabase { conn };
    db.initialize_schema().unwrap();
    db
}

fn create_test_db_with_player() -> PlayerDatabase {
    let mut db = create_test_db();

    // Insert a test player
    let player = Player {
        player_id: PlayerId::new(12345),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: Some("TEST".to_string()),
    };
    db.upsert_player(&player).unwrap();

    db
}

#[test]
fn test_database_creation() {
    let _db = create_test_db();
    // Should not panic - database creation successful
}

#[test]
fn test_upsert_player() {
    let mut db = create_test_db();

    let player = Player {
        player_id: PlayerId::new(12345),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: Some("TEST".to_string()),
    };

    // Insert player
    let result = db.upsert_player(&player);
    assert!(result.is_ok());

    // Update same player with different info
    let updated_player = Player {
        player_id: PlayerId::new(12345),
        name: "Updated Player".to_string(),
        position: "RB".to_string(),
        team: Some("NEW".to_string()),
    };

    let result = db.upsert_player(&updated_player);
    assert!(result.is_ok());
}

#[test]
fn test_upsert_weekly_stats_new() {
    let mut db = create_test_db_with_player();

    let stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12345),
        season: Season::new(2023),
        week: Week::new(1),
        projected_points: Some(15.5),
        actual_points: Some(18.2),
        created_at: 0,
        updated_at: 0,
    };

    let result = db.upsert_weekly_stats(&stats, false);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true for new insert
}

#[test]
fn test_upsert_weekly_stats_existing_no_force() {
    let mut db = create_test_db_with_player();

    let stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12345),
        season: Season::new(2023),
        week: Week::new(1),
        projected_points: Some(15.5),
        actual_points: Some(18.2),
        created_at: 0,
        updated_at: 0,
    };

    // Insert first time
    let result = db.upsert_weekly_stats(&stats, false);
    assert!(result.is_ok());
    assert!(result.unwrap());

    // Try to insert again without force - should be ignored
    let updated_stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12345),
        season: Season::new(2023),
        week: Week::new(1),
        projected_points: Some(20.0),
        actual_points: Some(25.0),
        created_at: 0,
        updated_at: 0,
    };

    let result = db.upsert_weekly_stats(&updated_stats, false);
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false for ignored insert
}

#[test]
fn test_upsert_weekly_stats_existing_with_force() {
    let mut db = create_test_db_with_player();

    let stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12345),
        season: Season::new(2023),
        week: Week::new(1),
        projected_points: Some(15.5),
        actual_points: Some(18.2),
        created_at: 0,
        updated_at: 0,
    };

    // Insert first time
    let result = db.upsert_weekly_stats(&stats, false);
    assert!(result.is_ok());

    // Force update
    let updated_stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12345),
        season: Season::new(2023),
        week: Week::new(1),
        projected_points: Some(20.0),
        actual_points: Some(25.0),
        created_at: 0,
        updated_at: 0,
    };

    let result = db.upsert_weekly_stats(&updated_stats, true);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true for forced update
}

#[test]
fn test_get_weekly_stats_existing() {
    let mut db = create_test_db_with_player();

    let stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12345),
        season: Season::new(2023),
        week: Week::new(1),
        projected_points: Some(15.5),
        actual_points: Some(18.2),
        created_at: 0,
        updated_at: 0,
    };

    db.upsert_weekly_stats(&stats, false).unwrap();

    let retrieved = db
        .get_weekly_stats(PlayerId::new(12345), Season::new(2023), Week::new(1))
        .unwrap();

    assert!(retrieved.is_some());
    let retrieved_stats = retrieved.unwrap();
    assert_eq!(retrieved_stats.player_id, PlayerId::new(12345));
    assert_eq!(retrieved_stats.season, Season::new(2023));
    assert_eq!(retrieved_stats.week, Week::new(1));
    assert_eq!(retrieved_stats.projected_points, Some(15.5));
    assert_eq!(retrieved_stats.actual_points, Some(18.2));
}

#[test]
fn test_get_weekly_stats_nonexistent() {
    let db = create_test_db();

    let result = db
        .get_weekly_stats(PlayerId::new(99999), Season::new(2023), Week::new(1))
        .unwrap();

    assert!(result.is_none());
}

#[test]
fn test_get_player_season_stats() {
    let mut db = create_test_db_with_player();

    // Insert multiple weeks for same player
    for week in 1..=5 {
        let stats = PlayerWeeklyStats {
            player_id: PlayerId::new(12345),
            season: Season::new(2023),
            week: Week::new(week),
            projected_points: Some(15.0 + week as f64),
            actual_points: Some(18.0 + week as f64),
            created_at: 0,
            updated_at: 0,
        };
        db.upsert_weekly_stats(&stats, false).unwrap();
    }

    let season_stats = db
        .get_player_season_stats(PlayerId::new(12345), Season::new(2023))
        .unwrap();

    assert_eq!(season_stats.len(), 5);

    // Should be ordered by week
    for (i, stats) in season_stats.iter().enumerate() {
        assert_eq!(stats.week, Week::new((i + 1) as u16));
    }
}

#[test]
fn test_get_projection_analysis_no_data() {
    let db = create_test_db();

    let analysis = db
        .get_projection_analysis(Season::new(2023), None, Some(10))
        .unwrap();

    assert!(analysis.is_empty());
}

#[test]
fn test_get_projection_analysis_with_data() {
    let mut db = create_test_db();

    // Insert player
    let player = Player {
        player_id: PlayerId::new(12345),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: Some("TEST".to_string()),
    };
    db.upsert_player(&player).unwrap();

    // Insert some weekly stats with projection errors
    for week in 1..=5 {
        let stats = PlayerWeeklyStats {
            player_id: PlayerId::new(12345),
            season: Season::new(2023),
            week: Week::new(week),
            projected_points: Some(20.0), // Consistently overestimated
            actual_points: Some(15.0),
            created_at: 0,
            updated_at: 0,
        };
        db.upsert_weekly_stats(&stats, false).unwrap();
    }

    let analysis = db
        .get_projection_analysis(Season::new(2023), None, Some(10))
        .unwrap();

    assert_eq!(analysis.len(), 1);
    let player_analysis = &analysis[0];
    assert_eq!(player_analysis.name, "Test Player");
    assert_eq!(player_analysis.position, "QB");
    assert_eq!(player_analysis.games_count, 5);
    assert!((player_analysis.avg_error - 5.0).abs() < 0.01); // 20.0 - 15.0 = 5.0 error
}

#[test]
fn test_estimate_week_performance_no_data() {
    let db = create_test_db();

    let projected_data = vec![(PlayerId::new(12345), 20.0), (PlayerId::new(12346), 15.0)];

    let estimates = db
        .estimate_week_performance(
            Season::new(2023),
            Week::new(5),
            &projected_data,
            Some(10),
            1.0,
        )
        .unwrap();

    assert_eq!(estimates.len(), 2);

    // Without historical data, should use ESPN projections as-is
    assert!((estimates[0].estimated_points - 20.0).abs() < 0.01);
    assert!((estimates[1].estimated_points - 15.0).abs() < 0.01);
    assert!(estimates[0].confidence < 0.5); // Low confidence without data
}

#[test]
fn test_estimate_week_performance_with_bias() {
    let mut db = create_test_db();

    // Insert player
    let player = Player {
        player_id: PlayerId::new(12345),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: Some("TEST".to_string()),
    };
    db.upsert_player(&player).unwrap();

    // Insert historical data showing consistent overestimation
    for week in 1..=4 {
        let stats = PlayerWeeklyStats {
            player_id: PlayerId::new(12345),
            season: Season::new(2023),
            week: Week::new(week),
            projected_points: Some(20.0), // ESPN consistently projects 20
            actual_points: Some(15.0),    // Player consistently scores 15
            created_at: 0,
            updated_at: 0,
        };
        db.upsert_weekly_stats(&stats, false).unwrap();
    }

    let projected_data = vec![
        (PlayerId::new(12345), 20.0), // ESPN projects 20 for week 5
    ];

    let estimates = db
        .estimate_week_performance(
            Season::new(2023),
            Week::new(5), // Estimate for week 5 based on weeks 1-4
            &projected_data,
            Some(10),
            1.0,
        )
        .unwrap();

    assert_eq!(estimates.len(), 1);
    let estimate = &estimates[0];

    // Should adjust down from 20.0 due to historical overestimation
    assert!(estimate.estimated_points < 20.0);
    assert!(estimate.estimated_points > 10.0); // But reasonable
    assert!(estimate.confidence > 0.5); // Higher confidence with good data
    assert!(estimate.reasoning.contains("overestimates"));
}

#[test]
fn test_clear_all_data() {
    let mut db = create_test_db_with_player();

    // Add some weekly stats data
    let stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12345),
        season: Season::new(2023),
        week: Week::new(1),
        projected_points: Some(15.0),
        actual_points: Some(18.0),
        created_at: 0,
        updated_at: 0,
    };
    db.upsert_weekly_stats(&stats, false).unwrap();

    // Verify data exists
    let retrieved_stats = db
        .get_weekly_stats(PlayerId::new(12345), Season::new(2023), Week::new(1))
        .unwrap();
    assert!(retrieved_stats.is_some());

    // Clear all data
    db.clear_all_data().unwrap();

    // Verify data is gone
    let retrieved_stats_after = db
        .get_weekly_stats(PlayerId::new(12345), Season::new(2023), Week::new(1))
        .unwrap();
    assert!(retrieved_stats_after.is_none());
}
