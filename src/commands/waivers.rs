//! Waiver command: the claims to submit this week, in priority order.
//!
//! Each claim is the add/drop that most improves your chance of winning this week's matchup,
//! assuming the claims above it went through, plus a backup in case its free agent is taken
//! by another team first. A backup drops the same player as its claim: ESPN processes claims
//! in order and cancels one whose drop player is already gone, so the backup only goes
//! through if the claim above it failed.
//!
//! Guard rails: every claim keeps a full team (no starting slot the roster could fill before
//! is left unfillable); a swap is never offered if it would cost projected starting-lineup
//! points over the rest of the regular season; injured players and players on bye are never
//! suggested as drops (their projections this week say nothing about their value); and free
//! agents who are injured (questionable included) or on bye are never suggested as adds.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::{
    cli::types::position::{unfilled_slots, SLOT_BENCH, SLOT_IR},
    espn::{
        compute::infer_bye_week,
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

/// This-week scores closer than this are treated as equal when choosing a drop.
const WEEK_TIE: f64 = 1e-6;

/// Parameters for the waivers command.
#[derive(Debug, Clone)]
pub struct WaiversParams {
    pub league_id: Option<crate::LeagueId>,
    pub season: Season,
    pub week: Week,
    pub team: Option<String>,
    pub team_id: Option<u32>,
    pub positions: Option<Vec<Position>>,
    /// Maximum number of claims to list.
    pub top: usize,
    /// Backups to list under each claim.
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
    /// Fallback adds at the same position with the same drop, best first: each assumes `add`
    /// and the backups above it were taken by other teams.
    backups: Vec<(usize, RosterScore)>,
}

/// What the planner needs to know about the league, roster and free agents.
trait ClaimModel {
    fn free_agents(&self) -> usize;
    /// Whether this member may be cut (not locked, injured or on bye).
    fn droppable(&self, member: Member) -> bool;
    /// Long-term value, to cut the weakest of otherwise equal drops.
    fn value(&self, member: Member) -> f64;
    /// Whether a free agent plays the same position as a roster member (like-for-like swap).
    fn same_position(&self, add: usize, member: Member) -> bool;
    fn score(&self, roster: &[Member]) -> RosterScore;
    /// Starting slots the roster cannot fill by position (one entry per opening).
    fn unfilled_slots(&self, roster: &[Member]) -> Vec<u8>;
}

/// Whether `after` leaves no starting slot unfillable that `before` could fill.
fn keeps_full_team(before: &[u8], after: &[u8]) -> bool {
    let count = |holes: &[u8], slot: u8| holes.iter().filter(|s| **s == slot).count();
    after
        .iter()
        .all(|&slot| count(after, slot) <= count(before, slot))
}

/// Order two candidate results: better this week first, ties within [`WEEK_TIE`].
fn cmp_week(a: f64, b: f64) -> std::cmp::Ordering {
    if (a - b).abs() < WEEK_TIE {
        std::cmp::Ordering::Equal
    } else {
        a.total_cmp(&b)
    }
}

/// Build the claim list: up to `max_claims` claims, each raising this week's objective by at
/// least `min_gain` from the roster left by the claims above it, with up to `backups`
/// fallback adds each.
fn plan_claims(
    model: &impl ClaimModel,
    mut roster: Vec<Member>,
    mut open_spots: usize,
    min_gain: f64,
    max_claims: usize,
    backups: usize,
) -> Vec<PlannedClaim> {
    let mut plan: Vec<PlannedClaim> = Vec::new();
    let mut current = model.score(&roster);

    // Result of adding `add` and dropping `drop` from the current roster, if the swap is
    // allowed: it must keep a full team, must not cost season points, and must help this week.
    let swap = |roster: &[Member], current: RosterScore, add: usize, drop: Option<Member>| {
        let mut next: Vec<Member> = roster
            .iter()
            .copied()
            .filter(|m| Some(*m) != drop)
            .collect();
        next.push(Member::Free(add));
        if !keeps_full_team(&model.unfilled_slots(roster), &model.unfilled_slots(&next)) {
            return None;
        }
        let after = model.score(&next);
        let allowed = after.season >= current.season - SEASON_TOLERANCE
            && after.week - current.week >= min_gain;
        allowed.then_some(after)
    };

    while plan.len() < max_claims {
        let available: Vec<usize> = (0..model.free_agents())
            .filter(|a| !roster.contains(&Member::Free(*a)))
            .collect();
        let mut drops: Vec<Option<Member>> = roster
            .iter()
            .copied()
            .filter(|m| model.droppable(*m))
            .map(Some)
            .collect();
        if open_spots > 0 {
            drops.push(None);
        }

        // Each free agent's best swap. Among drops that are equally good this week, prefer a
        // like-for-like swap, then the one that keeps the most season value, then the
        // weakest player.
        let rank_drop = |add: usize, d: &Option<Member>, after: &RosterScore| {
            let same = d.is_some_and(|m| model.same_position(add, m));
            let cut = d.map(|m| -model.value(m)).unwrap_or(f64::INFINITY);
            (after.week, same, after.season, cut)
        };
        let best = available
            .iter()
            .filter_map(|&add| {
                drops
                    .iter()
                    .filter_map(|&d| swap(&roster, current, add, d).map(|after| (add, d, after)))
                    .max_by(|x, y| {
                        let (wa, sa, ssa, ca) = rank_drop(add, &x.1, &x.2);
                        let (wb, sb, ssb, cb) = rank_drop(add, &y.1, &y.2);
                        cmp_week(wa, wb)
                            .then(sa.cmp(&sb))
                            .then(ssa.total_cmp(&ssb))
                            .then(ca.total_cmp(&cb))
                    })
            })
            .max_by(|x, y| cmp_week(x.2.week, y.2.week).then(x.2.season.total_cmp(&y.2.season)));

        let Some((add, drop, after)) = best else {
            break;
        };

        // Backups: the next-best adds at the same position with the same drop, standing in for
        // `add` when it (and the backups above) are claimed first by higher-priority teams.
        let mut fallback: Vec<(usize, RosterScore)> = available
            .iter()
            .filter(|&&b| b != add && model.same_position(add, Member::Free(b)))
            .filter_map(|&b| swap(&roster, current, b, drop).map(|after| (b, after)))
            .collect();
        fallback
            .sort_by(|x, y| cmp_week(y.1.week, x.1.week).then(y.1.season.total_cmp(&x.1.season)));
        fallback.truncate(backups);

        plan.push(PlannedClaim {
            add,
            drop,
            before: current,
            after,
            backups: fallback,
        });

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
    on_bye: bool,
}

impl Evaluated<'_> {
    fn total(&self) -> f64 {
        self.weekly.iter().map(|p| p.mean).sum()
    }
}

/// The real league: your roster, the free-agent shortlist, and this week's opponent.
struct League<'a> {
    mine: Vec<Evaluated<'a>>,
    free: Vec<Evaluated<'a>>,
    slots: &'a std::collections::BTreeMap<u8, u32>,
    /// Opponent's best lineup `(mean, variance)`; `None` without a matchup.
    opponent: Option<(f64, f64)>,
}

impl League<'_> {
    fn get(&self, m: Member) -> &Evaluated<'_> {
        match m {
            Member::Mine(i) => &self.mine[i],
            Member::Free(i) => &self.free[i],
        }
    }
}

impl ClaimModel for League<'_> {
    fn free_agents(&self) -> usize {
        self.free.len()
    }

    fn droppable(&self, m: Member) -> bool {
        let e = self.get(m);
        let now = &e.weekly[0];
        // Injured players are projected at zero while out, which says nothing about their
        // value once back; a bye says nothing about the rest of the season either.
        !now.locked && now.injury_status.is_none() && !e.on_bye
    }

    fn value(&self, m: Member) -> f64 {
        self.get(m).total()
    }

    fn same_position(&self, add: usize, m: Member) -> bool {
        self.free[add].weekly[0].position == self.get(m).weekly[0].position
    }

    fn score(&self, roster: &[Member]) -> RosterScore {
        let week_players = |w: usize| -> Vec<LineupPlayer> {
            roster
                .iter()
                .map(|m| self.get(*m).weekly[w].clone())
                .collect()
        };
        let now = week_players(0);
        let week = match self.opponent {
            Some((mean, var)) => best_lineup_for_matchup(&now, self.slots, mean, var).1,
            None => max_projection_lineup(&now, self.slots).mean,
        };
        let weeks = self.mine.first().map_or(1, |e| e.weekly.len());
        let season = (0..weeks)
            .map(|w| max_projection_lineup(&week_players(w), self.slots).mean)
            .sum();
        RosterScore { week, season }
    }

    fn unfilled_slots(&self, roster: &[Member]) -> Vec<u8> {
        let positions: Vec<Position> = roster
            .iter()
            .map(|m| self.get(*m).weekly[0].position)
            .collect();
        unfilled_slots(self.slots, &positions)
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
}

impl ClaimPlayer {
    fn new(e: &Evaluated) -> Self {
        let now = &e.weekly[0];
        Self {
            player_id: e.player.id,
            name: now.name.clone(),
            position: now.position.to_string(),
            projection: now.mean,
        }
    }
}

/// The backup for a claim: same drop, different add.
#[derive(Debug, Serialize)]
struct Backup {
    add: ClaimPlayer,
    after: f64,
    season_change: f64,
}

/// One waiver claim to submit.
#[derive(Debug, Serialize)]
struct Claim {
    /// Priority order, starting at 1. Submit each claim followed by its backup.
    order: usize,
    add: ClaimPlayer,
    /// `None` when the roster has an open spot.
    drop: Option<ClaimPlayer>,
    /// This week's win probability (or projected points with no matchup) before and after.
    before: f64,
    after: f64,
    /// Projected starting-lineup points gained through `through_week`.
    season_change: f64,
    /// Fallbacks in order; each assumes the adds above it were taken.
    backups: Vec<Backup>,
}

#[derive(Debug, Serialize)]
struct WaiverReport {
    season: u16,
    week: u16,
    through_week: u16,
    team: String,
    /// `None` when there is no matchup this week; claims are then ranked by projected points.
    opponent: Option<String>,
    /// Backups requested per claim.
    backups_requested: usize,
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

    let evaluate = |player: &'_ Player| -> Option<(Vec<LineupPlayer>, bool)> {
        let weekly: Vec<LineupPlayer> = weeks
            .iter()
            .map(|&w| ctx.lineup_player(player, SLOT_BENCH, false, w))
            .collect::<Option<_>>()?;
        let on_bye = serde_json::to_value(player)
            .ok()
            .and_then(|v| infer_bye_week(&v, ctx.season.as_u16()))
            == Some(ctx.week.as_u16());
        Some((weekly, on_bye))
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
            let (mut weekly, on_bye) = evaluate(player)?;
            weekly[0] = this_week[id].clone();
            Some(Evaluated {
                player,
                weekly,
                on_bye,
            })
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

    // Free agents: healthy players on no roster, at a position this league starts.
    let rostered: HashSet<i64> = ctx
        .league
        .teams
        .iter()
        .flat_map(|t| t.roster.iter().flat_map(|r| &r.entries))
        .map(|e| e.player_id)
        .collect();
    let startable = ctx.settings.rosterable_positions();
    let mut free: Vec<Evaluated> = pool
        .iter()
        .filter(|p| !rostered.contains(&p.id))
        .filter_map(|player| {
            let position =
                Position::from_default_position_id(player.default_position_id as u8).ok()?;
            let wanted = match &params.positions {
                Some(filter) => filter.contains(&position),
                None => startable.contains(&position),
            };
            if !wanted {
                return None;
            }
            let (weekly, on_bye) = evaluate(player)?;
            Some(Evaluated {
                player,
                weekly,
                on_bye,
            })
        })
        // Any injury designation (questionable included) or a bye this week rules a free
        // agent out.
        .filter(|e| e.weekly[0].injury_status.is_none() && !e.on_bye && e.weekly[0].mean > 0.0)
        .collect();
    // Shortlist by this week's projection, then by the rest of the season.
    free.sort_by(|a, b| {
        b.weekly[0]
            .mean
            .total_cmp(&a.weekly[0].mean)
            .then(b.total().total_cmp(&a.total()))
    });
    let mut per_position: HashMap<Position, usize> = HashMap::new();
    free.retain(|e| {
        let n = per_position.entry(e.weekly[0].position).or_default();
        *n += 1;
        *n <= SHORTLIST_PER_POSITION
    });

    let opponent = ctx.opponent().ok();
    let opponent_lineup = opponent.map(|opp| {
        let theirs = ctx.roster_players(opp);
        let lineup = max_projection_lineup(&theirs, &ctx.slots);
        (lineup.mean, lineup.variance)
    });

    let league = League {
        mine,
        free,
        slots: &ctx.slots,
        opponent: opponent_lineup,
    };
    let min_gain = if opponent_lineup.is_some() {
        0.001 // a tenth of a percentage point
    } else {
        0.1 // projected points
    };
    let plan = plan_claims(
        &league,
        (0..league.mine.len()).map(Member::Mine).collect(),
        open_spots,
        min_gain,
        params.top,
        params.backups,
    );

    let claims = plan
        .iter()
        .enumerate()
        .map(|(i, c)| Claim {
            order: i + 1,
            add: ClaimPlayer::new(&league.free[c.add]),
            drop: c.drop.map(|d| ClaimPlayer::new(league.get(d))),
            before: c.before.week,
            after: c.after.week,
            season_change: c.after.season - c.before.season,
            backups: c
                .backups
                .iter()
                .map(|&(b, after)| Backup {
                    add: ClaimPlayer::new(&league.free[b]),
                    after: after.week,
                    season_change: after.season - c.before.season,
                })
                .collect(),
        })
        .collect();

    let report = WaiverReport {
        season: ctx.season.as_u16(),
        week: ctx.week.as_u16(),
        through_week: through,
        team: me.display_name(),
        opponent: opponent.map(|o| o.display_name()),
        backups_requested: params.backups,
        claims,
    };

    if params.as_json {
        println!("{}", serde_json::to_string_pretty(&report)?); // tarpaulin::skip
    } else {
        print!("{}", report_text(&report)); // tarpaulin::skip
    }
    Ok(())
}

fn player_text(p: &ClaimPlayer) -> String {
    format!("{} ({} {:.1})", p.name, p.position, p.projection)
}

/// Render the report as text.
fn report_text(r: &WaiverReport) -> String {
    let has_matchup = r.opponent.is_some();
    let fmt_score = |v: f64| {
        if has_matchup {
            format!("{:.1}%", v * 100.0)
        } else {
            format!("{v:.1} pts")
        }
    };
    let change = |before: f64, after: f64| {
        let delta = if has_matchup {
            (after - before) * 100.0
        } else {
            after - before
        };
        format!("{} → {} ({delta:+.1})", fmt_score(before), fmt_score(after))
    };

    let mut out = match &r.opponent {
        Some(opp) => format!("Waiver claims · week {} · {} vs {}\n", r.week, r.team, opp),
        None => format!(
            "Waiver claims · week {} · {} (no matchup: ranked by projected points)\n",
            r.week, r.team
        ),
    };
    let Some(first) = r.claims.first() else {
        out +=
            "\nNo healthy free agent improves your lineup this week without costing you later.\n";
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

    for c in &r.claims {
        let drop = c
            .drop
            .as_ref()
            .map(|d| format!("drop {}", player_text(d)))
            .unwrap_or_else(|| "use your open roster spot".to_string());
        out += &format!(
            "{}. Add {}, {}\n   {}   season {:+.1}\n",
            c.order,
            player_text(&c.add),
            drop,
            change(c.before, c.after),
            c.season_change
        );
        let drop_name = match &c.drop {
            Some(d) => format!("drop {}", d.name),
            None => "use your open roster spot".to_string(),
        };
        let mut taken = vec![c.add.name.clone()];
        for b in &c.backups {
            out += &format!(
                "   Backup if {} {} taken: add {}, {}\n      {}   season {:+.1}\n",
                taken.join(" and "),
                if taken.len() == 1 { "is" } else { "are" },
                player_text(&b.add),
                drop_name,
                change(c.before, b.after),
                b.season_change
            );
            taken.push(b.add.name.clone());
        }
        if c.backups.is_empty() && r.backups_requested > 0 {
            out += "   No backup: every other option either doesn't help this week or would cost projected\n   points later (often a bye week this drop would leave uncovered).\n";
        }
        out += "\n";
    }

    out += &format!(
        "Submit in this order: claim 1, its backups, claim 2, its backups, and so on. Backups drop\n\
         the same player as their claim, so ESPN cancels them once one goes through. Each claim\n\
         assumes the ones above it succeeded. Season: projected starting-lineup points gained\n\
         through week {}. Every claim keeps a full team. Injured and on-bye players are never\n\
         suggested as drops or adds (questionable counts as injured).\n",
        r.through_week
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Toy model: each member has `(this week, season)` values and a position; a roster
    /// scores the sum of its best `starters` values in each.
    struct Toy {
        mine: Vec<(f64, f64, u8)>,
        free: Vec<(f64, f64, u8)>,
        starters: usize,
        undroppable: Vec<usize>,
        /// Positions the lineup needs one of each; a missing one is an unfilled slot.
        required: Vec<u8>,
    }

    impl Toy {
        fn get(&self, m: Member) -> (f64, f64, u8) {
            match m {
                Member::Mine(i) => self.mine[i],
                Member::Free(i) => self.free[i],
            }
        }

        fn plan(&self, open: usize, top: usize) -> Vec<PlannedClaim> {
            plan_claims(
                self,
                (0..self.mine.len()).map(Member::Mine).collect(),
                open,
                0.1,
                top,
                2,
            )
        }
    }

    impl ClaimModel for Toy {
        fn free_agents(&self) -> usize {
            self.free.len()
        }
        fn droppable(&self, m: Member) -> bool {
            !matches!(m, Member::Mine(i) if self.undroppable.contains(&i))
        }
        fn value(&self, m: Member) -> f64 {
            self.get(m).1
        }
        fn same_position(&self, add: usize, m: Member) -> bool {
            self.free[add].2 == self.get(m).2
        }
        fn score(&self, roster: &[Member]) -> RosterScore {
            let top = |pick: fn((f64, f64, u8)) -> f64| {
                let mut v: Vec<f64> = roster.iter().map(|m| pick(self.get(*m))).collect();
                v.sort_by(|a, b| b.total_cmp(a));
                v.iter().take(self.starters).sum::<f64>()
            };
            RosterScore {
                week: top(|v| v.0),
                season: top(|v| v.1),
            }
        }
        fn unfilled_slots(&self, roster: &[Member]) -> Vec<u8> {
            self.required
                .iter()
                .copied()
                .filter(|pos| !roster.iter().any(|m| self.get(*m).2 == *pos))
                .collect()
        }
    }

    #[test]
    fn best_claim_first_with_a_backup_that_shares_its_drop() {
        // Two starters; member 2 never starts.
        let toy = Toy {
            mine: vec![(20.0, 60.0, 1), (5.0, 20.0, 2), (1.0, 1.0, 3)],
            // Free agent 2 does not beat the starter it would replace, so it is no backup.
            free: vec![
                (12.0, 12.0, 4),
                (10.0, 10.0, 4),
                (4.0, 4.0, 4),
                (8.0, 8.0, 4),
            ],
            starters: 2,
            undroppable: vec![],
            required: vec![],
        };
        let plan = toy.plan(0, 5);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].add, 0);
        assert_eq!(plan[0].drop, Some(Member::Mine(2)));
        // If free agent 0 is taken, take free agent 1 with the same drop.
        let backups: Vec<usize> = plan[0].backups.iter().map(|b| b.0).collect();
        assert_eq!(backups, vec![1, 3]);
    }

    #[test]
    fn prefers_a_like_for_like_drop_when_this_week_is_equal() {
        // One starter. A starting kicker (pos 9), and a bench WR (pos 3) with more season
        // value than the kicker keeps once replaced; either drop gives the same week.
        let toy = Toy {
            mine: vec![(8.0, 8.0, 9), (1.0, 2.0, 3)],
            free: vec![(10.0, 10.0, 9)],
            starters: 1,
            undroppable: vec![],
            required: vec![],
        };
        let plan = toy.plan(0, 5);
        assert_eq!(plan[0].drop, Some(Member::Mine(0)));
    }

    #[test]
    fn never_trades_season_value_for_a_one_week_bump() {
        // Member 1 scores nothing this week but is a season starter.
        let toy = Toy {
            mine: vec![(20.0, 60.0, 1), (0.0, 50.0, 2)],
            free: vec![(15.0, 15.0, 3)],
            starters: 2,
            undroppable: vec![],
            required: vec![],
        };
        assert!(toy.plan(0, 5).is_empty());
        // With an open spot the add costs nothing, so it is recommended.
        let plan = toy.plan(1, 5);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].drop, None);
    }

    #[test]
    fn undroppable_players_are_never_cut() {
        let toy = Toy {
            mine: vec![(5.0, 5.0, 1), (0.0, 0.0, 1)],
            free: vec![(10.0, 10.0, 1)],
            starters: 1,
            undroppable: vec![1],
            required: vec![],
        };
        let plan = toy.plan(0, 5);
        assert_eq!(plan[0].drop, Some(Member::Mine(0)));
    }

    #[test]
    fn later_claims_assume_earlier_ones_succeeded() {
        let toy = Toy {
            mine: vec![(2.0, 2.0, 1), (1.0, 1.0, 2)],
            free: vec![(10.0, 10.0, 1), (9.0, 9.0, 2)],
            starters: 2,
            undroppable: vec![],
            required: vec![],
        };
        let plan = toy.plan(0, 5);
        assert_eq!(plan.len(), 2);
        assert_ne!(plan[0].drop, plan[1].drop);
        assert_eq!(plan[1].before, plan[0].after);
        assert_eq!(toy.plan(0, 1).len(), 1);
    }

    #[test]
    fn never_drops_the_only_player_at_a_required_position() {
        // A kicker (pos 9) is required. He is the weakest player, but dropping him for a
        // better WR (pos 3) would leave the kicker slot empty.
        let toy = Toy {
            mine: vec![(10.0, 10.0, 3), (2.0, 2.0, 9)],
            free: vec![(12.0, 12.0, 3)],
            starters: 1,
            undroppable: vec![],
            required: vec![3, 9],
        };
        let plan = toy.plan(0, 5);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].drop, Some(Member::Mine(0)));
    }

    #[test]
    fn filling_an_existing_hole_is_allowed() {
        // No kicker on the roster yet: adding one may drop anyone.
        let toy = Toy {
            mine: vec![(10.0, 10.0, 3), (1.0, 1.0, 3)],
            free: vec![(8.0, 8.0, 9)],
            starters: 2,
            undroppable: vec![],
            required: vec![3, 9],
        };
        let plan = toy.plan(0, 5);
        assert_eq!(plan[0].drop, Some(Member::Mine(1)));
    }

    #[test]
    fn keeps_full_team_compares_holes_per_slot() {
        assert!(keeps_full_team(&[], &[]));
        assert!(keeps_full_team(&[17], &[]));
        assert!(!keeps_full_team(&[], &[17]));
        // Trading one hole for a different one is not allowed either.
        assert!(!keeps_full_team(&[0], &[17]));
        assert!(keeps_full_team(&[2, 2], &[2]));
    }

    #[test]
    fn backups_stand_in_for_the_same_position() {
        // Free agent 1 (another position) helps with the same drop but is no substitute.
        let toy = Toy {
            mine: vec![(20.0, 20.0, 1), (1.0, 1.0, 2)],
            free: vec![(12.0, 12.0, 4), (11.0, 11.0, 5), (9.0, 9.0, 4)],
            starters: 2,
            undroppable: vec![],
            required: vec![],
        };
        let plan = toy.plan(0, 1);
        let backups: Vec<usize> = plan[0].backups.iter().map(|b| b.0).collect();
        assert_eq!(backups, vec![2]);
    }
}
