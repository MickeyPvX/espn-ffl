//! Waiver command: free agents ranked by how much they would help your team.
//!
//! Each free agent is valued by what actually changes if you add them and drop your least
//! useful player: the projected points your *starting lineup* gains over the rest of the
//! regular season (so a fifth-string bench WR is worth nothing, and bye weeks count), and
//! the change in your chance of winning this week's matchup.

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

/// Parameters for the waivers command.
#[derive(Debug, Clone)]
pub struct WaiversParams {
    pub league_id: Option<crate::LeagueId>,
    pub season: Season,
    pub week: Week,
    pub team: Option<String>,
    pub team_id: Option<u32>,
    pub positions: Option<Vec<Position>>,
    /// Number of recommendations to show.
    pub top: usize,
    /// Last week to count; defaults to the end of the regular season.
    pub through_week: Option<Week>,
    pub refresh: bool,
    pub as_json: bool,
}

/// One add/drop recommendation.
#[derive(Debug, Serialize)]
struct WaiverPick {
    player_id: i64,
    name: String,
    position: String,
    injury_status: Option<String>,
    /// League-wide ESPN ownership, for a sense of how contested the claim is.
    percent_owned: Option<f64>,
    /// ESPN's projection this week.
    week_projection: f64,
    /// ESPN's projected points over the evaluated weeks.
    projection_total: f64,
    /// Projected starting-lineup points gained over the evaluated weeks.
    lineup_points_gained: f64,
    /// Change in this week's win probability, when there is a matchup.
    win_probability_change: Option<f64>,
    /// Player to drop, or `None` when the roster has an open spot.
    drop: Option<String>,
}

#[derive(Debug, Serialize)]
struct WaiverReport {
    season: u16,
    from_week: u16,
    through_week: u16,
    team: String,
    picks: Vec<WaiverPick>,
}

/// A player with a lineup candidate for each evaluated week.
struct Evaluated<'a> {
    player: &'a Player,
    weekly: Vec<LineupPlayer>,
}

impl Evaluated<'_> {
    fn total(&self) -> f64 {
        self.weekly.iter().map(|p| p.mean).sum()
    }
}

/// Lineup points gained with a given drop (`None`: no drop, using an open roster spot).
struct DropOption {
    drop: Option<usize>,
    gained: f64,
    /// The dropped player's own projected points, used to break ties.
    dropped_total: f64,
}

/// Lineup gains closer than this are treated as equal when choosing a drop.
const DROP_TIE_TOLERANCE: f64 = 0.05;

/// The drop that leaves the best lineup; ties go to the open spot, then the weakest player.
///
/// Ties are common: dropping any deep bench player who would never start costs nothing in
/// lineup points, and among those the one with the lowest projection is the safest cut.
fn choose_drop(options: &[DropOption]) -> (Option<usize>, f64) {
    let best = options
        .iter()
        .map(|o| o.gained)
        .fold(f64::NEG_INFINITY, f64::max);
    options
        .iter()
        .filter(|o| o.gained >= best - DROP_TIE_TOLERANCE)
        .min_by(|a, b| {
            (a.drop.is_some(), a.dropped_total)
                .partial_cmp(&(b.drop.is_some(), b.dropped_total))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|o| (o.drop, o.gained))
        .unwrap_or((None, 0.0))
}

/// Projected starting-lineup points over all evaluated weeks for this set of players.
fn season_lineup_points(roster: &[&Evaluated], weeks: usize, ctx: &MatchupContext) -> f64 {
    (0..weeks)
        .map(|w| {
            let players: Vec<LineupPlayer> = roster.iter().map(|p| p.weekly[w].clone()).collect();
            max_projection_lineup(&players, &ctx.slots).mean
        })
        .sum()
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
    let me = ctx.me();
    let my_entries: Vec<_> = me
        .roster
        .iter()
        .flat_map(|r| &r.entries)
        .filter(|e| e.lineup_slot_id != SLOT_IR)
        .collect();
    let mine: Vec<Evaluated> = my_entries
        .iter()
        .filter_map(|e| {
            let player = by_id.get(&e.player_id).copied()?;
            Some(Evaluated {
                player,
                weekly: to_eval(player)?,
            })
        })
        .collect();
    let locked: HashSet<i64> = my_entries
        .iter()
        .filter(|e| {
            e.player_pool_entry
                .as_ref()
                .is_some_and(|p| p.lineup_locked)
        })
        .map(|e| e.player_id)
        .collect();

    // Roster spots excluding IR; an open one means a pickup needs no drop.
    let roster_spots: u32 = ctx
        .settings
        .active_lineup_slots()
        .iter()
        .filter(|(slot, _)| **slot != SLOT_IR)
        .map(|(_, count)| count)
        .sum();
    let has_open_spot = (my_entries.len() as u32) < roster_spots;

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
        .filter(|e| e.total() > 0.0)
        .collect();
    free_agents.sort_by(|a, b| b.total().total_cmp(&a.total()));
    let mut per_position: HashMap<Position, usize> = HashMap::new();
    free_agents.retain(|e| {
        let n = per_position.entry(e.weekly[0].position).or_default();
        *n += 1;
        *n <= SHORTLIST_PER_POSITION
    });

    // This week's matchup baseline, when there is one.
    let opponent_lineup = ctx.opponent().ok().map(|opp| {
        let theirs = ctx.roster_players(opp);
        max_projection_lineup(&theirs, &ctx.slots)
    });
    let this_week = ctx.roster_players(me);
    let win_with = |players: &[LineupPlayer]| {
        opponent_lineup
            .as_ref()
            .map(|opp| best_lineup_for_matchup(players, &ctx.slots, opp.mean, opp.variance).1)
    };
    let base_win = win_with(&this_week);

    let my_refs: Vec<&Evaluated> = mine.iter().collect();
    let baseline = season_lineup_points(&my_refs, weeks.len(), &ctx);

    let mut picks: Vec<WaiverPick> = free_agents
        .iter()
        .map(|fa| {
            // Try every legal drop (and no drop, given an open spot); keep the best.
            let mut options: Vec<Option<usize>> = (0..mine.len())
                .filter(|&i| !locked.contains(&mine[i].player.id))
                .map(Some)
                .collect();
            if has_open_spot {
                options.push(None);
            }
            let evaluated: Vec<DropOption> = options
                .into_iter()
                .map(|drop| {
                    let mut roster: Vec<&Evaluated> = mine
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| Some(*i) != drop)
                        .map(|(_, e)| e)
                        .collect();
                    roster.push(fa);
                    let gained = season_lineup_points(&roster, weeks.len(), &ctx) - baseline;
                    let dropped_total = drop.map(|i| mine[i].total()).unwrap_or(0.0);
                    DropOption {
                        drop,
                        gained,
                        dropped_total,
                    }
                })
                .collect();
            let (drop, gained) = choose_drop(&evaluated);

            let drop_id = drop.map(|i| mine[i].player.id);
            let mut players: Vec<LineupPlayer> = this_week
                .iter()
                .filter(|p| Some(p.player_id) != drop_id)
                .cloned()
                .collect();
            players.push(fa.weekly[0].clone());
            let win_change = win_with(&players).zip(base_win).map(|(w, b)| w - b);

            WaiverPick {
                player_id: fa.player.id,
                name: fa.weekly[0].name.clone(),
                position: fa.weekly[0].position.to_string(),
                injury_status: fa.weekly[0].injury_status.clone(),
                percent_owned: fa.player.ownership.as_ref().and_then(|o| o.percent_owned),
                week_projection: fa.weekly[0].mean,
                projection_total: fa.total(),
                lineup_points_gained: gained,
                win_probability_change: win_change,
                drop: drop.map(|i| mine[i].weekly[0].name.clone()),
            }
        })
        .filter(|p| {
            p.lineup_points_gained > 0.05 || p.win_probability_change.unwrap_or(0.0) > 0.0005
        })
        .collect();
    picks.sort_by(|a, b| b.lineup_points_gained.total_cmp(&a.lineup_points_gained));
    picks.truncate(params.top);

    let report = WaiverReport {
        season: ctx.season.as_u16(),
        from_week: ctx.week.as_u16(),
        through_week: through,
        team: me.display_name(),
        picks,
    };

    if params.as_json {
        println!("{}", serde_json::to_string_pretty(&report)?); // tarpaulin::skip
    } else {
        print!("{}", report_text(&report)); // tarpaulin::skip
    }
    Ok(())
}

/// Render the report as text.
fn report_text(r: &WaiverReport) -> String {
    let mut out = format!(
        "Waiver targets for {} · weeks {}-{}\n\n",
        r.team, r.from_week, r.through_week
    );
    if r.picks.is_empty() {
        out += "No free agent would improve your projected starting lineup.\n";
        return out;
    }
    out += &format!(
        "{:<24} {:<5} {:>6} {:>6} {:>7} {:>9} {:>7}  {}\n",
        "Player", "Pos", "Owned", "Wk Pts", "ROS Pts", "+Lineup", "+Win", "Drop"
    );
    for p in &r.picks {
        let status = p
            .injury_status
            .as_deref()
            .map(|s| format!(" [{s}]"))
            .unwrap_or_default();
        out += &format!(
            "{:<24} {:<5} {:>5.0}% {:>6.1} {:>7.1} {:>+9.1} {:>7}  {}{status}\n",
            p.name.chars().take(24).collect::<String>(),
            p.position,
            p.percent_owned.unwrap_or(0.0),
            p.week_projection,
            p.projection_total,
            p.lineup_points_gained,
            p.win_probability_change
                .map(|w| format!("{:+.1}%", w * 100.0))
                .unwrap_or_else(|| "--".to_string()),
            p.drop.as_deref().unwrap_or("(open spot)"),
        );
    }
    out += "\n+Lineup: projected points your starting lineup gains over these weeks after the add/drop.\n";
    out += "+Win: change in your chance to win this week's matchup.\n";
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(drop: Option<usize>, gained: f64, dropped_total: f64) -> DropOption {
        DropOption {
            drop,
            gained,
            dropped_total,
        }
    }

    #[test]
    fn choose_drop_prefers_the_biggest_lineup_gain() {
        let options = [option(Some(0), 2.0, 10.0), option(Some(1), 5.0, 40.0)];
        assert_eq!(choose_drop(&options), (Some(1), 5.0));
    }

    #[test]
    fn choose_drop_breaks_ties_with_the_weakest_player() {
        let options = [
            option(Some(0), 3.0, 30.0),
            option(Some(1), 3.0, 4.0),
            option(Some(2), 3.02, 12.0),
        ];
        assert_eq!(choose_drop(&options).0, Some(1));
    }

    #[test]
    fn choose_drop_uses_an_open_spot_before_cutting_anyone() {
        let options = [option(Some(0), 3.0, 0.0), option(None, 3.0, 0.0)];
        assert_eq!(choose_drop(&options).0, None);
        assert_eq!(choose_drop(&[]), (None, 0.0));
    }
}
