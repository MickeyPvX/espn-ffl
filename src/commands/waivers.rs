//! Waiver command: the claims to submit this week, in priority order.
//!
//! ESPN processes a team's waiver claims in the order submitted, and cancels a claim whose
//! drop player is already gone. So the plan is a list: the add/drop that most improves your
//! chance of winning this week's matchup, then backups for it (other adds with the same drop,
//! which only go through if the claims above them failed), then the best next move assuming
//! the first went through, and so on.
//!
//! A swap is only offered if it does not cost projected starting-lineup points over the rest
//! of the regular season, so a one-week gain never comes from dropping a player you need.
//! Injured players are never suggested as drops: ESPN projects them at zero while they are
//! out, which says nothing about their value once they return.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::{
    cli::types::position::{SLOT_BENCH, SLOT_IR},
    espn::{
        http::{get_player_data, PlayerDataRequest},
        matchup::{best_lineup_for_matchup, max_projection_lineup, LineupPlayer},
        types::Player,
    },
    Position, Result, Season, Week,
};

use super::matchup::MatchupContext;

/// Free agents per position given the full add/drop evaluation, chosen by projected points.
const SHORTLIST_PER_POSITION: usize = 12;

/// Rest-of-season starting-lineup points a swap may cost and still be offered.
const SEASON_TOLERANCE: f64 = 0.5;

/// Parameters for the waivers command.
#[derive(Debug, Clone)]
pub struct WaiversParams {
    pub league_id: Option<crate::LeagueId>,
    pub season: Season,
    pub week: Week,
    pub team: Option<String>,
    pub team_id: Option<u32>,
    pub positions: Option<Vec<Position>>,
    /// Maximum number of claims (including backups) to list.
    pub top: usize,
    /// Backup claims to list under each claim.
    pub backups: usize,
    /// Last week counted for the season column; defaults to the end of the regular season.
    pub through_week: Option<Week>,
    pub refresh: bool,
    pub as_json: bool,
}

/// A roster member while planning: one of your players, or a free agent you claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Member {
    Mine(usize),
    Free(usize),
}

/// How good a roster is: this week's objective (win probability, or projected points when
/// there is no matchup) and projected starting-lineup points through the season.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RosterScore {
    week: f64,
    season: f64,
}

/// One planned claim, as indices into the free agents and the roster at that point.
#[derive(Debug, Clone, PartialEq)]
struct PlannedClaim {
    add: usize,
    drop: Option<Member>,
    before: RosterScore,
    after: RosterScore,
    /// Index (in the plan) of the claim this one backs up.
    backup_for: Option<usize>,
}

/// Build the claim list.
///
/// `score` evaluates a roster; `droppable` says which members may be cut (not locked);
/// `value` breaks ties between equally good drops (the lowest-value player goes first);
/// `interchangeable` says whether one free agent can stand in for another (same position),
/// which is what makes an add a backup rather than a separate move;
/// `open_spots` is how many claims can be made without dropping anyone. Only swaps that
/// raise this week's objective by at least `min_gain` are listed.
#[allow(clippy::too_many_arguments)]
fn plan_claims(
    mut roster: Vec<Member>,
    free_agents: usize,
    mut open_spots: usize,
    droppable: impl Fn(Member) -> bool,
    value: impl Fn(Member) -> f64,
    interchangeable: impl Fn(usize, usize) -> bool,
    mut score: impl FnMut(&[Member]) -> RosterScore,
    min_gain: f64,
    max_claims: usize,
    backups: usize,
) -> Vec<PlannedClaim> {
    let mut plan: Vec<PlannedClaim> = Vec::new();
    let mut current = score(&roster);

    while plan.len() < max_claims {
        // Every legal swap from here, with the best drop for each free agent.
        let mut best_per_add: Vec<(usize, Option<Member>, RosterScore)> = Vec::new();
        for add in 0..free_agents {
            if roster.contains(&Member::Free(add)) {
                continue;
            }
            let mut drops: Vec<Option<Member>> = roster
                .iter()
                .copied()
                .filter(|m| droppable(*m))
                .map(Some)
                .collect();
            if open_spots > 0 {
                drops.push(None);
            }
            let best = drops
                .into_iter()
                .filter_map(|drop| {
                    let mut next: Vec<Member> = roster
                        .iter()
                        .copied()
                        .filter(|m| Some(*m) != drop)
                        .collect();
                    next.push(Member::Free(add));
                    let after = score(&next);
                    (after.season >= current.season - SEASON_TOLERANCE).then_some((drop, after))
                })
                .max_by(|(da, a), (db, b)| {
                    // Best this week; then most season value kept; then cut the weakest.
                    let cut = |d: &Option<Member>| d.map(&value).unwrap_or(f64::NEG_INFINITY);
                    a.week
                        .total_cmp(&b.week)
                        .then(a.season.total_cmp(&b.season))
                        .then(cut(db).total_cmp(&cut(da)))
                });
            if let Some((drop, after)) = best {
                if after.week - current.week >= min_gain {
                    best_per_add.push((add, drop, after));
                }
            }
        }
        best_per_add.sort_by(|a, b| {
            b.2.week
                .total_cmp(&a.2.week)
                .then(b.2.season.total_cmp(&a.2.season))
        });

        let Some(&(add, drop, after)) = best_per_add.first() else {
            break;
        };
        let primary = plan.len();
        plan.push(PlannedClaim {
            add,
            drop,
            before: current,
            after,
            backup_for: None,
        });

        // Backups: alternatives at the same position whose best move uses the same drop. They
        // fail automatically once the primary succeeds, so they cost nothing to submit.
        for &(b_add, b_drop, b_after) in best_per_add.iter().skip(1) {
            if plan.len() >= max_claims || plan.len() - primary > backups {
                break;
            }
            if b_drop == drop && drop.is_some() && interchangeable(add, b_add) {
                plan.push(PlannedClaim {
                    add: b_add,
                    drop,
                    before: current,
                    after: b_after,
                    backup_for: Some(primary),
                });
            }
        }

        // Plan the next claim as if this one went through.
        match drop {
            Some(d) => roster.retain(|m| *m != d),
            None => open_spots -= 1,
        }
        roster.push(Member::Free(add));
        current = after;
    }
    plan
}

/// A player with a lineup candidate for each evaluated week (index 0 is this week).
struct Evaluated<'a> {
    player: &'a Player,
    weekly: Vec<LineupPlayer>,
}

impl Evaluated<'_> {
    fn total(&self) -> f64 {
        self.weekly.iter().map(|p| p.mean).sum()
    }
}

/// A player in a claim, as reported.
#[derive(Debug, Serialize)]
struct ClaimPlayer {
    player_id: i64,
    name: String,
    position: String,
    /// ESPN's projection this week.
    projection: f64,
    injury_status: Option<String>,
}

impl ClaimPlayer {
    fn new(e: &Evaluated) -> Self {
        let now = &e.weekly[0];
        Self {
            player_id: e.player.id,
            name: now.name.clone(),
            position: now.position.to_string(),
            projection: now.mean,
            injury_status: now.injury_status.clone(),
        }
    }
}

/// One waiver claim to submit.
#[derive(Debug, Serialize)]
struct Claim {
    /// Priority order to submit the claim in, starting at 1.
    order: usize,
    add: ClaimPlayer,
    /// `None` when the roster has an open spot.
    drop: Option<ClaimPlayer>,
    /// This week's win probability (or projected points with no matchup) before and after.
    before: f64,
    after: f64,
    /// Projected starting-lineup points gained through `through_week`.
    season_change: f64,
    /// The claim (by `order`) this one backs up.
    backup_for: Option<usize>,
}

#[derive(Debug, Serialize)]
struct WaiverReport {
    season: u16,
    week: u16,
    through_week: u16,
    team: String,
    /// `None` when there is no matchup this week; claims are then ranked by projected points.
    opponent: Option<String>,
    claims: Vec<Claim>,
}

/// Handle the waivers command.
pub async fn handle_waivers(params: WaiversParams) -> Result<()> {
    let ctx = MatchupContext::load(
        params.league_id,
        params.season,
        params.week,
        params.team.as_deref(),
        params.team_id,
        None,
        params.refresh,
    )
    .await?;

    let through = params
        .through_week
        .map(|w| w.as_u16())
        .unwrap_or(ctx.last_regular_week)
        .max(ctx.week.as_u16());
    let weeks: Vec<Week> = (ctx.week.as_u16()..=through).map(Week::new).collect();

    if !params.as_json {
        println!("Loading player projections...");
    }
    let raw = get_player_data(PlayerDataRequest {
        debug: false,
        refresh: params.refresh,
        league_id: ctx.league_id,
        player_names: None,
        positions: None,
        season: ctx.season,
        week: ctx.week,
        injury_status_filter: None,
        roster_status_filter: None,
        fallback_slot_ids: Some(ctx.settings.rosterable_slot_ids()),
    })
    .await?;
    let pool: Vec<Player> = serde_json::from_value(raw)?;
    let by_id: HashMap<i64, &Player> = pool.iter().map(|p| (p.id, p)).collect();

    let to_eval = |player: &Player| -> Option<Vec<LineupPlayer>> {
        weeks
            .iter()
            .map(|&w| ctx.lineup_player(player, SLOT_BENCH, false, w))
            .collect()
    };

    // My roster: IR stashes stay put; everyone else is a lineup option and a drop candidate.
    // This week's entries keep their real slot and lock, so locked players stay in place.
    let me = ctx.me();
    let this_week: HashMap<i64, LineupPlayer> = ctx
        .roster_players(me)
        .into_iter()
        .map(|p| (p.player_id, p))
        .collect();
    let mine: Vec<Evaluated> = this_week
        .keys()
        .filter_map(|id| {
            let player = by_id.get(id).copied()?;
            let mut weekly = to_eval(player)?;
            weekly[0] = this_week[id].clone();
            Some(Evaluated { player, weekly })
        })
        .collect();

    // Roster spots excluding IR; an open one means a pickup needs no drop.
    let roster_spots: usize = ctx
        .settings
        .active_lineup_slots()
        .iter()
        .filter(|(slot, _)| **slot != SLOT_IR)
        .map(|(_, count)| *count as usize)
        .sum();
    let open_spots = roster_spots.saturating_sub(mine.len());

    // Free agents: anyone on no roster, at a position this league starts.
    let rostered: HashSet<i64> = ctx
        .league
        .teams
        .iter()
        .flat_map(|t| t.roster.iter().flat_map(|r| &r.entries))
        .map(|e| e.player_id)
        .collect();
    let startable = ctx.settings.rosterable_positions();
    let mut free_agents: Vec<Evaluated> = pool
        .iter()
        .filter(|p| !rostered.contains(&p.id))
        .filter_map(|player| {
            let position =
                Position::from_default_position_id(player.default_position_id as u8).ok()?;
            let wanted = match &params.positions {
                Some(filter) => filter.contains(&position),
                None => startable.contains(&position),
            };
            wanted.then(|| {
                Some(Evaluated {
                    player,
                    weekly: to_eval(player)?,
                })
            })?
        })
        .filter(|e| e.weekly[0].mean > 0.0 || e.total() > 0.0)
        .collect();
    // Shortlist by this week's projection, then by the rest of the season.
    free_agents.sort_by(|a, b| {
        b.weekly[0]
            .mean
            .total_cmp(&a.weekly[0].mean)
            .then(b.total().total_cmp(&a.total()))
    });
    let mut per_position: HashMap<Position, usize> = HashMap::new();
    free_agents.retain(|e| {
        let n = per_position.entry(e.weekly[0].position).or_default();
        *n += 1;
        *n <= SHORTLIST_PER_POSITION
    });

    let opponent = ctx.opponent().ok();
    let opponent_lineup = opponent.map(|opp| {
        let theirs = ctx.roster_players(opp);
        max_projection_lineup(&theirs, &ctx.slots)
    });

    let evaluated = |m: Member| -> &Evaluated {
        match m {
            Member::Mine(i) => &mine[i],
            Member::Free(i) => &free_agents[i],
        }
    };
    let score = |roster: &[Member]| -> RosterScore {
        let now: Vec<LineupPlayer> = roster
            .iter()
            .map(|m| evaluated(*m).weekly[0].clone())
            .collect();
        let week = match &opponent_lineup {
            Some(opp) => best_lineup_for_matchup(&now, &ctx.slots, opp.mean, opp.variance).1,
            None => max_projection_lineup(&now, &ctx.slots).mean,
        };
        let season = (0..weeks.len())
            .map(|w| {
                let players: Vec<LineupPlayer> = roster
                    .iter()
                    .map(|m| evaluated(*m).weekly[w].clone())
                    .collect();
                max_projection_lineup(&players, &ctx.slots).mean
            })
            .sum();
        RosterScore { week, season }
    };

    let min_gain = if opponent_lineup.is_some() {
        0.001 // a tenth of a percentage point
    } else {
        0.1 // projected points
    };
    let plan = plan_claims(
        (0..mine.len()).map(Member::Mine).collect(),
        free_agents.len(),
        open_spots,
        // Never cut a locked player, or an injured one: ESPN projects injured players at zero
        // for the weeks they miss, so projections alone would make a star look worthless.
        |m| {
            let now = &evaluated(m).weekly[0];
            !now.locked && now.injury_status.is_none()
        },
        |m| evaluated(m).total(),
        |a, b| free_agents[a].weekly[0].position == free_agents[b].weekly[0].position,
        score,
        min_gain,
        params.top,
        params.backups,
    );

    let claims = plan
        .iter()
        .enumerate()
        .map(|(i, c)| Claim {
            order: i + 1,
            add: ClaimPlayer::new(&free_agents[c.add]),
            drop: c.drop.map(|d| ClaimPlayer::new(evaluated(d))),
            before: c.before.week,
            after: c.after.week,
            season_change: c.after.season - c.before.season,
            backup_for: c.backup_for.map(|b| b + 1),
        })
        .collect();

    let report = WaiverReport {
        season: ctx.season.as_u16(),
        week: ctx.week.as_u16(),
        through_week: through,
        team: me.display_name(),
        opponent: opponent.map(|o| o.display_name()),
        claims,
    };

    if params.as_json {
        println!("{}", serde_json::to_string_pretty(&report)?); // tarpaulin::skip
    } else {
        print!("{}", report_text(&report)); // tarpaulin::skip
    }
    Ok(())
}

fn player_cell(p: &ClaimPlayer) -> String {
    let status = p
        .injury_status
        .as_deref()
        .map(|s| format!(" [{}]", s.chars().next().unwrap_or('?')))
        .unwrap_or_default();
    format!(
        "{:<20} {:<4} {:>4.1}{status}",
        p.name.chars().take(20).collect::<String>(),
        p.position,
        p.projection
    )
}

/// Render the report as text.
fn report_text(r: &WaiverReport) -> String {
    let has_matchup = r.opponent.is_some();
    let fmt_score = |v: f64| {
        if has_matchup {
            format!("{:.1}%", v * 100.0)
        } else {
            format!("{v:.1}")
        }
    };

    let mut out = match &r.opponent {
        Some(opp) => format!("Waiver claims · week {} · {} vs {}\n", r.week, r.team, opp),
        None => format!(
            "Waiver claims · week {} · {} (no matchup: ranked by projected points)\n",
            r.week, r.team
        ),
    };
    let Some(first) = r.claims.first() else {
        out += "\nNo free agent improves your team this week without costing you later.\n";
        return out;
    };
    out += &format!(
        "{} with your best current lineup: {}\n\n",
        if has_matchup {
            "Win chance"
        } else {
            "Projected points"
        },
        fmt_score(first.before)
    );

    out += &format!(
        " #  {:<32} {:<32} {:<20} {:>6}\n",
        "Add (proj)",
        "Drop (proj)",
        if has_matchup { "Win chance" } else { "Points" },
        "Season"
    );
    for c in &r.claims {
        let change = if has_matchup {
            format!("{:+.1}", (c.after - c.before) * 100.0)
        } else {
            format!("{:+.1}", c.after - c.before)
        };
        let drop = c
            .drop
            .as_ref()
            .map(player_cell)
            .unwrap_or_else(|| "(open roster spot)".to_string());
        let note = c
            .backup_for
            .map(|b| format!("  backup if #{b} fails"))
            .unwrap_or_default();
        out += &format!(
            "{:>2}  {:<32} {:<32} {:<20} {:>+6.1}{note}\n",
            c.order,
            player_cell(&c.add),
            drop,
            format!(
                "{} → {} ({change})",
                fmt_score(c.before),
                fmt_score(c.after)
            ),
            c.season_change,
        );
    }
    out += &format!(
        "\nSubmit in this order. A claim whose drop player is already gone is cancelled, so a\n\
         backup only goes through if the claims above it failed; later claims assume the ones\n\
         above them succeeded. Season: projected starting-lineup points gained through week {}.\n",
        r.through_week
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Toy model: each member has a value; this week's score is the sum of the best
    /// `starters` values, and the season score is the same over the long-term values.
    struct Toy {
        mine: Vec<(f64, f64)>,
        free: Vec<(f64, f64)>,
        starters: usize,
        /// Position group per free agent; backups must share one.
        groups: Vec<u8>,
    }

    impl Toy {
        fn get(&self, m: Member) -> (f64, f64) {
            match m {
                Member::Mine(i) => self.mine[i],
                Member::Free(i) => self.free[i],
            }
        }

        fn score(&self, roster: &[Member]) -> RosterScore {
            let top = |pick: fn((f64, f64)) -> f64| {
                let mut v: Vec<f64> = roster.iter().map(|m| pick(self.get(*m))).collect();
                v.sort_by(|a, b| b.total_cmp(a));
                v.iter().take(self.starters).sum::<f64>()
            };
            RosterScore {
                week: top(|v| v.0),
                season: top(|v| v.1),
            }
        }

        fn plan(&self, open: usize, top: usize, backups: usize) -> Vec<PlannedClaim> {
            plan_claims(
                (0..self.mine.len()).map(Member::Mine).collect(),
                self.free.len(),
                open,
                |_| true,
                |m| self.get(m).1,
                |a, b| self.groups.get(a) == self.groups.get(b),
                |r| self.score(r),
                0.1,
                top,
                backups,
            )
        }
    }

    #[test]
    fn claims_are_ordered_by_gain_with_backups_sharing_the_drop() {
        // Two starters. Member 1 is a weak starter (5 this week); member 2 never starts.
        let toy = Toy {
            mine: vec![(20.0, 60.0), (5.0, 20.0), (1.0, 1.0)],
            free: vec![(12.0, 12.0), (10.0, 10.0), (8.0, 8.0)],
            starters: 2,
            groups: vec![],
        };
        let plan = toy.plan(0, 10, 2);
        // Best: add the 12 for the useless bench player (keeps the season-relevant one).
        assert_eq!(plan[0].add, 0);
        assert_eq!(plan[0].drop, Some(Member::Mine(2)));
        assert_eq!(plan[0].backup_for, None);
        // Backups reuse the same drop, best first.
        assert_eq!((plan[1].add, plan[1].backup_for), (1, Some(0)));
        assert_eq!((plan[2].add, plan[2].backup_for), (2, Some(0)));
        assert_eq!(plan[1].drop, Some(Member::Mine(2)));
        // Nothing else improves this week once the 12 is in, so the plan ends.
        assert_eq!(plan.len(), 3);
    }

    #[test]
    fn never_trades_season_value_for_a_one_week_bump() {
        // Member 1 is on bye this week (0) but is a season starter (50).
        let toy = Toy {
            mine: vec![(20.0, 60.0), (0.0, 50.0)],
            free: vec![(15.0, 15.0)],
            starters: 2,
            groups: vec![],
        };
        assert!(toy.plan(0, 10, 2).is_empty());
        // With an open spot the same add costs nothing, so it is recommended.
        let plan = toy.plan(1, 10, 2);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].drop, None);
    }

    #[test]
    fn later_claims_assume_earlier_ones_succeeded() {
        let toy = Toy {
            mine: vec![(2.0, 2.0), (1.0, 1.0)],
            free: vec![(10.0, 10.0), (9.0, 9.0)],
            starters: 2,
            groups: vec![],
        };
        let plan = toy.plan(0, 10, 0);
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].drop, Some(Member::Mine(1)));
        // The second claim cannot reuse the first drop, and starts from the first's result.
        assert_eq!(plan[1].drop, Some(Member::Mine(0)));
        assert_eq!(plan[1].before, plan[0].after);
    }

    #[test]
    fn respects_the_claim_limit_and_backup_count() {
        let toy = Toy {
            mine: vec![(1.0, 1.0)],
            free: (0..6).map(|i| (10.0 - i as f64, 10.0 - i as f64)).collect(),
            starters: 1,
            groups: vec![],
        };
        let plan = toy.plan(0, 10, 1);
        assert_eq!(plan.len(), 2); // one claim plus one backup
        assert_eq!(toy.plan(0, 1, 3).len(), 1);
    }

    #[test]
    fn a_different_position_is_its_own_claim_not_a_backup() {
        // Two starters, two useless bench players; free agents at two positions.
        let toy = Toy {
            mine: vec![(5.0, 5.0), (4.0, 4.0), (0.5, 0.5), (0.4, 0.4)],
            free: vec![(10.0, 10.0), (9.0, 9.0)],
            starters: 2,
            groups: vec![1, 2],
        };
        let plan = toy.plan(0, 10, 2);
        assert_eq!(plan.len(), 2);
        assert!(plan.iter().all(|c| c.backup_for.is_none()));
        assert_ne!(plan[0].drop, plan[1].drop);
    }
}
