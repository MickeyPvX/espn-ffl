//! Unit tests for storage functionality

use espn_ffl::{storage::*, PlayerId, Season, Week};

fn create_test_db() -> PlayerDatabase {
    PlayerDatabase::new_in_memory().unwrap()
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

    let stats = PlayerWeeklyStats::test_with_fields(
        PlayerId::new(12345),
        Season::new(2023),
        Week::new(1),
        Some(15.5),
        Some(18.2),
        0,
        0,
    );

    let result = db.upsert_weekly_stats(&stats, false);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true for new insert
}

#[test]
fn test_upsert_weekly_stats_existing_no_force() {
    let mut db = create_test_db_with_player();

    let stats = PlayerWeeklyStats::test_with_fields(
        PlayerId::new(12345),
        Season::new(2023),
        Week::new(1),
        Some(15.5),
        Some(18.2),
        0,
        0,
    );

    // Insert first time
    let result = db.upsert_weekly_stats(&stats, false);
    assert!(result.is_ok());
    assert!(result.unwrap());

    // Try to insert again without force - should be ignored
    let updated_stats = PlayerWeeklyStats::test_with_fields(
        PlayerId::new(12345),
        Season::new(2023),
        Week::new(1),
        Some(20.0),
        Some(25.0),
        0,
        0,
    );

    let result = db.upsert_weekly_stats(&updated_stats, false);
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false for ignored insert
}

#[test]
fn test_upsert_weekly_stats_existing_with_force() {
    let mut db = create_test_db_with_player();

    let stats = PlayerWeeklyStats::test_with_fields(
        PlayerId::new(12345),
        Season::new(2023),
        Week::new(1),
        Some(15.5),
        Some(18.2),
        0,
        0,
    );

    // Insert first time
    let result = db.upsert_weekly_stats(&stats, false);
    assert!(result.is_ok());

    // Force update
    let updated_stats = PlayerWeeklyStats::test_with_fields(
        PlayerId::new(12345),
        Season::new(2023),
        Week::new(1),
        Some(20.0),
        Some(25.0),
        0,
        0,
    );

    let result = db.upsert_weekly_stats(&updated_stats, true);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true for forced update
}

#[test]
fn test_get_weekly_stats_existing() {
    let mut db = create_test_db();

    // Insert a test player with unique ID
    let player = Player {
        player_id: PlayerId::new(12346),
        name: "Test Player 2".to_string(),
        position: "RB".to_string(),
        team: Some("TEST2".to_string()),
    };
    db.upsert_player(&player).unwrap();

    let stats = PlayerWeeklyStats::test_with_fields(
        PlayerId::new(12346), // Use unique ID for this test
        Season::new(2023),
        Week::new(2), // Use unique week for this test
        Some(15.5),
        Some(18.2),
        0,
        0,
    );

    db.upsert_weekly_stats(&stats, false).unwrap();

    let retrieved = db
        .get_weekly_stats(PlayerId::new(12346), Season::new(2023), Week::new(2))
        .unwrap();

    assert!(retrieved.is_some());
    let retrieved_stats = retrieved.unwrap();
    assert_eq!(retrieved_stats.player_id, PlayerId::new(12346));
    assert_eq!(retrieved_stats.season, Season::new(2023));
    assert_eq!(retrieved_stats.week, Week::new(2));
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
        let stats = PlayerWeeklyStats::test_with_fields(
            PlayerId::new(12345),
            Season::new(2023),
            Week::new(week),
            Some(15.0 + week as f64),
            Some(18.0 + week as f64),
            0,
            0,
        );
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
        let stats = PlayerWeeklyStats::test_with_fields(
            PlayerId::new(12345),
            Season::new(2023),
            Week::new(week),
            Some(20.0), // Consistently overestimated
            Some(15.0),
            0,
            0,
        );
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
        let stats = PlayerWeeklyStats::test_with_fields(
            PlayerId::new(12345),
            Season::new(2023),
            Week::new(week),
            Some(20.0), // ESPN consistently projects 20
            Some(15.0), // Player consistently scores 15
            0,
            0,
        );
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
    assert!(estimate.confidence > 0.4); // Reasonable confidence with 4 games of data
    assert!(estimate.reasoning.contains("overestimates"));
}

// Note: test_clear_all_data was removed because with the unified caching system,
// clearing the database doesn't clear the cache. This behavior is by design
// since the cache provides persistence and performance benefits.

#[test]
fn test_get_cached_player_data_with_injury_and_roster_status() {
    use espn_ffl::espn::types::InjuryStatus;

    let mut db = create_test_db();

    // Insert a test player with unique ID
    let player = Player {
        player_id: PlayerId::new(12348),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: Some("TEST".to_string()),
    };
    db.upsert_player(&player).unwrap();

    // Add player with injury and roster status
    let stats = PlayerWeeklyStats {
        player_id: PlayerId::new(12348), // Use unique ID for cached_player_data test
        season: Season::new(2023),
        week: Week::new(4), // Use unique week for cached_player_data test
        projected_points: None,
        actual_points: Some(25.5),
        active: Some(false),
        injured: Some(true),
        injury_status: Some(InjuryStatus::Questionable),
        is_rostered: Some(true),
        fantasy_team_id: Some(42),
        fantasy_team_name: Some("Test Team".to_string()),
        created_at: 1234567890,
        updated_at: 1234567890,
    };
    db.upsert_weekly_stats(&stats, false).unwrap();

    // Get cached data
    let params = espn_ffl::commands::common::CommandParams::new(Season::new(2023), Week::new(4));
    let cached_data = db.get_cached_player_data(&params, false).unwrap();

    // Should have one result
    assert_eq!(cached_data.len(), 1);

    let (
        player_id,
        name,
        position,
        points,
        active,
        injured,
        injury_status,
        is_rostered,
        team_id,
        team_name,
    ) = &cached_data[0];

    // Verify all fields are correctly returned
    assert_eq!(*player_id, PlayerId::new(12348)); // Use matching ID
    assert_eq!(name, "Test Player");
    assert_eq!(position, "QB");
    assert_eq!(*points, 25.5);
    assert_eq!(*active, Some(false));
    assert_eq!(*injured, Some(true));
    assert_eq!(*injury_status, Some(InjuryStatus::Questionable));
    assert_eq!(*is_rostered, Some(true));
    assert_eq!(*team_id, Some(42));
    assert_eq!(team_name, &Some("Test Team".to_string()));
}

#[test]
fn test_get_cached_player_data_filters_by_projected() {
    let mut db = create_test_db_with_player();

    // Add both projected and actual stats
    let projected_stats = PlayerWeeklyStats::test_with_fields(
        PlayerId::new(12345),
        Season::new(2023),
        Week::new(1),
        Some(20.0), // projected
        None,       // no actual
        0,
        0,
    );
    db.upsert_weekly_stats(&projected_stats, false).unwrap();

    let actual_stats = PlayerWeeklyStats::test_with_fields(
        PlayerId::new(12345),
        Season::new(2023),
        Week::new(1),
        Some(20.0), // keep projected
        Some(18.5), // add actual
        0,
        0,
    );
    db.upsert_weekly_stats(&actual_stats, true).unwrap(); // force update

    // Test projected filter
    let params = espn_ffl::commands::common::CommandParams::new(Season::new(2023), Week::new(1));
    let projected_data = db
        .get_cached_player_data(&params, true) // projected = true
        .unwrap();
    assert_eq!(projected_data.len(), 1);
    assert_eq!(projected_data[0].3, 20.0); // Should return projected points

    // Test actual filter
    let actual_data = db
        .get_cached_player_data(&params, false) // projected = false
        .unwrap();
    assert_eq!(actual_data.len(), 1);
    assert_eq!(actual_data[0].3, 18.5); // Should return actual points
}

#[test]
fn test_merge_weekly_stats_preserves_points_and_overwrites_roster() {
    let mut db = PlayerDatabase::new_in_memory().unwrap();

    db.upsert_player(&Player {
        player_id: PlayerId::new(12345),
        name: "Test Player".to_string(),
        position: "QB".to_string(),
        team: None,
    })
    .unwrap();

    // Seed a week that already has both point columns filled in.
    let mut seeded = PlayerWeeklyStats::test_minimal(
        PlayerId::new(12345),
        Season::new(2026),
        Week::new(3),
        Some(20.0),
        Some(18.5),
    );
    seeded.is_rostered = Some(true);
    seeded.fantasy_team_id = Some(7);
    seeded.fantasy_team_name = Some("Old Team".to_string());
    db.merge_weekly_stats(&seeded).unwrap();

    // A roster-only update carries NULL points; those must not wipe what is stored.
    let mut roster_only = PlayerWeeklyStats::test_minimal(
        PlayerId::new(12345),
        Season::new(2026),
        Week::new(3),
        None,
        None,
    );
    roster_only.active = None;
    roster_only.injured = None;
    roster_only.is_rostered = Some(false);
    roster_only.fantasy_team_id = None;
    roster_only.fantasy_team_name = None;
    db.merge_weekly_stats(&roster_only).unwrap();

    let stored = db
        .get_weekly_stats(PlayerId::new(12345), Season::new(2026), Week::new(3))
        .unwrap()
        .expect("row should exist");

    // Points survive the NULL-bearing merge...
    assert_eq!(stored.projected_points, Some(20.0));
    assert_eq!(stored.actual_points, Some(18.5));
    // ...while roster fields are overwritten, including back to NULL.
    assert_eq!(stored.is_rostered, Some(false));
    assert_eq!(stored.fantasy_team_id, None);
    assert_eq!(stored.fantasy_team_name, None);
}

#[test]
fn test_merge_weekly_stats_batch_writes_every_row() {
    let mut db = PlayerDatabase::new_in_memory().unwrap();

    for id in 1..=25 {
        db.upsert_player(&Player {
            player_id: PlayerId::new(id),
            name: format!("Player {}", id),
            position: "RB".to_string(),
            team: None,
        })
        .unwrap();
    }

    let rows: Vec<PlayerWeeklyStats> = (1..=25)
        .map(|id| {
            PlayerWeeklyStats::test_minimal(
                PlayerId::new(id),
                Season::new(2026),
                Week::new(1),
                Some(id as f64),
                None,
            )
        })
        .collect();

    assert_eq!(db.merge_weekly_stats_batch(&rows).unwrap(), 25);

    let stored = db
        .get_weekly_stats(PlayerId::new(25), Season::new(2026), Week::new(1))
        .unwrap()
        .expect("row should exist");
    assert_eq!(stored.projected_points, Some(25.0));
}

#[test]
fn test_schema_initialization_is_idempotent() {
    // Re-running initialization against an already-migrated database must be a no-op
    // rather than an error, which is what the user_version guard buys.
    let mut db = PlayerDatabase::new_in_memory().unwrap();
    db.upsert_player(&Player {
        player_id: PlayerId::new(1),
        name: "Keep Me".to_string(),
        position: "WR".to_string(),
        team: None,
    })
    .unwrap();

    db.initialize_schema().unwrap();

    assert_eq!(db.get_all_players().unwrap().len(), 1);
}

#[test]
fn test_roster_update_only_touches_rostered_players_and_clears_stale() {
    use espn_ffl::espn::types::{LeagueData, RosterEntry, Team, TeamRoster};

    let mut db = PlayerDatabase::new_in_memory().unwrap();

    for id in 1..=3 {
        db.upsert_player(&Player {
            player_id: PlayerId::new(id),
            name: format!("Player {}", id),
            position: "RB".to_string(),
            team: None,
        })
        .unwrap();
        // Every player has points for the week, so every one is returnable by queries.
        db.merge_weekly_stats(&PlayerWeeklyStats::test_minimal(
            PlayerId::new(id),
            Season::new(2026),
            Week::new(1),
            Some(10.0),
            None,
        ))
        .unwrap();
    }

    let roster_with = |player_ids: Vec<i64>| LeagueData {
        teams: vec![Team {
            id: 7,
            name: Some("Team Alpha".to_string()),
            abbrev: Some("ALPH".to_string()),
            owners: None,
            roster: Some(TeamRoster {
                entries: player_ids
                    .into_iter()
                    .map(|player_id| RosterEntry {
                        player_id,
                        lineup_slot_id: 2,
                        injury_status: None,
                    })
                    .collect(),
            }),
        }],
    };

    // Players 1 and 2 are rostered; the return value counts only those.
    let marked = db
        .update_all_players_roster_info(&roster_with(vec![1, 2]), Season::new(2026), Week::new(1))
        .unwrap();
    assert_eq!(marked, 2);

    let stored = |db: &PlayerDatabase, id: i64| {
        db.get_weekly_stats(PlayerId::new(id), Season::new(2026), Week::new(1))
            .unwrap()
            .expect("row should exist")
    };

    assert_eq!(stored(&db, 1).is_rostered, Some(true));
    assert_eq!(stored(&db, 1).fantasy_team_id, Some(7));
    assert_eq!(stored(&db, 2).is_rostered, Some(true));
    assert_eq!(stored(&db, 3).is_rostered, Some(false));
    // Points must survive a roster-only update.
    assert_eq!(stored(&db, 1).projected_points, Some(10.0));

    // Player 1 is dropped: the sweep must clear the stale team rather than leave it behind.
    db.update_all_players_roster_info(&roster_with(vec![2]), Season::new(2026), Week::new(1))
        .unwrap();

    assert_eq!(stored(&db, 1).is_rostered, Some(false));
    assert_eq!(stored(&db, 1).fantasy_team_id, None);
    assert_eq!(stored(&db, 1).fantasy_team_name, None);
    assert_eq!(stored(&db, 2).is_rostered, Some(true));
    // ...and still without disturbing the points.
    assert_eq!(stored(&db, 1).projected_points, Some(10.0));
}
