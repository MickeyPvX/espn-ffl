use serde_json::Value;
use std::collections::BTreeMap;

use crate::espn::types::ScoringItem;

#[cfg(test)]
mod tests;

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
pub fn select_weekly_stats<'a>(
    player: &'a Value,
    season: u16,
    week: u16,
    stat_source_id: u8,
) -> Option<&'a Value> {
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
