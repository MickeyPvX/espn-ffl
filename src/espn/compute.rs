use std::collections::BTreeMap;

use crate::espn::types::{Player, ScoringItem};

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
pub fn select_weekly_stats(
    player: &Player,
    season: u16,
    week: u16,
    stat_source_id: u8,
) -> Option<&BTreeMap<String, f64>> {
    player.stats.iter().find_map(|s| {
        if s.season_id.as_u16() == season
            && s.scoring_period_id.as_u16() == week
            && s.stat_source_id == stat_source_id
            && s.stat_split_type_id == 1
        {
            Some(&s.stats)
        } else {
            None
        }
    })
}

/// Compute fantasy points for one player's week, given their slot and a scoring index.
pub fn compute_points_for_week(
    weekly_stats_map: &BTreeMap<String, f64>,
    player_slot_id: u8,
    scoring_index: &BTreeMap<u16, (f64, BTreeMap<u8, f64>)>,
) -> f64 {
    let mut total = 0.0;
    for (stat_id_str, &stat_value) in weekly_stats_map {
        // ESPN stat keys are strings; convert to u16
        let Ok(stat_id) = stat_id_str.parse::<u16>() else {
            continue;
        };
        if let Some((base_pts, overrides)) = scoring_index.get(&stat_id) {
            let per_unit = overrides.get(&player_slot_id).copied().unwrap_or(*base_pts);
            total += stat_value * per_unit;
        }
    }
    total
}
