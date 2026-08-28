//! Draft board: rank the player pool by value over replacement in this league's scoring.
//!
//! ESPN's own draft room ranks players with generic, league-agnostic rankings. This command
//! recomputes each player's season projection under the league's actual scoring settings,
//! measures it against the replacement level implied by the league's actual starting lineup,
//! and cross-references ESPN's average draft position to surface players going later than
//! their value warrants.
//!
//! With `--live` it also reads the draft as it happens, removing players already taken and
//! reporting what the drafting team still needs.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::{
    espn::{
        cache_settings::load_or_fetch_league_settings,
        compute::{
            build_scoring_index, compute_points_for_week, projected_weeks, select_season_stats,
        },
        draft::{get_draft_detail, DraftResponse},
        http::get_draft_pool,
        types::LeagueSettings,
        vor::{compute_replacement_levels, Projected, ReplacementLevels},
    },
    LeagueId, PlayerId, Position, Result, Season,
};

use super::{common::scoring_slot_id, league_data::resolve_league_id};

/// Default number of players to pull from ESPN for the board.
///
/// Deep enough to cover every draftable player plus the replacement tier behind them.
const DEFAULT_POOL_SIZE: u32 = 700;

/// Default number of rows to print.
const DEFAULT_TOP: usize = 40;

/// Configuration for the draft board command.
#[derive(Debug)]
pub struct DraftBoardParams {
    pub league_id: Option<LeagueId>,
    pub season: Season,
    pub positions: Option<Vec<Position>>,
    pub top: Option<usize>,
    pub pool_size: u32,
    pub rank_type: String,
    pub as_json: bool,
    pub live: bool,
    /// Bypass the cached draft pool and refetch projections and ADP from ESPN.
    pub refresh: bool,
    pub team: Option<String>,
    pub team_id: Option<u32>,
    pub debug: bool,
}

impl DraftBoardParams {
    pub fn new(season: Season) -> Self {
        Self {
            league_id: None,
            season,
            positions: None,
            top: None,
            pool_size: DEFAULT_POOL_SIZE,
            rank_type: "PPR".to_string(),
            as_json: false,
            live: false,
            refresh: false,
            team: None,
            team_id: None,
            debug: false,
        }
    }
}

/// One row of the draft board.
#[derive(Debug, Clone, Serialize)]
pub struct DraftBoardEntry {
    pub player_id: PlayerId,
    pub name: String,
    pub position: String,
    /// Season projection scored under this league's settings.
    pub projected_points: f64,
    /// Points above the replacement-level player at this position.
    pub value_over_replacement: f64,
    /// Rank by value over replacement, 1-based.
    pub value_rank: usize,
    /// ESPN's average draft position, when published.
    pub average_draft_position: Option<f64>,
    /// Average auction value, when published.
    pub auction_value: Option<f64>,
    /// `average_draft_position - value_rank`. Positive means the player is typically drafted
    /// later than this board values them, i.e. a bargain.
    pub adp_delta: Option<f64>,
    /// Bye week, inferred from the gap in ESPN's weekly projections.
    pub bye_week: Option<u16>,
    /// True when the player has already been taken in a live draft.
    pub drafted: bool,
    /// Fantasy team that drafted the player, when known.
    pub drafted_by: Option<String>,
}

/// The board plus the context needed to interpret it.
#[derive(Debug, Serialize)]
pub struct DraftBoard {
    pub league_name: Option<String>,
    pub season: u16,
    pub team_count: usize,
    /// The league's configured starting lineup, as slot label -> count per team.
    pub starting_lineup: Vec<(String, u32)>,
    /// How many leaguewide starters each position ends up supplying once flex slots are
    /// allocated to whoever actually fills them.
    pub starters_by_position: BTreeMap<String, usize>,
    /// Replacement-level points per position.
    pub replacement_points: BTreeMap<String, f64>,
    pub entries: Vec<DraftBoardEntry>,
    /// Live-draft context, absent for a static board.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<LiveDraftState>,
}

/// Live draft context.
#[derive(Debug, Clone, Serialize)]
pub struct LiveDraftState {
    pub drafted: bool,
    pub in_progress: bool,
    pub picks_made: usize,
    pub total_picks: usize,
    pub current_round: Option<u32>,
    pub current_overall_pick: Option<u32>,
    pub on_the_clock: Option<String>,
    /// The viewing team, when identified.
    pub my_team: Option<String>,
    pub my_team_id: Option<u32>,
    /// Positions already filled by the viewing team.
    pub my_roster: Vec<String>,
    /// Starting slots the viewing team has yet to fill.
    pub my_needs: Vec<String>,
}

/// Build and display a draft board, optionally re-reading the draft on an interval.
///
/// In watch mode the player pool is fetched once and only the draft state is re-read, since
/// projections do not change mid-draft and the pool request is by far the expensive one.
pub async fn handle_draft_board_watch(
    mut params: DraftBoardParams,
    interval_secs: u64,
) -> Result<()> {
    // Watching only makes sense against live draft state.
    params.live = true;
    let interval = std::time::Duration::from_secs(interval_secs.max(1));

    // Projections and replacement levels do not change mid-draft, and the pool request is by
    // far the expensive one, so it is fetched once and only draft state is re-read.
    let pool = ScoredPool::fetch(&params).await?;

    loop {
        // Clear the screen so each refresh replaces the previous board.
        print!("\x1b[2J\x1b[H");

        let board = pool.build_board(&params).await?;
        print_board(&board, params.top.unwrap_or(DEFAULT_TOP));

        if board.live.as_ref().is_some_and(|l| l.drafted) {
            println!("\nDraft complete — exiting watch.");
            return Ok(());
        }

        println!(
            "\nRefreshing every {}s · Ctrl-C to stop",
            interval.as_secs()
        );
        tokio::time::sleep(interval).await;
    }
}

/// Build and display a draft board.
pub async fn handle_draft_board(params: DraftBoardParams) -> Result<()> {
    let pool = ScoredPool::fetch(&params).await?;
    let board = pool.build_board(&params).await?;

    if params.as_json {
        println!("{}", serde_json::to_string_pretty(&board)?);
    } else {
        print_board(&board, params.top.unwrap_or(DEFAULT_TOP));
    }

    Ok(())
}

/// The player pool scored under league settings, plus everything derived from it that does
/// not depend on draft state.
struct ScoredPool {
    league_id: LeagueId,
    settings: LeagueSettings,
    team_count: usize,
    scored: Vec<(crate::espn::types::Player, Position, f64, Option<u16>)>,
    levels: ReplacementLevels,
}

impl ScoredPool {
    /// Fetch the pool and compute projections and replacement levels once.
    async fn fetch(params: &DraftBoardParams) -> Result<Self> {
        let league_id = resolve_league_id(params.league_id)?;
        let verbose = !params.as_json;

        if verbose {
            println!("Loading league settings...");
        }
        let settings = load_or_fetch_league_settings(league_id, false, params.season).await?;
        let scoring_index = build_scoring_index(&settings.scoring_settings.scoring_items);
        let team_count = settings.size.unwrap_or(12) as usize;

        if verbose {
            println!("Fetching player pool ({} players)...", params.pool_size);
        }
        // The pool is always fetched unfiltered: replacement level depends on which
        // positions compete for flex slots, and value rank is only comparable across the
        // whole pool. `--position` narrows the printed rows, not the arithmetic.
        let raw = get_draft_pool(
            league_id,
            params.season,
            params.pool_size,
            &params.rank_type,
            params.refresh,
            params.debug,
        )
        .await?;

        let players: Vec<crate::espn::types::Player> = serde_json::from_value(raw)?;
        if verbose {
            println!("Scoring {} players under league settings...", players.len());
        }

        let rosterable = settings.rosterable_positions();

        // Score every player's season projection under this league's rules.
        let mut scored: Vec<(crate::espn::types::Player, Position, f64, Option<u16>)> = Vec::new();
        for player in players {
            let Ok(position) = Position::from_default_position_id(
                u8::try_from(player.default_position_id).unwrap_or(u8::MAX),
            ) else {
                continue;
            };
            if !rosterable.contains(&position) {
                continue;
            }

            let Ok(value) = serde_json::to_value(&player) else {
                continue;
            };
            let Some(stats) = select_season_stats(&value, params.season.as_u16(), 1) else {
                continue;
            };

            let slot = scoring_slot_id(player.default_position_id as i32);
            let points = compute_points_for_week(stats, slot, &scoring_index);
            let bye = infer_bye_week(&value, params.season.as_u16());

            scored.push((player, position, points, bye));
        }

        // Replacement levels come from the full pool, including players already drafted: the
        // league still starts the same number of players regardless of who owns them.
        let projected: Vec<Projected> = scored
            .iter()
            .map(|(_, position, points, _)| Projected {
                position: *position,
                points: *points,
            })
            .collect();
        let levels = compute_replacement_levels(&settings, &projected, team_count);

        Ok(Self {
            league_id,
            settings,
            team_count,
            scored,
            levels,
        })
    }

    /// Combine the scored pool with current draft state into a board.
    async fn build_board(&self, params: &DraftBoardParams) -> Result<DraftBoard> {
        let draft = if params.live {
            Some(get_draft_detail(self.league_id, params.season).await?)
        } else {
            None
        };

        let taken = draft
            .as_ref()
            .map(|d| d.draft_detail.taken_players())
            .unwrap_or_default();

        let entries = build_entries(
            &self.scored,
            &self.levels,
            &taken,
            draft.as_ref(),
            params.positions.as_deref(),
        );

        Ok(self.assemble(params, entries, draft))
    }

    /// Wrap ranked entries in the surrounding league context.
    fn assemble(
        &self,
        params: &DraftBoardParams,
        entries: Vec<DraftBoardEntry>,
        draft: Option<DraftResponse>,
    ) -> DraftBoard {
        let settings = &self.settings;
        let levels = &self.levels;
        let team_count = self.team_count;

        DraftBoard {
            league_name: settings.name.clone(),
            season: params.season.as_u16(),
            team_count,
            starting_lineup: settings
                .starting_lineup_slots()
                .into_iter()
                .map(|(slot, count)| (slot_label(slot), count))
                .collect(),
            starters_by_position: levels
                .starter_counts
                .iter()
                .map(|(pos, total)| (pos.to_string(), *total))
                .collect(),
            replacement_points: levels
                .replacement_points
                .iter()
                .map(|(pos, pts)| (pos.to_string(), (pts * 10.0).round() / 10.0))
                .collect(),
            entries,
            live: draft
                .as_ref()
                .map(|d| build_live_state(d, settings, params.team.as_deref(), params.team_id)),
        }
    }
}

/// Rank scored players by value over replacement and attach draft market data.
fn build_entries(
    scored: &[(crate::espn::types::Player, Position, f64, Option<u16>)],
    levels: &ReplacementLevels,
    taken: &HashSet<PlayerId>,
    draft: Option<&DraftResponse>,
    position_filter: Option<&[Position]>,
) -> Vec<DraftBoardEntry> {
    // Who drafted whom, for live boards.
    let drafted_by: HashMap<PlayerId, String> = draft
        .map(|d| {
            d.draft_detail
                .completed_picks()
                .into_iter()
                .filter_map(|pick| {
                    let player = pick.drafted_player()?;
                    let team = d
                        .team(pick.team_id)
                        .map(|t| t.display_name())
                        .unwrap_or_else(|| format!("Team {}", pick.team_id));
                    Some((player, team))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut entries: Vec<DraftBoardEntry> = scored
        .iter()
        .map(|(player, position, points, bye)| {
            let player_id = PlayerId::new(player.id);
            DraftBoardEntry {
                player_id,
                name: player
                    .full_name
                    .clone()
                    .unwrap_or_else(|| format!("Player {}", player.id)),
                position: position.to_string(),
                projected_points: (points * 10.0).round() / 10.0,
                value_over_replacement: (levels.value_over_replacement(*position, *points) * 10.0)
                    .round()
                    / 10.0,
                value_rank: 0, // assigned after sorting
                average_draft_position: player.average_draft_position(),
                auction_value: player.auction_value(),
                adp_delta: None, // needs value_rank
                bye_week: *bye,
                drafted: taken.contains(&player_id),
                drafted_by: drafted_by.get(&player_id).cloned(),
            }
        })
        .collect();

    // Rank by value over replacement across the whole pool, before any position filter, so
    // ranks stay comparable when the board is narrowed to one position.
    entries.sort_by(|a, b| {
        b.value_over_replacement
            .partial_cmp(&a.value_over_replacement)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (i, entry) in entries.iter_mut().enumerate() {
        entry.value_rank = i + 1;
        entry.adp_delta = entry
            .average_draft_position
            .map(|adp| ((adp - entry.value_rank as f64) * 10.0).round() / 10.0);
    }

    if let Some(filter) = position_filter {
        if !filter.is_empty() {
            entries.retain(|e| {
                e.position
                    .parse::<Position>()
                    .is_ok_and(|pos| filter.iter().any(|f| slot_aware_match(*f, pos)))
            });
        }
    }

    entries
}

/// Match a position filter against a player's position, treating FLEX as slot-shaped.
fn slot_aware_match(filter: Position, position: Position) -> bool {
    match filter {
        Position::FLEX | Position::BE | Position::IR => filter
            .lineup_slot_ids()
            .iter()
            .any(|slot| position.fills_slot(*slot)),
        _ => filter == position,
    }
}

/// Infer a player's bye week from the gap in ESPN's weekly projections.
///
/// ESPN emits a projection block for every week of the season but leaves it empty on the
/// bye, so the missing week in an otherwise complete run is the bye.
fn infer_bye_week(player_value: &serde_json::Value, season: u16) -> Option<u16> {
    let weeks = projected_weeks(player_value, season);
    if weeks.is_empty() {
        return None;
    }

    let first = *weeks.first()?;
    let last = *weeks.last()?;
    let present: HashSet<u16> = weeks.into_iter().collect();

    (first..=last).find(|w| !present.contains(w))
}

/// Summarise live draft state, including the viewing team's roster and remaining needs.
fn build_live_state(
    draft: &DraftResponse,
    settings: &LeagueSettings,
    team_name: Option<&str>,
    team_id: Option<u32>,
) -> LiveDraftState {
    // Identify the viewing team: explicit id, then name, then the SWID cookie.
    let my_team = team_id
        .and_then(|id| draft.team(id))
        .or_else(|| team_name.and_then(|n| draft.team_by_name(n)))
        .or_else(|| {
            std::env::var("ESPN_SWID")
                .ok()
                .and_then(|swid| draft.team_for_owner(&swid))
        });

    let detail = &draft.draft_detail;
    let on_clock = detail.on_the_clock();

    let (my_roster, my_needs) = match my_team {
        Some(team) => {
            let picks = detail.picks_for_team(team.id);
            let roster: Vec<String> = picks
                .iter()
                .filter_map(|p| p.lineup_slot_id)
                .map(slot_label)
                .collect();
            (roster.clone(), remaining_needs(settings, &roster))
        }
        None => (Vec::new(), Vec::new()),
    };

    LiveDraftState {
        drafted: detail.drafted,
        in_progress: detail.in_progress,
        picks_made: detail.completed_picks().len(),
        total_picks: detail.picks.len(),
        current_round: on_clock.map(|p| p.round_id),
        current_overall_pick: on_clock.map(|p| p.overall_pick_number),
        on_the_clock: on_clock.and_then(|p| {
            draft
                .team(p.team_id)
                .map(|t| t.display_name())
                .or(Some(format!("Team {}", p.team_id)))
        }),
        my_team: my_team.map(|t| t.display_name()),
        my_team_id: my_team.map(|t| t.id),
        my_roster,
        my_needs,
    }
}

/// Starting slots the team has not yet filled.
///
/// Compares the league's starting lineup against the slots the team's picks were assigned to.
fn remaining_needs(settings: &LeagueSettings, filled_slots: &[String]) -> Vec<String> {
    let mut filled: HashMap<String, usize> = HashMap::new();
    for slot in filled_slots {
        *filled.entry(slot.clone()).or_insert(0) += 1;
    }

    let mut needs = Vec::new();
    for (slot, count) in settings.starting_lineup_slots() {
        let label = slot_label(slot);
        let already = filled.get(&label).copied().unwrap_or(0);
        for _ in already..count as usize {
            needs.push(label.clone());
        }
    }
    needs
}

/// Human-readable name for a lineup slot id.
fn slot_label(slot: u8) -> String {
    match slot {
        0 => "QB".to_string(),
        2 => "RB".to_string(),
        3 => "RB/WR".to_string(),
        4 => "WR".to_string(),
        5 => "WR/TE".to_string(),
        6 => "TE".to_string(),
        7 => "OP".to_string(),
        16 => "D/ST".to_string(),
        17 => "K".to_string(),
        20 => "BE".to_string(),
        21 => "IR".to_string(),
        23 => "FLEX".to_string(),
        other => format!("SLOT{}", other),
    }
}

/// Render the board as a table.
fn print_board(board: &DraftBoard, top: usize) {
    println!();
    if let Some(name) = &board.league_name {
        println!("{} · {} · {} teams", name, board.season, board.team_count);
    }

    let lineup: Vec<String> = board
        .starting_lineup
        .iter()
        .map(|(slot, n)| {
            if *n > 1 {
                format!("{}{}", slot, n)
            } else {
                slot.clone()
            }
        })
        .collect();
    if !lineup.is_empty() {
        println!("Starting lineup: {}", lineup.join(" "));
    }

    // Flex slots are handed to whichever position actually fills them, which is what moves
    // the replacement level; worth showing since it is not obvious from the lineup alone.
    let allocation: Vec<String> = board
        .starters_by_position
        .iter()
        .map(|(pos, total)| format!("{} {}", pos, total))
        .collect();
    if !allocation.is_empty() {
        println!("Starters drafted leaguewide: {}", allocation.join(" · "));
    }

    if let Some(live) = &board.live {
        print_live_header(live);
    }

    println!();
    println!(
        "{:>4}  {:<24} {:<6} {:>7} {:>8} {:>7} {:>7} {:>4}",
        "#", "Name", "Pos", "Proj", "VOR", "ADP", "Δ", "Bye"
    );
    println!("{}", "-".repeat(76));

    let mut shown = 0;
    for entry in &board.entries {
        // On a live board, players already taken are off the table.
        if entry.drafted {
            continue;
        }
        if shown >= top {
            break;
        }

        let adp = entry
            .average_draft_position
            .map(|v| format!("{:.1}", v))
            .unwrap_or_else(|| "--".to_string());
        let delta = entry
            .adp_delta
            .map(|d| format!("{:+.0}", d))
            .unwrap_or_else(|| "--".to_string());
        let bye = entry
            .bye_week
            .map(|b| b.to_string())
            .unwrap_or_else(|| "--".to_string());

        println!(
            "{:>4}  {:<24} {:<6} {:>7.1} {:>8.1} {:>7} {:>7} {:>4}",
            entry.value_rank,
            entry.name.chars().take(24).collect::<String>(),
            entry.position,
            entry.projected_points,
            entry.value_over_replacement,
            adp,
            delta,
            bye,
        );
        shown += 1;
    }

    println!();
    let replacement: Vec<String> = board
        .replacement_points
        .iter()
        .map(|(pos, pts)| format!("{} {:.0}", pos, pts))
        .collect();
    println!("Replacement level: {}", replacement.join(" · "));
    println!("Δ = ADP minus value rank; positive means the player usually goes later than this board rates them.");
}

/// Print the live-draft banner above the board.
fn print_live_header(live: &LiveDraftState) {
    println!();
    if live.drafted {
        println!("Draft complete · {} picks made", live.picks_made);
    } else {
        match (live.current_round, live.current_overall_pick) {
            (Some(round), Some(overall)) => {
                let who = live.on_the_clock.as_deref().unwrap_or("unknown");
                println!(
                    "Round {} · pick {} of {} · ON THE CLOCK: {}",
                    round, overall, live.total_picks, who
                );
            }
            _ => println!("Draft not started · {} picks scheduled", live.total_picks),
        }
    }

    if let Some(team) = &live.my_team {
        println!("You: {}", team);
        if live.my_roster.is_empty() {
            println!("Your roster: (empty)");
        } else {
            println!("Your roster: {}", live.my_roster.join(" "));
        }
        if !live.my_needs.is_empty() {
            println!("Still need: {}", live.my_needs.join(" "));
        }
    }
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

    #[test]
    fn slot_aware_match_treats_flex_as_rb_wr_te() {
        assert!(slot_aware_match(Position::FLEX, Position::RB));
        assert!(slot_aware_match(Position::FLEX, Position::WR));
        assert!(slot_aware_match(Position::FLEX, Position::TE));
        assert!(!slot_aware_match(Position::FLEX, Position::QB));
        assert!(slot_aware_match(Position::QB, Position::QB));
        assert!(!slot_aware_match(Position::QB, Position::RB));
    }

    fn espn_player(
        id: i64,
        name: &str,
        position_id: i8,
        adp: Option<f64>,
    ) -> crate::espn::types::Player {
        crate::espn::types::Player {
            id,
            full_name: Some(name.to_string()),
            default_position_id: position_id,
            stats: vec![],
            active: Some(true),
            injured: Some(false),
            injury_status: None,
            pro_team_id: None,
            ownership: adp.map(|v| crate::espn::types::Ownership {
                average_draft_position: Some(v),
                ..Default::default()
            }),
            draft_ranks: None,
        }
    }

    /// Three RBs and a QB, with the QB projecting highest in raw points.
    fn scored_pool() -> Vec<(crate::espn::types::Player, Position, f64, Option<u16>)> {
        vec![
            (
                espn_player(1, "Top RB", 2, Some(1.5)),
                Position::RB,
                300.0,
                Some(6),
            ),
            (
                espn_player(2, "Mid RB", 2, Some(20.0)),
                Position::RB,
                250.0,
                Some(7),
            ),
            (
                espn_player(3, "Low RB", 2, Some(40.0)),
                Position::RB,
                200.0,
                None,
            ),
            (
                espn_player(4, "Big QB", 1, Some(30.0)),
                Position::QB,
                380.0,
                Some(9),
            ),
        ]
    }

    fn levels_for_test() -> ReplacementLevels {
        let mut replacement_points = HashMap::new();
        // QBs are plentiful, RBs are not.
        replacement_points.insert(Position::QB, 350.0);
        replacement_points.insert(Position::RB, 150.0);
        ReplacementLevels {
            starter_counts: BTreeMap::new(),
            replacement_points,
        }
    }

    #[test]
    fn entries_rank_by_value_not_raw_points() {
        let entries = build_entries(
            &scored_pool(),
            &levels_for_test(),
            &HashSet::new(),
            None,
            None,
        );

        // The QB projects 80 more points than the top RB but is worth only 30 over
        // replacement, so it must rank below all three RBs.
        assert_eq!(entries[0].name, "Top RB");
        assert_eq!(entries[0].value_rank, 1);
        assert_eq!(entries[0].value_over_replacement, 150.0);

        let qb = entries.iter().find(|e| e.name == "Big QB").unwrap();
        assert_eq!(qb.value_over_replacement, 30.0);
        assert_eq!(
            qb.value_rank, 4,
            "highest projection should still rank last"
        );
    }

    #[test]
    fn adp_delta_flags_players_going_later_than_their_value() {
        let entries = build_entries(
            &scored_pool(),
            &levels_for_test(),
            &HashSet::new(),
            None,
            None,
        );

        // "Low RB" is the 3rd most valuable but is drafted around pick 40.
        let low = entries.iter().find(|e| e.name == "Low RB").unwrap();
        assert_eq!(low.value_rank, 3);
        assert_eq!(low.adp_delta, Some(37.0), "positive delta means a bargain");

        // "Top RB" goes almost exactly where the board rates it.
        let top = entries.iter().find(|e| e.name == "Top RB").unwrap();
        assert_eq!(top.adp_delta, Some(0.5));
    }

    #[test]
    fn drafted_players_are_marked_and_attributed() {
        let taken: HashSet<PlayerId> = [PlayerId::new(1)].into_iter().collect();
        let entries = build_entries(&scored_pool(), &levels_for_test(), &taken, None, None);

        let top = entries.iter().find(|e| e.name == "Top RB").unwrap();
        assert!(top.drafted);

        let mid = entries.iter().find(|e| e.name == "Mid RB").unwrap();
        assert!(!mid.drafted);
    }

    #[test]
    fn position_filter_preserves_overall_value_ranks() {
        let entries = build_entries(
            &scored_pool(),
            &levels_for_test(),
            &HashSet::new(),
            None,
            Some(&[Position::QB]),
        );

        assert_eq!(entries.len(), 1);
        // Rank stays the pool-wide rank so a filtered board is still comparable.
        assert_eq!(entries[0].name, "Big QB");
        assert_eq!(entries[0].value_rank, 4);
    }

    #[test]
    fn remaining_needs_subtracts_slots_already_filled() {
        use crate::espn::types::{RosterSettings, ScoringSettings};

        let mut lineup_slot_counts: HashMap<String, u32> =
            (0..=24u8).map(|s| (s.to_string(), 0)).collect();
        lineup_slot_counts.insert("0".to_string(), 1); // QB
        lineup_slot_counts.insert("2".to_string(), 2); // RB
        lineup_slot_counts.insert("20".to_string(), 5); // bench, must be excluded

        let settings = LeagueSettings {
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
        };

        // One RB already drafted: one RB and the QB remain, bench is not a "need".
        let needs = remaining_needs(&settings, &["RB".to_string()]);
        assert_eq!(needs, vec!["QB".to_string(), "RB".to_string()]);

        // Nothing drafted yet.
        let empty = remaining_needs(&settings, &[]);
        assert_eq!(empty, vec!["QB", "RB", "RB"]);

        // Bench picks do not reduce starting needs.
        let benched = remaining_needs(&settings, &["BE".to_string(), "BE".to_string()]);
        assert_eq!(benched, vec!["QB", "RB", "RB"]);
    }

    #[test]
    fn slot_label_covers_the_common_slots() {
        assert_eq!(slot_label(0), "QB");
        assert_eq!(slot_label(4), "WR");
        assert_eq!(slot_label(23), "FLEX");
        assert_eq!(slot_label(16), "D/ST");
        assert_eq!(slot_label(99), "SLOT99");
    }
}
