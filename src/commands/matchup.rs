//! Matchup command: win probability and the lineup most likely to win this week.
//!
//! See [`crate::espn::matchup`] for the model and why it takes ESPN's projections as given.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::{
    cli::types::position::{lineup_slot_label, SLOT_IR},
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{build_scoring_index, compute_points_for_week},
        http::{get_league_roster_data, get_league_view},
        matchup::{
            best_lineup_for_matchup, blend_player_variance, max_projection_lineup, win_probability,
            Lineup, LineupPlayer, VarianceModel,
        },
        types::{resolve_team, InjuryStatus, LeagueData, LeagueSettings, Player, Team},
    },
    storage::PlayerDatabase,
    EspnError, LeagueId, PlayerId, Position, Result, Season, Week,
};

use super::{
    common::{scoring_slot_id, ScoringIndex},
    league_data::resolve_league_id,
};

/// Parameters for the matchup command.
#[derive(Debug, Clone)]
pub struct MatchupParams {
    pub league_id: Option<LeagueId>,
    pub season: Season,
    pub week: Week,
    pub team: Option<String>,
    pub team_id: Option<u32>,
    /// Opponent override by name; otherwise taken from the league schedule.
    pub opponent: Option<String>,
    pub refresh: bool,
    pub as_json: bool,
}

/// One week of a league schedule, as returned by the `mMatchupScore` view.
#[derive(Debug, Deserialize)]
struct ScheduleResponse {
    #[serde(default)]
    schedule: Vec<ScheduledMatchup>,
}

#[derive(Debug, Deserialize)]
struct ScheduledMatchup {
    #[serde(rename = "matchupPeriodId")]
    matchup_period_id: u16,
    home: Option<MatchupSide>,
    away: Option<MatchupSide>,
}

#[derive(Debug, Deserialize)]
struct MatchupSide {
    #[serde(rename = "teamId")]
    team_id: u32,
}

/// The team `team_id` plays in `period`, if the schedule has one.
///
/// Regular-season matchup periods are one week each, so the period number is the week.
fn scheduled_opponent(schedule: &ScheduleResponse, period: u16, team_id: u32) -> Option<u32> {
    schedule
        .schedule
        .iter()
        .filter(|m| m.matchup_period_id == period)
        .find_map(|m| {
            let home = m.home.as_ref()?.team_id;
            let away = m.away.as_ref()?.team_id;
            match team_id {
                id if id == home => Some(away),
                id if id == away => Some(home),
                _ => None,
            }
        })
}

/// ESPN's projection for `player` in `week`, in this league's scoring.
pub(crate) fn weekly_projection(
    player: &Player,
    season: Season,
    week: Week,
    scoring: &ScoringIndex,
) -> f64 {
    player
        .stats
        .iter()
        .find(|s| {
            s.season_id == season
                && s.scoring_period_id == week
                && s.stat_source_id == 1
                && s.stat_split_type_id == 1
        })
        .and_then(|s| serde_json::to_value(&s.stats).ok())
        .map(|stats| {
            compute_points_for_week(
                &stats,
                scoring_slot_id(player.default_position_id as i32),
                scoring,
            )
        })
        .unwrap_or(0.0)
}

/// Everything needed to evaluate lineups for one team's week: league rules, rosters, the
/// opponent, and the spread model. Shared by the matchup and waiver commands.
pub(crate) struct MatchupContext {
    pub league_id: LeagueId,
    pub season: Season,
    pub week: Week,
    pub settings: LeagueSettings,
    pub scoring: ScoringIndex,
    pub slots: BTreeMap<u8, u32>,
    pub league: LeagueData,
    pub my_team_id: u32,
    /// This week's opponent, if the schedule (or `--opponent`) names one.
    pub opponent_id: Option<u32>,
    /// Final week of the regular season, from the league schedule.
    pub last_regular_week: u16,
    variance_model: VarianceModel,
    squared_misses: HashMap<PlayerId, Vec<f64>>,
}

impl MatchupContext {
    /// Fetch settings, rosters and schedule, resolve both teams and fit the spread model.
    pub async fn load(
        league_id: Option<LeagueId>,
        season: Season,
        week: Week,
        team: Option<&str>,
        team_id: Option<u32>,
        opponent: Option<&str>,
        refresh: bool,
    ) -> Result<Self> {
        let league_id = resolve_league_id(league_id)?;
        let settings = load_or_fetch_league_settings(league_id, false, season).await?;
        let scoring = build_scoring_index(&settings.scoring_settings.scoring_items);
        let slots = settings.starting_lineup_slots();

        let (league, _) =
            get_league_roster_data(false, league_id, season, Some(week), refresh).await?;
        let my_team_id = resolve_team(&league.teams, team_id, team)
            .ok_or_else(|| EspnError::Cache {
                message: "could not identify your team; pass --team-id or --team".to_string(),
            })?
            .id;

        let raw = get_league_view(league_id, season, &["mMatchupScore"]).await?;
        let schedule: ScheduleResponse = serde_json::from_value(raw)?;
        let last_regular_week = schedule
            .schedule
            .iter()
            .map(|m| m.matchup_period_id)
            .max()
            .unwrap_or(week.as_u16());

        let opponent_id = match opponent {
            Some(name) => league
                .teams
                .iter()
                .find(|t| t.id != my_team_id && t.name_matches(name))
                .map(|t| t.id),
            None => scheduled_opponent(&schedule, week.as_u16(), my_team_id),
        };

        // Variance curves use all earlier history; per-player spread uses this season only.
        let db = PlayerDatabase::new()?;
        let history = db.projection_history_before(season, week)?;
        let fit_rows: Vec<(Position, f64, f64)> = history
            .iter()
            .filter_map(|(_, pos, _, proj, actual)| Some((pos.parse().ok()?, *proj, *actual)))
            .collect();
        let variance_model = VarianceModel::fit(&fit_rows);
        let mut squared_misses: HashMap<PlayerId, Vec<f64>> = HashMap::new();
        for (player_id, _, row_season, proj, actual) in &history {
            if *row_season == season {
                squared_misses
                    .entry(*player_id)
                    .or_default()
                    .push((actual - proj).powi(2));
            }
        }

        Ok(Self {
            league_id,
            season,
            week,
            settings,
            scoring,
            slots,
            league,
            my_team_id,
            opponent_id,
            last_regular_week,
            variance_model,
            squared_misses,
        })
    }

    fn team(&self, id: u32) -> &Team {
        self.league
            .teams
            .iter()
            .find(|t| t.id == id)
            .expect("team ids come from this league")
    }

    pub fn me(&self) -> &Team {
        self.team(self.my_team_id)
    }

    /// This week's opponent, or an error explaining how to name one.
    pub fn opponent(&self) -> Result<&Team> {
        self.opponent_id
            .map(|id| self.team(id))
            .ok_or_else(|| EspnError::Cache {
                message: format!(
                    "no opponent found in week {}; pass --opponent to name one",
                    self.week
                ),
            })
    }

    /// ESPN's projection for `player` in `week`, zeroed for players ruled out.
    pub fn projection(&self, player: &Player, week: Week) -> f64 {
        let ruled_out = matches!(
            player.injury_status,
            Some(InjuryStatus::Out | InjuryStatus::InjuryReserve)
        );
        if ruled_out && week == self.week {
            0.0
        } else {
            weekly_projection(player, self.season, week, &self.scoring)
        }
    }

    /// A lineup candidate for `player` in `week`, with a projection and spread.
    pub fn lineup_player(
        &self,
        player: &Player,
        current_slot: u8,
        locked: bool,
        week: Week,
    ) -> Option<LineupPlayer> {
        let position = Position::from_default_position_id(player.default_position_id as u8).ok()?;
        let mean = self.projection(player, week);
        let misses = self
            .squared_misses
            .get(&PlayerId::new(player.id))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        Some(LineupPlayer {
            player_id: player.id,
            name: player
                .full_name
                .clone()
                .unwrap_or_else(|| format!("Player {}", player.id)),
            position,
            mean,
            variance: blend_player_variance(self.variance_model.variance(position, mean), misses),
            current_slot,
            locked,
            injury_status: player
                .injury_status
                .as_ref()
                .filter(|s| !matches!(s, InjuryStatus::Active | InjuryStatus::Unknown))
                .map(|s| s.as_str().to_string()),
        })
    }

    /// A team's roster as this week's lineup candidates. Players on IR are left out.
    pub fn roster_players(&self, team: &Team) -> Vec<LineupPlayer> {
        team.roster
            .iter()
            .flat_map(|r| &r.entries)
            .filter(|e| e.lineup_slot_id != SLOT_IR)
            .filter_map(|entry| {
                let pool = entry.player_pool_entry.as_ref()?;
                self.lineup_player(
                    &pool.player,
                    entry.lineup_slot_id,
                    pool.lineup_locked,
                    self.week,
                )
            })
            .collect()
    }
}

/// A lineup slot as reported in output.
#[derive(Debug, Serialize)]
struct SlotReport {
    slot: String,
    player: Option<LineupPlayer>,
}

/// A lineup's totals and players.
#[derive(Debug, Serialize)]
struct LineupReport {
    projected: f64,
    std_dev: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    win_probability: Option<f64>,
    slots: Vec<SlotReport>,
}

impl LineupReport {
    fn new(lineup: &Lineup, players: &[LineupPlayer], win_probability: Option<f64>) -> Self {
        Self {
            projected: lineup.mean,
            std_dev: lineup.std_dev(),
            win_probability,
            slots: lineup
                .assignments
                .iter()
                .map(|(slot, p)| SlotReport {
                    slot: lineup_slot_label(*slot),
                    player: p.map(|i| players[i].clone()),
                })
                .collect(),
        }
    }
}

/// Starters to add and starters to remove when moving from one lineup to another.
#[derive(Debug, Default, Serialize)]
struct LineupChanges {
    start: Vec<LineupPlayer>,
    bench: Vec<LineupPlayer>,
}

impl LineupChanges {
    fn between(from: &Lineup, to: &Lineup, players: &[LineupPlayer]) -> Self {
        let before = from.starters();
        let after = to.starters();
        Self {
            start: after
                .iter()
                .filter(|i| !before.contains(i))
                .map(|&i| players[i].clone())
                .collect(),
            bench: before
                .iter()
                .filter(|i| !after.contains(i))
                .map(|&i| players[i].clone())
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.start.is_empty() && self.bench.is_empty()
    }
}

/// Everything the matchup command reports.
#[derive(Debug, Serialize)]
struct MatchupReport {
    season: u16,
    week: u16,
    team: String,
    opponent: String,
    current: LineupReport,
    max_projection: LineupReport,
    recommended: LineupReport,
    opponent_lineup: LineupReport,
    /// Moves that raise projected points (fixes for byes, injuries, stale lineups).
    projection_moves: LineupChanges,
    /// Further moves that trade projected points for a better chance of winning.
    risk_moves: LineupChanges,
}

/// Handle the matchup command.
pub async fn handle_matchup(params: MatchupParams) -> Result<()> {
    let ctx = MatchupContext::load(
        params.league_id,
        params.season,
        params.week,
        params.team.as_deref(),
        params.team_id,
        params.opponent.as_deref(),
        params.refresh,
    )
    .await?;
    let slots = &ctx.slots;
    let mine = ctx.roster_players(ctx.me());
    let opponent = ctx.opponent()?;
    let theirs = ctx.roster_players(opponent);

    // Assume the opponent sets their best projected lineup.
    let opp = max_projection_lineup(&theirs, slots);
    let win = |l: &Lineup| win_probability(l.mean, l.variance, opp.mean, opp.variance);

    let current = Lineup::current(&mine, slots);
    let max_proj = max_projection_lineup(&mine, slots);
    let (best, best_wp) = best_lineup_for_matchup(&mine, slots, opp.mean, opp.variance);

    let report = MatchupReport {
        season: ctx.season.as_u16(),
        week: ctx.week.as_u16(),
        team: ctx.me().display_name(),
        opponent: opponent.display_name(),
        current: LineupReport::new(&current, &mine, Some(win(&current))),
        max_projection: LineupReport::new(&max_proj, &mine, Some(win(&max_proj))),
        recommended: LineupReport::new(&best, &mine, Some(best_wp)),
        opponent_lineup: LineupReport::new(&opp, &theirs, None),
        projection_moves: LineupChanges::between(&current, &max_proj, &mine),
        risk_moves: LineupChanges::between(&max_proj, &best, &mine),
    };

    if params.as_json {
        println!("{}", serde_json::to_string_pretty(&report)?); // tarpaulin::skip
    } else {
        print!("{}", report_text(&report)); // tarpaulin::skip
    }
    Ok(())
}

fn player_line(verb: &str, p: &LineupPlayer) -> String {
    let status = p
        .injury_status
        .as_deref()
        .map(|s| format!("  [{s}]"))
        .unwrap_or_default();
    format!(
        "  {verb:<6} {:<24} {:<5} {:>5.1} ±{:<4.1}{status}\n",
        p.name.chars().take(24).collect::<String>(),
        p.position.to_string(),
        p.mean,
        p.std_dev()
    )
}

fn changes_text(changes: &LineupChanges) -> String {
    let mut out = String::new();
    for p in &changes.start {
        out += &player_line("Start", p);
    }
    for p in &changes.bench {
        out += &player_line("Bench", p);
    }
    out
}

/// Render the report as text.
fn report_text(r: &MatchupReport) -> String {
    let pct = |wp: Option<f64>| {
        wp.map(|w| format!("{:>4.0}%", w * 100.0))
            .unwrap_or_default()
    };
    let row = |label: &str, l: &LineupReport| {
        format!(
            "{label:<28} {:>6.1}  ±{:<5.1} {}\n",
            l.projected,
            l.std_dev,
            pct(l.win_probability)
        )
    };

    let mut out = format!(
        "Week {} · {} vs {}\n\n{:<28} {:>6}  {:<6} {:>5}\n",
        r.week, r.team, r.opponent, "", "Proj", " ±SD", "Win"
    );
    out += &row("Your current lineup", &r.current);
    out += &row("Max projected lineup", &r.max_projection);
    out += &row("Best chance to win", &r.recommended);
    out += &row(&format!("{} (best lineup)", r.opponent), &r.opponent_lineup);
    out += "\n";

    for slot in &r.current.slots {
        match &slot.player {
            None => out += &format!("⚠ Your {} slot is empty.\n", slot.slot),
            Some(p) if p.mean <= 0.0 => {
                out += &format!(
                    "⚠ {} is in your lineup with no projection (bye or ruled out).\n",
                    p.name
                )
            }
            _ => {}
        }
    }

    let margin = r.max_projection.projected - r.opponent_lineup.projected;
    out += &if margin < 0.0 {
        format!(
            "You're projected to lose by {:.1}. Variance helps the underdog, so upside beats a steady floor.\n",
            -margin
        )
    } else {
        format!(
            "You're projected to win by {:.1}. Variance helps the underdog, so a steady floor beats upside.\n",
            margin
        )
    };

    if !r.projection_moves.is_empty() {
        out += &format!(
            "\nLineup fixes (+{:.1} projected points):\n",
            r.max_projection.projected - r.current.projected
        );
        out += &changes_text(&r.projection_moves);
    }
    if r.risk_moves.is_empty() {
        out +=
            "\nThe max projected lineup is also your best chance to win; no risk moves needed.\n";
    } else {
        out += &format!(
            "\nRisk moves (win chance {:.0}% → {:.0}%, costs {:.1} projected points):\n",
            r.max_projection.win_probability.unwrap_or(0.0) * 100.0,
            r.recommended.win_probability.unwrap_or(0.0) * 100.0,
            r.max_projection.projected - r.recommended.projected
        );
        out += &changes_text(&r.risk_moves);
    }

    out += &format!(
        "\nRecommended lineup\n  {:<6} {:<24} {:<5} {:>5}  {:<5}\n",
        "Slot", "Player", "Pos", "Proj", "±SD"
    );
    for slot in &r.recommended.slots {
        match &slot.player {
            Some(p) => {
                let flag = if p.locked { "  (locked)" } else { "" };
                out += &format!(
                    "  {:<6} {:<24} {:<5} {:>5.1}  ±{:<4.1}{flag}\n",
                    slot.slot,
                    p.name.chars().take(24).collect::<String>(),
                    p.position.to_string(),
                    p.mean,
                    p.std_dev()
                );
            }
            None => out += &format!("  {:<6} (no eligible player)\n", slot.slot),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_opponent_finds_either_side() {
        let schedule: ScheduleResponse = serde_json::from_value(serde_json::json!({
            "schedule": [
                {"matchupPeriodId": 9, "home": {"teamId": 1}, "away": {"teamId": 2}},
                {"matchupPeriodId": 10, "home": {"teamId": 3}, "away": {"teamId": 1}},
                {"matchupPeriodId": 10, "home": {"teamId": 2}, "away": {"teamId": 4}},
                // Playoff byes have no away side.
                {"matchupPeriodId": 15, "home": {"teamId": 1}},
            ]
        }))
        .unwrap();
        assert_eq!(scheduled_opponent(&schedule, 10, 1), Some(3));
        assert_eq!(scheduled_opponent(&schedule, 10, 3), Some(1));
        assert_eq!(scheduled_opponent(&schedule, 9, 2), Some(1));
        assert_eq!(scheduled_opponent(&schedule, 15, 1), None);
        assert_eq!(scheduled_opponent(&schedule, 11, 1), None);
    }

    #[test]
    fn weekly_projection_uses_the_requested_week_and_league_scoring() {
        let player: Player = serde_json::from_value(serde_json::json!({
            "id": 1,
            "defaultPositionId": 3,
            "stats": [
                {"seasonId": 2026, "scoringPeriodId": 10, "statSourceId": 1, "statSplitTypeId": 1,
                 "stats": {"53": 5.0, "42": 60.0}},
                {"seasonId": 2026, "scoringPeriodId": 10, "statSourceId": 0, "statSplitTypeId": 1,
                 "stats": {"53": 9.0}},
                {"seasonId": 2026, "scoringPeriodId": 9, "statSourceId": 1, "statSplitTypeId": 1,
                 "stats": {"53": 1.0}}
            ]
        }))
        .unwrap();
        // PPR: 1 per reception, 0.1 per receiving yard.
        let scoring: ScoringIndex =
            BTreeMap::from([(53, (1.0, BTreeMap::new())), (42, (0.1, BTreeMap::new()))]);
        let proj = weekly_projection(&player, Season::new(2026), Week::new(10), &scoring);
        assert!((proj - 11.0).abs() < 1e-9);
        assert_eq!(
            weekly_projection(&player, Season::new(2026), Week::new(11), &scoring),
            0.0
        );
    }
}
