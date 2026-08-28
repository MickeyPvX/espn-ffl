//! Value over replacement for draft ranking.
//!
//! Raw projected points are not directly comparable across positions: a 300-point QB is
//! ordinary when every team can start a 280-point QB, while a 240-point TE is a large edge
//! when the twelfth-best TE scores 150. Value over replacement (VOR) removes that by
//! measuring each player against the worst starter their league will actually field.
//!
//! The replacement level is derived from the league's own roster settings rather than a
//! convention, so a league that starts two flex spots or a superflex gets different numbers.

use std::collections::{BTreeMap, HashMap};

use crate::{espn::types::LeagueSettings, Position};

/// A player reduced to the fields VOR needs.
#[derive(Debug, Clone)]
pub struct Projected {
    pub position: Position,
    pub points: f64,
}

/// How many players at each position a league starts across all teams.
///
/// Includes flex slots, which are allocated to whichever positions actually fill them.
#[derive(Debug, Clone, Default)]
pub struct ReplacementLevels {
    /// Number of starters leaguewide at each position.
    pub starter_counts: BTreeMap<Position, usize>,
    /// Points scored by the last startable player at each position.
    pub replacement_points: HashMap<Position, f64>,
}

impl ReplacementLevels {
    /// Points the replacement-level player at this position scores.
    ///
    /// Positions the league does not start have no replacement level and yield 0.0, which
    /// makes their VOR equal to their raw projection.
    pub fn replacement_for(&self, position: Position) -> f64 {
        self.replacement_points
            .get(&position)
            .copied()
            .unwrap_or(0.0)
    }

    /// Value of a player over the replacement level at their position.
    pub fn value_over_replacement(&self, position: Position, points: f64) -> f64 {
        points - self.replacement_for(position)
    }
}

/// Lineup slot id for the flex spot (RB/WR/TE eligible).
const FLEX_SLOT: u8 = 23;

/// Compute replacement levels for a league from its roster settings and a projection pool.
///
/// Dedicated slots are counted first: with 12 teams starting two RBs, the 24 best RBs are
/// starters and the 25th sets the RB replacement level. Flex slots are then handed to the
/// best players still unclaimed across the flex-eligible positions, which is what actually
/// happens in a draft — so a league where flex is usually filled by a WR pushes the WR
/// baseline deeper than the RB one.
pub fn compute_replacement_levels(
    settings: &LeagueSettings,
    pool: &[Projected],
    team_count: usize,
) -> ReplacementLevels {
    let starting_slots = settings.starting_lineup_slots();

    // Group the pool by position, best first.
    let mut by_position: HashMap<Position, Vec<f64>> = HashMap::new();
    for p in pool {
        by_position.entry(p.position).or_default().push(p.points);
    }
    for points in by_position.values_mut() {
        points.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Dedicated (non-flex) starters per position.
    let mut starter_counts: BTreeMap<Position, usize> = BTreeMap::new();
    let mut flex_slots = 0usize;

    for (&slot, &per_team) in &starting_slots {
        let total = per_team as usize * team_count;
        if slot == FLEX_SLOT {
            flex_slots += total;
            continue;
        }

        // A dedicated slot maps to exactly one position.
        if let Some(position) = position_for_slot(slot) {
            *starter_counts.entry(position).or_insert(0) += total;
        }
    }

    // Hand flex slots to the best players not already claimed as dedicated starters.
    if flex_slots > 0 {
        let flex_positions: Vec<Position> = by_position
            .keys()
            .copied()
            .filter(|p| p.fills_slot(FLEX_SLOT))
            .collect();

        // Track how deep we have gone into each position's list.
        let mut cursor: HashMap<Position, usize> = flex_positions
            .iter()
            .map(|p| (*p, starter_counts.get(p).copied().unwrap_or(0)))
            .collect();

        for _ in 0..flex_slots {
            // Pick whichever position offers the best remaining player.
            let best = flex_positions
                .iter()
                .filter_map(|pos| {
                    let idx = cursor[pos];
                    by_position
                        .get(pos)
                        .and_then(|list| list.get(idx))
                        .map(|pts| (*pos, *pts))
                })
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let Some((pos, _)) = best else { break };
            *cursor.get_mut(&pos).unwrap() += 1;
            *starter_counts.entry(pos).or_insert(0) += 1;
        }
    }

    // Replacement level is the next player after the last starter.
    let mut replacement_points = HashMap::new();
    for (&position, &starters) in &starter_counts {
        let Some(list) = by_position.get(&position) else {
            continue;
        };
        // Fall back to the worst available player when the pool is shallower than the
        // number of starters, rather than treating replacement as free.
        let points = list
            .get(starters)
            .or_else(|| list.last())
            .copied()
            .unwrap_or(0.0);
        replacement_points.insert(position, points);
    }

    ReplacementLevels {
        starter_counts,
        replacement_points,
    }
}

/// The single position a dedicated lineup slot is filled by.
///
/// Returns `None` for multi-position slots, which are handled as flex.
fn position_for_slot(slot: u8) -> Option<Position> {
    const CANDIDATES: [Position; 12] = [
        Position::QB,
        Position::RB,
        Position::WR,
        Position::TE,
        Position::K,
        Position::DEF,
        Position::P,
        Position::DT,
        Position::DE,
        Position::LB,
        Position::DB,
        Position::S,
    ];

    let mut matches = CANDIDATES
        .into_iter()
        .filter(|p| p.lineup_slot_ids().contains(&slot));

    let first = matches.next()?;
    // Ambiguous slots are not dedicated slots.
    matches.next().is_none().then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::espn::types::{RosterSettings, ScoringSettings};

    /// Build settings with the given starting slots (slot id -> count per team).
    fn settings_with_slots(slots: &[(u8, u32)]) -> LeagueSettings {
        // ESPN always returns every slot 0..=24, most of them zero.
        let mut lineup_slot_counts: HashMap<String, u32> =
            (0..=24u8).map(|s| (s.to_string(), 0)).collect();
        for (slot, count) in slots {
            lineup_slot_counts.insert(slot.to_string(), *count);
        }

        LeagueSettings {
            scoring_settings: ScoringSettings {
                scoring_items: vec![],
            },
            roster_settings: RosterSettings {
                lineup_slot_counts,
                position_limits: HashMap::new(),
            },
            draft_settings: Default::default(),
            size: None,
            name: None,
        }
    }

    fn pool(entries: &[(Position, f64)]) -> Vec<Projected> {
        entries
            .iter()
            .map(|(position, points)| Projected {
                position: *position,
                points: *points,
            })
            .collect()
    }

    #[test]
    fn replacement_level_is_the_player_after_the_last_starter() {
        // 2 teams starting 1 QB each => QB3 is the replacement level.
        let settings = settings_with_slots(&[(0, 1)]);
        let players = pool(&[
            (Position::QB, 300.0),
            (Position::QB, 280.0),
            (Position::QB, 250.0),
            (Position::QB, 200.0),
        ]);

        let levels = compute_replacement_levels(&settings, &players, 2);

        assert_eq!(levels.starter_counts[&Position::QB], 2);
        assert_eq!(levels.replacement_for(Position::QB), 250.0);
        assert_eq!(
            levels.value_over_replacement(Position::QB, 300.0),
            50.0,
            "top QB is worth 50 over the first non-starter"
        );
    }

    #[test]
    fn scarce_positions_produce_higher_value_than_raw_points() {
        // 2 teams: 1 QB and 1 TE each. QBs are deep and flat, TEs fall off a cliff.
        let settings = settings_with_slots(&[(0, 1), (6, 1)]);
        let players = pool(&[
            (Position::QB, 300.0),
            (Position::QB, 295.0),
            (Position::QB, 290.0),
            (Position::TE, 240.0),
            (Position::TE, 160.0),
            (Position::TE, 150.0),
        ]);

        let levels = compute_replacement_levels(&settings, &players, 2);

        let qb_value = levels.value_over_replacement(Position::QB, 300.0);
        let te_value = levels.value_over_replacement(Position::TE, 240.0);

        // The TE scores 60 fewer raw points but is the more valuable pick.
        assert!(
            te_value > qb_value,
            "expected scarce TE ({}) to beat plentiful QB ({})",
            te_value,
            qb_value
        );
    }

    #[test]
    fn flex_slots_deepen_the_position_that_fills_them() {
        // 1 team: 1 RB, 1 WR, 1 FLEX. WRs are better, so the flex should go to a WR,
        // pushing the WR baseline one deeper than the RB baseline.
        let settings = settings_with_slots(&[(2, 1), (4, 1), (FLEX_SLOT, 1)]);
        let players = pool(&[
            (Position::WR, 300.0),
            (Position::WR, 290.0),
            (Position::WR, 280.0),
            (Position::RB, 200.0),
            (Position::RB, 190.0),
            (Position::RB, 180.0),
        ]);

        let levels = compute_replacement_levels(&settings, &players, 1);

        assert_eq!(levels.starter_counts[&Position::WR], 2, "WR takes the flex");
        assert_eq!(levels.starter_counts[&Position::RB], 1);
        assert_eq!(levels.replacement_for(Position::WR), 280.0);
        assert_eq!(levels.replacement_for(Position::RB), 190.0);
    }

    #[test]
    fn zero_count_slots_are_ignored() {
        // Slot 1 (TQB) and slot 7 (OP) are present in ESPN's payload but set to zero.
        let settings = settings_with_slots(&[(0, 1)]);
        let players = pool(&[(Position::QB, 300.0), (Position::QB, 250.0)]);

        let levels = compute_replacement_levels(&settings, &players, 1);

        assert_eq!(levels.starter_counts.len(), 1);
        assert!(levels.starter_counts.contains_key(&Position::QB));
    }

    #[test]
    fn shallow_pool_falls_back_to_worst_available() {
        // Two teams start a QB each but only two QBs exist; replacement cannot be free.
        let settings = settings_with_slots(&[(0, 1)]);
        let players = pool(&[(Position::QB, 300.0), (Position::QB, 250.0)]);

        let levels = compute_replacement_levels(&settings, &players, 2);

        assert_eq!(levels.replacement_for(Position::QB), 250.0);
    }

    #[test]
    fn position_for_slot_rejects_ambiguous_slots() {
        assert_eq!(position_for_slot(0), Some(Position::QB));
        assert_eq!(position_for_slot(6), Some(Position::TE));
        assert_eq!(position_for_slot(17), Some(Position::K));
        // FLEX is filled by three positions, so it is not a dedicated slot.
        assert_eq!(position_for_slot(FLEX_SLOT), None);
    }
}
