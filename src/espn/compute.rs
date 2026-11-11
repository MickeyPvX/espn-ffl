use serde_json::Value;
use std::collections::BTreeMap;

use crate::espn::types::ScoringItem;

pub fn build_scoring_index(items: &[ScoringItem]) -> BTreeMap<u16, (f64, BTreeMap<u8, f64>)> {
    let mut idx = BTreeMap::new();
    for it in items {
        idx.insert(it.stat_id, (it.points, it.points_overrides.clone()));
    }
    idx
}

/// Select the stat block for a specific season/week/source.
/// `stat_source_id`: 0 = actual, 1 = projected.
/// `stat_split_type_id`: 1 = weekly, 0 = season total.
/// Returns the `stats` map if found.
pub fn select_weekly_stats(
    player: &Value,
    season: u16,
    week: u16,
    stat_source_id: u8,
) -> Option<&Value> {
    let stats = player.get("stats")?.as_array()?;
    stats.iter().find_map(|s| {
        let season_id = s.get("seasonId").and_then(|v| v.as_u64())? as u16;
        let sp = s.get("scoringPeriodId").and_then(|v| v.as_u64())? as u16;
        let src = s.get("statSourceId").and_then(|v| v.as_u64())? as u8;
        let split = s.get("statSplitTypeId").and_then(|v| v.as_u64())? as u8;
        if season_id == season && sp == week && src == stat_source_id && split == 1 {
            s.get("stats")
        } else {
            None
        }
    })
}

/// Select the whole-season stat block for a season/source.
///
/// Season totals are the block with `scoringPeriodId == 0` and `statSplitTypeId == 0`, as
/// opposed to the per-week blocks [`select_weekly_stats`] looks for. This is what carries a
/// preseason projection, before any week has been played.
pub fn select_season_stats(player: &Value, season: u16, stat_source_id: u8) -> Option<&Value> {
    let stats = player.get("stats")?.as_array()?;
    stats.iter().find_map(|s| {
        let season_id = s.get("seasonId").and_then(|v| v.as_u64())? as u16;
        let sp = s.get("scoringPeriodId").and_then(|v| v.as_u64())? as u16;
        let src = s.get("statSourceId").and_then(|v| v.as_u64())? as u8;
        let split = s.get("statSplitTypeId").and_then(|v| v.as_u64())? as u8;
        if season_id == season && sp == 0 && src == stat_source_id && split == 0 {
            s.get("stats")
        } else {
            None
        }
    })
}

/// Weeks in a season for which the player has a non-empty projected stat block.
///
/// ESPN emits a weekly projection block for every week but leaves it empty on a bye, so the
/// missing week numbers identify byes without a separate schedule lookup.
pub fn projected_weeks(player: &Value, season: u16) -> Vec<u16> {
    let Some(stats) = player.get("stats").and_then(|s| s.as_array()) else {
        return Vec::new();
    };

    let mut weeks: Vec<u16> = stats
        .iter()
        .filter_map(|s| {
            let season_id = s.get("seasonId").and_then(|v| v.as_u64())? as u16;
            let sp = s.get("scoringPeriodId").and_then(|v| v.as_u64())? as u16;
            let src = s.get("statSourceId").and_then(|v| v.as_u64())? as u8;
            let split = s.get("statSplitTypeId").and_then(|v| v.as_u64())? as u8;
            let has_stats = s
                .get("stats")
                .and_then(|v| v.as_object())
                .is_some_and(|m| !m.is_empty());

            (season_id == season && sp > 0 && src == 1 && split == 1 && has_stats).then_some(sp)
        })
        .collect();

    weeks.sort_unstable();
    weeks.dedup();
    weeks
}

/// Compute fantasy points for one player's week, given their slot and a scoring index.
pub fn compute_points_for_week(
    weekly_stats_obj: &Value,
    player_slot_id: u8,
    scoring_index: &BTreeMap<u16, (f64, BTreeMap<u8, f64>)>,
) -> f64 {
    let Some(stats_map) = weekly_stats_obj.as_object() else {
        return 0.0;
    };

    let mut total = 0.0;
    for (stat_id_str, stat_val) in stats_map {
        // ESPN stat keys are strings; convert to u16
        let Ok(stat_id) = stat_id_str.parse::<u16>() else {
            continue;
        };
        let Some(raw) = stat_val.as_f64() else {
            continue;
        };
        if let Some((base_pts, overrides)) = scoring_index.get(&stat_id) {
            let per_unit = overrides.get(&player_slot_id).copied().unwrap_or(*base_pts);
            total += raw * per_unit;
        }
    }
    total
}

/// Infer a player's bye week from the gap in ESPN's weekly projections.
///
/// ESPN emits a projection block for every week of the season but leaves it empty on the
/// bye, so the missing week in an otherwise complete run is the bye.
pub fn infer_bye_week(player_value: &serde_json::Value, season: u16) -> Option<u16> {
    let weeks = projected_weeks(player_value, season);
    if weeks.is_empty() {
        return None;
    }

    let first = *weeks.first()?;
    let last = *weeks.last()?;
    let present: std::collections::HashSet<u16> = weeks.into_iter().collect();

    (first..=last).find(|w| !present.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn infer_bye_week_finds_the_missing_week() {
        // Weeks 1-5 projected, week 3 empty => bye in week 3.
        let stats: Vec<serde_json::Value> = (1..=5)
            .map(|w| {
                let inner = if w == 3 {
                    json!({})
                } else {
                    json!({"53": 5.0})
                };
                json!({
                    "seasonId": 2026,
                    "scoringPeriodId": w,
                    "statSourceId": 1,
                    "statSplitTypeId": 1,
                    "stats": inner,
                })
            })
            .collect();

        let player = json!({ "stats": stats });
        assert_eq!(infer_bye_week(&player, 2026), Some(3));
    }

    #[test]
    fn infer_bye_week_is_none_without_projections() {
        let player = json!({ "stats": [] });
        assert_eq!(infer_bye_week(&player, 2026), None);
    }
}
