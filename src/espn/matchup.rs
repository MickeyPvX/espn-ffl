//! Matchup win probability and risk-aware lineup selection.
//!
//! Backtesting on real league data (2025-2026) found nothing available here that predicts
//! where a player will land *relative to* ESPN's weekly projection: correcting for a player's
//! past misses made projections worse. What *is* predictable is how widely outcomes spread
//! around the projection, which depends mostly on position and projection level.
//!
//! So this module keeps ESPN's projection as each player's expected score, models the score
//! as normally distributed around it, and picks the lineup with the best chance of outscoring
//! this week's opponent. That differs from "highest projected total" exactly when it matters:
//! an underdog should take on variance (a chance at an upset), a favourite should shed it.

use std::collections::{BTreeMap, HashMap};

use serde::{Serialize, Serializer};

use crate::{cli::types::position::slot_flexibility, Position};

/// Smallest variance assigned to a playing player (a 2-point standard deviation).
const MIN_VARIANCE: f64 = 4.0;

/// Rows a position needs before its variance curve is fitted rather than defaulted.
const MIN_FIT_ROWS: usize = 50;

/// Weight, in weeks, given to the position-level variance when blending in a player's history.
///
/// Calibrated by walk-forward backtest: a player's own spread carries little information
/// beyond what position and projection already give, so it takes many weeks to move the
/// estimate. Lower values fit the next week's outcomes measurably worse.
pub const PLAYER_HISTORY_PRIOR_WEEKS: f64 = 16.0;

/// Risk weights tried when searching for the lineup with the best win probability.
///
/// Each candidate lineup maximises `mean + lambda * variance`; the optimum for a
/// win-probability objective sits near `lambda = -margin / (2 * total variance)`, which for
/// realistic margins (±60 points) and variances (~900) stays well inside ±0.1.
const LAMBDA_STEP: f64 = 0.002;
const LAMBDA_STEPS: i32 = 50;

/// Win-probability gain below which a riskier lineup is not worth recommending over a
/// higher-projected one.
const WIN_PROB_TOLERANCE: f64 = 0.002;

/// Variance of a player's weekly score around their projection, as `a + b * projection`.
#[derive(Debug, Clone)]
pub struct VarianceModel {
    coefficients: HashMap<Position, (f64, f64)>,
}

impl Default for VarianceModel {
    /// Coefficients fitted on 2025 data for a PPR league, used for any position without
    /// enough local history to fit its own curve.
    fn default() -> Self {
        let coefficients = HashMap::from([
            (Position::QB, (15.4, 2.32)),
            (Position::RB, (1.3, 3.78)),
            (Position::WR, (3.7, 4.06)),
            (Position::TE, (5.4, 3.86)),
            (Position::K, (25.0, 0.0)),
            (Position::DEF, (25.0, 2.39)),
        ]);
        Self { coefficients }
    }
}

impl VarianceModel {
    /// Fit a variance curve per position from `(position, projected, actual)` history.
    ///
    /// Regresses each squared miss on the projection. Positions with too little history keep
    /// the default curve, and a negative slope (kickers: ESPN's projections barely separate
    /// them) is replaced by a flat curve at the average squared miss.
    pub fn fit(history: &[(Position, f64, f64)]) -> Self {
        let mut model = Self::default();
        let mut by_position: HashMap<Position, Vec<(f64, f64)>> = HashMap::new();
        for &(position, projected, actual) in history {
            if projected > 0.0 {
                by_position
                    .entry(position)
                    .or_default()
                    .push((projected, (actual - projected).powi(2)));
            }
        }

        for (position, points) in by_position {
            if points.len() < MIN_FIT_ROWS {
                continue;
            }
            let n = points.len() as f64;
            let mean_x = points.iter().map(|(x, _)| x).sum::<f64>() / n;
            let mean_y = points.iter().map(|(_, y)| y).sum::<f64>() / n;
            let sxx: f64 = points.iter().map(|(x, _)| (x - mean_x).powi(2)).sum();
            let sxy: f64 = points
                .iter()
                .map(|(x, y)| (x - mean_x) * (y - mean_y))
                .sum();
            let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
            let coefficients = if slope > 0.0 {
                (mean_y - slope * mean_x, slope)
            } else {
                (mean_y, 0.0)
            };
            model.coefficients.insert(position, coefficients);
        }
        model
    }

    /// Variance for a player at `position` projected for `projection` points.
    ///
    /// A player projected for nothing (bye, ruled out) is certain to score nothing.
    pub fn variance(&self, position: Position, projection: f64) -> f64 {
        if projection <= 0.0 {
            return 0.0;
        }
        let (a, b) = self
            .coefficients
            .get(&position)
            .copied()
            .unwrap_or((20.0, 2.0));
        (a + b * projection).max(MIN_VARIANCE)
    }
}

/// Blend a position-level variance with a player's own squared misses this season.
///
/// Weighted as if the position estimate were [`PLAYER_HISTORY_PRIOR_WEEKS`] weeks of data.
/// Misses are measured around ESPN's projection rather than the player's own average,
/// because the player's average miss does not persist (see the module docs).
pub fn blend_player_variance(position_variance: f64, squared_misses: &[f64]) -> f64 {
    if position_variance <= 0.0 {
        return 0.0;
    }
    let n = squared_misses.len() as f64;
    let total: f64 = squared_misses.iter().sum();
    ((total + PLAYER_HISTORY_PRIOR_WEEKS * position_variance) / (n + PLAYER_HISTORY_PRIOR_WEEKS))
        .max(MIN_VARIANCE)
}

fn serialize_display<S: Serializer, T: std::fmt::Display>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// A rostered player as a lineup candidate.
#[derive(Debug, Clone, Serialize)]
pub struct LineupPlayer {
    pub player_id: i64,
    pub name: String,
    #[serde(serialize_with = "serialize_display")]
    pub position: Position,
    /// Expected points: ESPN's projection in league scoring.
    pub mean: f64,
    pub variance: f64,
    /// Slot the player currently occupies on ESPN.
    pub current_slot: u8,
    /// The player's game has started, so they cannot be moved in or out.
    pub locked: bool,
    pub injury_status: Option<String>,
}

impl LineupPlayer {
    pub fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }
}

/// A starting lineup: each starting slot and the player (by index) filling it.
#[derive(Debug, Clone, PartialEq)]
pub struct Lineup {
    /// `(slot, player index)`, with `None` when no eligible player could fill the slot.
    pub assignments: Vec<(u8, Option<usize>)>,
    pub mean: f64,
    pub variance: f64,
}

impl Lineup {
    fn from_assignments(assignments: Vec<(u8, Option<usize>)>, players: &[LineupPlayer]) -> Self {
        let starters = assignments.iter().filter_map(|(_, p)| *p);
        let (mean, variance) = starters.fold((0.0, 0.0), |(m, v), i| {
            (m + players[i].mean, v + players[i].variance)
        });
        Self {
            assignments,
            mean,
            variance,
        }
    }

    /// The lineup the team currently has set on ESPN.
    pub fn current(players: &[LineupPlayer], slots: &BTreeMap<u8, u32>) -> Self {
        let mut assignments: Vec<(u8, Option<usize>)> = players
            .iter()
            .enumerate()
            .filter(|(_, p)| slots.contains_key(&p.current_slot))
            .map(|(i, p)| (p.current_slot, Some(i)))
            .collect();
        // Show unfilled slots too, so an empty spot in the lineup is visible.
        for (&slot, &count) in slots {
            let filled = assignments.iter().filter(|(s, _)| *s == slot).count();
            for _ in filled..count as usize {
                assignments.push((slot, None));
            }
        }
        assignments.sort_by_key(|(slot, _)| slot_order(*slot));
        Self::from_assignments(assignments, players)
    }

    /// Indices of the players in the lineup.
    pub fn starters(&self) -> Vec<usize> {
        self.assignments.iter().filter_map(|(_, p)| *p).collect()
    }

    pub fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }
}

/// Display order for slots: QB, RB, WR, TE, flex slots, K, D/ST.
fn slot_order(slot: u8) -> (u8, u8) {
    match slot {
        16 => (3, slot), // D/ST after K
        17 => (2, slot),
        s if slot_flexibility(s) > 1 => (1, s),
        s => (0, s),
    }
}

/// Fill `slots` with the players that maximise the sum of `value`.
///
/// Locked starters keep their slot and locked bench players stay out. Remaining slots are
/// filled tightest-first so a flex slot never takes a player only a dedicated slot could use.
/// Players projected for nothing are never started; a slot with no other option stays empty.
pub fn build_lineup(
    players: &[LineupPlayer],
    slots: &BTreeMap<u8, u32>,
    value: impl Fn(&LineupPlayer) -> f64,
) -> Lineup {
    let mut used = vec![false; players.len()];
    let mut open = slots.clone();
    let mut assignments = Vec::new();

    for (i, player) in players.iter().enumerate() {
        if !player.locked {
            continue;
        }
        used[i] = true;
        if let Some(count) = open.get_mut(&player.current_slot) {
            if *count > 0 {
                *count -= 1;
                assignments.push((player.current_slot, Some(i)));
            }
        }
    }

    let mut order: Vec<u8> = open
        .into_iter()
        .flat_map(|(slot, count)| std::iter::repeat_n(slot, count as usize))
        .collect();
    order.sort_by_key(|slot| (slot_flexibility(*slot), *slot));

    for slot in order {
        let best = players
            .iter()
            .enumerate()
            .filter(|(i, p)| !used[*i] && p.mean > 0.0 && p.position.fills_slot(slot))
            .max_by(|(_, a), (_, b)| value(a).total_cmp(&value(b)))
            .map(|(i, _)| i);
        if let Some(i) = best {
            used[i] = true;
        }
        assignments.push((slot, best));
    }

    assignments.sort_by_key(|(slot, _)| slot_order(*slot));
    Lineup::from_assignments(assignments, players)
}

/// The lineup with the highest projected total.
pub fn max_projection_lineup(players: &[LineupPlayer], slots: &BTreeMap<u8, u32>) -> Lineup {
    build_lineup(players, slots, |p| p.mean)
}

/// Standard normal cumulative distribution function.
pub fn normal_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / std::f64::consts::SQRT_2)
}

/// Complementary error function (Numerical Recipes `erfcc`, fractional error < 1.2e-7).
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let poly = -z * z - 1.265_512_23
        + t * (1.000_023_68
            + t * (0.374_091_96
                + t * (0.096_784_18
                    + t * (-0.186_288_06
                        + t * (0.278_868_07
                            + t * (-1.135_203_98
                                + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77))))))));
    let r = t * poly.exp();
    if x >= 0.0 {
        r
    } else {
        2.0 - r
    }
}

/// Chance a team with score distribution `(mean, variance)` outscores the opponent's.
///
/// Players are treated as independent, which backtesting supports: the predicted spread of
/// matchup margins (29.7 points) matched the observed spread (28.2).
pub fn win_probability(my_mean: f64, my_var: f64, opp_mean: f64, opp_var: f64) -> f64 {
    let sd = (my_var + opp_var).sqrt();
    if sd == 0.0 {
        return match my_mean.partial_cmp(&opp_mean) {
            Some(std::cmp::Ordering::Greater) => 1.0,
            Some(std::cmp::Ordering::Less) => 0.0,
            _ => 0.5,
        };
    }
    normal_cdf((my_mean - opp_mean) / sd)
}

/// The lineup with the best chance of beating an opponent scoring `(opp_mean, opp_var)`.
///
/// Searches lineups that maximise `mean + lambda * variance` across a range of risk weights,
/// then keeps the one with the highest win probability. Ties (within
/// [`WIN_PROB_TOLERANCE`]) go to the higher projected total, so the recommendation only
/// trades points for variance when that actually buys a better chance to win.
pub fn best_lineup_for_matchup(
    players: &[LineupPlayer],
    slots: &BTreeMap<u8, u32>,
    opp_mean: f64,
    opp_var: f64,
) -> (Lineup, f64) {
    let mut candidates: Vec<(Lineup, f64)> = (-LAMBDA_STEPS..=LAMBDA_STEPS)
        .map(|k| {
            let lambda = k as f64 * LAMBDA_STEP;
            let lineup = build_lineup(players, slots, |p| p.mean + lambda * p.variance);
            let wp = win_probability(lineup.mean, lineup.variance, opp_mean, opp_var);
            (lineup, wp)
        })
        .collect();

    let best_wp = candidates
        .iter()
        .map(|(_, wp)| *wp)
        .fold(f64::NEG_INFINITY, f64::max);
    candidates.retain(|(_, wp)| *wp >= best_wp - WIN_PROB_TOLERANCE);
    candidates
        .into_iter()
        .max_by(|(a, _), (b, _)| a.mean.total_cmp(&b.mean))
        .expect("at least one candidate lineup")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(id: i64, position: Position, mean: f64, sd: f64, slot: u8) -> LineupPlayer {
        LineupPlayer {
            player_id: id,
            name: format!("P{id}"),
            position,
            mean,
            variance: sd * sd,
            current_slot: slot,
            locked: false,
            injury_status: None,
        }
    }

    /// One QB, one RB, one WR, one FLEX.
    fn slots() -> BTreeMap<u8, u32> {
        BTreeMap::from([(0, 1), (2, 1), (4, 1), (23, 1)])
    }

    #[test]
    fn normal_cdf_matches_known_values() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-7);
        assert!((normal_cdf(1.0) - 0.841_344_75).abs() < 1e-6);
        assert!((normal_cdf(-1.96) - 0.024_997_9).abs() < 1e-6);
    }

    #[test]
    fn win_probability_is_symmetric_and_handles_certainty() {
        let p = win_probability(110.0, 400.0, 100.0, 500.0);
        let q = win_probability(100.0, 500.0, 110.0, 400.0);
        assert!((p + q - 1.0).abs() < 1e-9);
        assert!(p > 0.5);
        assert_eq!(win_probability(10.0, 0.0, 5.0, 0.0), 1.0);
        assert_eq!(win_probability(5.0, 0.0, 5.0, 0.0), 0.5);
    }

    #[test]
    fn max_projection_fills_dedicated_slots_before_flex() {
        let players = vec![
            player(1, Position::QB, 20.0, 7.0, 0),
            player(2, Position::RB, 15.0, 6.0, 2),
            player(3, Position::RB, 14.0, 6.0, 20),
            player(4, Position::WR, 9.0, 5.0, 4),
            player(5, Position::TE, 3.0, 3.0, 20),
        ];
        let lineup = max_projection_lineup(&players, &slots());
        let mut starters = lineup.starters();
        starters.sort();
        // The WR must take the WR slot even though both RBs outscore him.
        assert_eq!(starters, vec![0, 1, 2, 3]);
        assert!((lineup.mean - 58.0).abs() < 1e-9);
    }

    #[test]
    fn zero_projection_players_are_not_started() {
        let players = vec![
            player(1, Position::QB, 0.0, 0.0, 0), // on bye
            player(2, Position::RB, 15.0, 6.0, 2),
            player(3, Position::WR, 10.0, 5.0, 4),
        ];
        let lineup = max_projection_lineup(&players, &slots());
        assert!(lineup.assignments.contains(&(0, None)));
        assert!(!lineup.starters().contains(&0));
    }

    #[test]
    fn locked_players_stay_where_they_are() {
        let mut players = vec![
            player(1, Position::QB, 20.0, 7.0, 0),
            player(2, Position::RB, 5.0, 3.0, 2),
            player(3, Position::RB, 15.0, 6.0, 20),
            player(4, Position::WR, 10.0, 5.0, 4),
        ];
        players[1].locked = true; // low scorer locked in the RB slot
        players[2].locked = true; // better RB locked on the bench
        let lineup = max_projection_lineup(&players, &slots());
        assert!(lineup.assignments.contains(&(2, Some(1))));
        assert!(!lineup.starters().contains(&2));
    }

    /// QB, RB and WR are clear starters; FLEX is steady 12 ±3 against volatile 11 ±10.
    fn flex_decision() -> Vec<LineupPlayer> {
        vec![
            player(1, Position::QB, 20.0, 7.0, 0),
            player(2, Position::RB, 15.0, 6.0, 2),
            player(3, Position::WR, 25.0, 5.0, 4),
            player(4, Position::WR, 12.0, 3.0, 23),
            player(5, Position::WR, 11.0, 10.0, 20),
        ]
    }
    const STEADY: usize = 3;
    const VOLATILE: usize = 4;

    #[test]
    fn underdog_prefers_upside_and_favourite_prefers_floor() {
        let players = flex_decision();
        let max_proj = max_projection_lineup(&players, &slots());
        assert!(max_proj.starters().contains(&STEADY));

        // Projected to lose by 30: take the volatile player for a shot at the upset.
        let opp = max_proj.mean + 30.0;
        let (lineup, wp) = best_lineup_for_matchup(&players, &slots(), opp, 400.0);
        assert!(lineup.starters().contains(&VOLATILE));
        assert!(wp > win_probability(max_proj.mean, max_proj.variance, opp, 400.0));

        // Projected to win by 30: keep the steady points.
        let opp = max_proj.mean - 30.0;
        let (lineup, _) = best_lineup_for_matchup(&players, &slots(), opp, 400.0);
        assert!(lineup.starters().contains(&STEADY));
    }

    #[test]
    fn even_matchup_keeps_the_max_projection_lineup() {
        let players = flex_decision();
        let max_proj = max_projection_lineup(&players, &slots());
        let (lineup, _) = best_lineup_for_matchup(&players, &slots(), max_proj.mean, 400.0);
        assert_eq!(lineup, max_proj);
    }

    #[test]
    fn variance_model_fits_slope_and_floors_bad_slopes() {
        // Squared misses grow with projection for WRs: miss = ±sqrt(4 * proj).
        let mut history = Vec::new();
        for i in 0..200 {
            let proj = 5.0 + (i % 15) as f64;
            let miss = (4.0 * proj).sqrt() * if i % 2 == 0 { 1.0 } else { -1.0 };
            history.push((Position::WR, proj, proj + miss));
            // Kickers: misses shrink with projection, which should flatten to a constant.
            let k_miss = (60.0 - 2.0 * proj).sqrt();
            history.push((Position::K, proj, proj + k_miss));
        }
        let model = VarianceModel::fit(&history);
        assert!((model.variance(Position::WR, 10.0) - 40.0).abs() < 1e-6);
        let k10 = model.variance(Position::K, 10.0);
        assert!((k10 - model.variance(Position::K, 15.0)).abs() < 1e-9);
        assert_eq!(model.variance(Position::WR, 0.0), 0.0);
    }

    #[test]
    fn blending_leans_on_position_until_history_builds_up() {
        assert_eq!(blend_player_variance(50.0, &[]), 50.0);
        // Four huge misses move a 50 variance only part of the way toward 400.
        let blended = blend_player_variance(50.0, &[400.0; 4]);
        assert!((blended - 120.0).abs() < 1e-9);
        assert_eq!(blend_player_variance(0.0, &[400.0]), 0.0);
    }
}
