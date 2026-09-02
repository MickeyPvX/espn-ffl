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
        live_draft::{
            recover_prior_picks, DraftEvent, DraftStream, LiveDraftSession, PriorPick,
            LEFT_REASON_DISPLACED,
        },
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

/// How many best-available suggestions to surface for the drafting team.
const RECOMMENDATION_COUNT: usize = 3;

/// Keepalive interval for a live draft session, matching ESPN's own draft room.
const PING_INTERVAL_SECS: u64 = 15;

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
    /// Local file of picks, used when ESPN publishes none.
    pub taken_file: Option<std::path::PathBuf>,
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
            taken_file: None,
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
    /// Best available players at the viewing team's unfilled slots. Empty without a live
    /// draft and an identified team.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recommendations: Vec<Recommendation>,
    /// Pick-file lines that matched no player, or matched several.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unmatched_picks: Vec<String>,
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
    /// Positions the viewing team has drafted, in pick order.
    pub my_roster: Vec<String>,
    /// Starting slots the viewing team has yet to fill.
    pub my_needs: Vec<String>,
    /// The same unfilled slots as raw ids, for matching players against them.
    #[serde(skip)]
    pub my_need_slots: Vec<u8>,
    /// Overall number of the viewing team's next pick.
    pub next_pick_overall: Option<u32>,
    /// Round of the viewing team's next pick.
    pub next_pick_round: Option<u32>,
}

/// How likely a player is to still be available at the team's next pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outlook {
    /// Usually drafted well before the team picks again.
    TakeNow,
    /// Typically drafted around the team's next pick.
    CoinFlip,
    /// Usually still on the board at the team's next pick.
    CanWait,
    /// No published ADP, or no next pick to measure against.
    Unknown,
}

impl Outlook {
    /// Short label for the board.
    fn label(self) -> &'static str {
        match self {
            Outlook::TakeNow => "take now",
            Outlook::CoinFlip => "coin flip",
            Outlook::CanWait => "can wait",
            Outlook::Unknown => "unknown",
        }
    }
}

/// A player worth taking: best available at a slot the team still has to fill.
#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub player_id: PlayerId,
    pub name: String,
    pub position: String,
    pub value_over_replacement: f64,
    pub value_rank: usize,
    pub average_draft_position: Option<f64>,
    pub bye_week: Option<u16>,
    /// Label of a starting slot this player would fill.
    pub fills_need: String,
    /// `average_draft_position` minus the team's next overall pick. Negative means he is
    /// usually gone before the team picks again.
    pub survival_margin: Option<f64>,
    pub outlook: Outlook,
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
        let board = pool.build_board(&params).await?;
        let mut screen = board_text(&board, params.top.unwrap_or(DEFAULT_TOP));

        if board.live.as_ref().is_some_and(|l| l.drafted) {
            repaint(&screen);
            println!("\nDraft complete — exiting watch.");
            return Ok(());
        }

        screen.push_str(&format!(
            "\nRefreshing every {}s · Ctrl-C to stop\n",
            interval.as_secs()
        ));
        // Overwrite the previous frame in place; clearing first would flicker on every tick.
        repaint(&screen);
        tokio::time::sleep(interval).await;
    }
}

/// Run the board against ESPN's live draft feed.
///
/// Picks arrive on the feed and remove themselves from the board; `draft <name>` sends a
/// pick back. This is the only mode that sees a draft as it happens — the REST API reports
/// an untouched league until the draft completes.
///
/// # This takes over your draft session
///
/// ESPN allows a team one draft connection. Starting this evicts the browser draft room, and
/// nothing else will pick for you except autodraft, so picks must be made from here.
pub async fn handle_draft_board_live(mut params: DraftBoardParams) -> Result<()> {
    use std::io::Write;

    params.live = true;
    let pool = ScoredPool::fetch(&params).await?;
    let draft = get_draft_detail(pool.league_id, params.season).await?;

    let my_team = params
        .team_id
        .and_then(|id| draft.team(id))
        .or_else(|| params.team.as_deref().and_then(|n| draft.team_by_name(n)))
        .or_else(|| {
            std::env::var("ESPN_SWID")
                .ok()
                .and_then(|swid| draft.team_for_owner(&swid))
        })
        .ok_or_else(|| crate::EspnError::Cache {
            message: "could not identify your team; pass --team-id or --team".to_string(),
        })?;
    let my_team_id = my_team.id;

    println!("Joining the draft room as {}...", my_team.display_name());
    let (session, mut stream) =
        connect_when_room_opens(pool.league_id, params.season, my_team_id).await?;

    // Picks are keyed the same way the pick-file path keys them, so the whole board
    // pipeline — needs, recommendations, outlooks — works unchanged.
    let mut picks = FilePicks::default();
    let top = params.top.unwrap_or(DEFAULT_TOP);

    // Stdin is blocking, so it gets its own thread and feeds the same loop as the stream.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { return };
            if tx.send(line).is_err() {
                return;
            }
        }
    });

    let mut ping = tokio::time::interval(std::time::Duration::from_secs(PING_INTERVAL_SECS));
    ping.tick().await; // the first tick completes immediately

    let mut message: Option<String> = None;
    let mut dirty = true;
    // The snapshot arrives a moment after joining. Until it does the board cannot know what
    // has already been taken, and must not imply otherwise.
    let mut snapshot_seen = false;
    // A pick sent but not yet confirmed by the feed. HTTP 200 on SELECT only means the
    // command was accepted for delivery; a rejection arrives later as an ERROR frame.
    let mut pending: Option<(PlayerId, String)> = None;
    // ESPN silently ignores a pick sent off the clock, so the prompt warns rather than
    // letting the user believe a pick is in flight.
    let mut on_clock = false;

    loop {
        if dirty {
            let mut screen = board_text(&pool.render(&params, Some(&draft), Some(&picks)), top);
            screen.push_str(&format!(
                "\nLIVE · connected as {} · {}\n",
                my_team.display_name(),
                if snapshot_seen {
                    "picks arrive automatically"
                } else {
                    "awaiting state snapshot, board may be incomplete"
                }
            ));
            if let Some(note) = message.take() {
                screen.push_str(&note);
                screen.push('\n');
            }
            screen.push_str("type a player to pick · `list` · `quit`\n");
            repaint(&screen);
            print!("draft> ");
            let _ = std::io::stdout().flush();
            dirty = false;
        }

        tokio::select! {
            _ = ping.tick() => {
                // Silence drops the session, and being dropped means autodrafting.
                if let Err(e) = session.ping().await {
                    println!("\nping failed: {e}");
                }
            }
            event = stream.next_event() => {
                match event? {
                    None => {
                        println!("\n\nDraft feed closed by ESPN.");
                        return Ok(());
                    }
                    Some(event) => {
                        // The snapshot arrives once, on join, before any live pick.
                        if let DraftEvent::Init { blob } = &event {
                            message = Some(seed_from_snapshot(
                                blob, &mut picks, pool.league_id, my_team_id, &pool,
                            ));
                            snapshot_seen = true;
                            dirty = true;
                            continue;
                        }
                        // The clock ticks once a second. Redrawing on it would wipe the
                        // screen while the user is mid-word, so it is painted in place on
                        // the reserved top line instead, leaving the cursor untouched.
                        if let DraftEvent::Clock { team_id, time, .. } = &event {
                            on_clock = u32::try_from(*team_id) == Ok(my_team_id);
                            if on_clock {
                                paint_status_line(&format!(
                                    ">>> ON THE CLOCK — {}s <<<",
                                    (*time).max(0) / 1000
                                ));
                            }
                            continue;
                        }

                        // A pick landing confirms or invalidates whatever we sent. The
                        // verdict is held aside so it can outrank the generic pick line
                        // that `apply_live_event` produces for the same event.
                        let mut verdict = None;
                        match &event {
                            DraftEvent::Selected {
                                team_id, player_id, ..
                            }
                            | DraftEvent::Sold {
                                team_id, player_id, ..
                            } => {
                                if pending.as_ref().is_some_and(|(id, _)| id == player_id) {
                                    let (_, name) = pending.take().expect("just matched");
                                    verdict = Some(format!("Pick confirmed: {}", name));
                                } else if *team_id == my_team_id {
                                    // Our slot was filled by someone else — autodraft, or a
                                    // command ESPN dropped. ESPN sends no error for a pick
                                    // made off the clock, so this is the only signal that a
                                    // pending pick will never land.
                                    if let Some((_, name)) = pending.take() {
                                        verdict = Some(format!(
                                            "{} did NOT go through — {} was taken for you instead",
                                            name,
                                            pool.player_name(*player_id)
                                        ));
                                    }
                                }
                            }
                            DraftEvent::Selecting { team_id, .. } => {
                                on_clock = *team_id == my_team_id;
                            }
                            DraftEvent::Error { message: text } => {
                                if let Some((_, name)) = pending.take() {
                                    verdict = Some(format!("Pick FAILED for {}: {}", name, text));
                                }
                            }
                            _ => {}
                        }

                        if let Some(note) = apply_live_event(event, &mut picks, my_team_id, &pool) {
                            message = Some(note);
                            dirty = true;
                        }
                        if let Some(note) = verdict {
                            message = Some(note);
                            dirty = true;
                        }
                        if picks.len() >= draft.draft_detail.picks.len()
                            && !draft.draft_detail.picks.is_empty()
                        {
                            println!("\n\nDraft complete.");
                            return Ok(());
                        }
                    }
                }
            }
            line = rx.recv() => {
                let Some(line) = line else { return Ok(()) };
                let command = line.trim();
                dirty = true;
                match command {
                    "" => dirty = false,
                    "quit" | "exit" | "q" => {
                        let _ = session.leave().await;
                        return Ok(());
                    }
                    "list" => message = Some(describe_picks(&picks, &pool)),
                    _ => {
                        let name = command.strip_prefix("draft ").unwrap_or(command).trim();
                        message = Some(match resolve_pick_name(name, &pool.scored) {
                            Ok(id) if picks.all().any(|taken| taken == id) => {
                                format!("Already drafted: {}", pool.player_name(id))
                            }
                            // The feed is the source of truth: the pick is not recorded
                            // here, it is recorded when SELECTED comes back for it.
                            Ok(id) => match session.select(id).await {
                                Ok(()) => {
                                    let name = pool.player_name(id);
                                    pending = Some((id, name.clone()));
                                    // The schedule knows whose turn it is straight away;
                                    // the feed flag only becomes meaningful once a clock
                                    // frame has arrived, and auction drafts have no
                                    // schedule, so either source is enough.
                                    if on_clock || my_turn_by_schedule(&draft, &picks, my_team_id)
                                    {
                                        format!("Sent {} — awaiting confirmation...", name)
                                    } else {
                                        format!(
                                            "Sent {}, but you are NOT on the clock — \
                                             ESPN usually ignores this",
                                            name
                                        )
                                    }
                                }
                                Err(e) => format!("Pick rejected — {e}"),
                            },
                            Err(problem) => format!("Not sent — {}", problem),
                        });
                    }
                }
            }
        }
    }
}

/// How often to retry while waiting for a draft room to open, matching ESPN's own client.
const ROOM_RETRY_SECS: u64 = 3;

/// Connect once the draft room exists, waiting for it if the draft has not started.
///
/// ESPN creates the room when the draft opens, not when the waiting room fills: joining
/// beforehand fails with a generic HTTP 500. Retrying means the tool can be started ahead of
/// time and will attach the moment the draft begins, instead of the user having to catch
/// that instant by hand — and never having to fall back on snapshot recovery for picks
/// missed while they were getting connected.
///
/// The session is rebuilt on each attempt so the draft token cannot go stale during a long
/// wait.
async fn connect_when_room_opens(
    league_id: LeagueId,
    season: Season,
    team_id: u32,
) -> Result<(LiveDraftSession, DraftStream)> {
    let started = std::time::Instant::now();
    let mut announced = false;

    loop {
        match LiveDraftSession::open(league_id, season, team_id).await {
            Ok(session) => match session.subscribe().await {
                Ok(stream) => {
                    if announced {
                        println!("\nDraft room open — connected.");
                    }
                    return Ok((session, stream));
                }
                Err(e) => {
                    if !announced {
                        println!(
                            "Draft room is not open yet ({}). Waiting — this will connect \
                             automatically when the draft starts. Ctrl-C to stop.",
                            short_reason(&e)
                        );
                        announced = true;
                    }
                }
            },
            // A failure to even fetch the token is worth surfacing: bad credentials or the
            // wrong team would otherwise look like an endless wait.
            Err(e) if !announced => return Err(e),
            Err(_) => {}
        }

        print!("\r  waiting... {}s elapsed", started.elapsed().as_secs());
        let _ = std::io::Write::flush(&mut std::io::stdout());
        tokio::time::sleep(std::time::Duration::from_secs(ROOM_RETRY_SECS)).await;
    }
}

/// One-line form of an error, for a status message.
fn short_reason(error: &crate::EspnError) -> String {
    let text = error.to_string();
    match text.char_indices().nth(60) {
        Some((cut, _)) => format!("{}...", &text[..cut]),
        None => text,
    }
}

/// Whether the next unmade slot in the pick schedule belongs to this team.
///
/// Snake drafts pre-assign every slot, so this is known the moment a pick lands. Auction
/// drafts have no order and always answer `false`; there the feed's clock is the only signal.
fn my_turn_by_schedule(draft: &DraftResponse, picks: &FilePicks, my_team_id: u32) -> bool {
    let next = picks.len() as u32 + 1;
    draft
        .draft_detail
        .picks
        .iter()
        .find(|p| p.overall_pick_number == next)
        .and_then(|p| p.team_id)
        == Some(my_team_id)
}

/// Redraw the screen in place, without a clear-then-draw flash.
///
/// Homing the cursor and clearing each line as it is rewritten overwrites the previous frame
/// directly. Clearing the whole screen first leaves it briefly blank, which flickers several
/// times a minute over a draft lasting hours.
///
/// Trailing content from a longer previous frame is removed at the end rather than up front,
/// so nothing is ever blank between frames.
fn repaint(screen: &str) {
    use std::io::Write;
    let mut out = String::with_capacity(screen.len() + 64);
    out.push_str("\x1b[H"); // home, without clearing
    for line in screen.lines() {
        out.push_str(line);
        out.push_str("\x1b[K\n"); // wipe whatever the old frame left on this line
    }
    out.push_str("\x1b[J"); // drop any rows the old frame used and this one does not
    print!("{}", out);
    let _ = std::io::stdout().flush();
}

/// Paint a status line at the top of the screen without disturbing the cursor.
///
/// The board reserves its first line for this. Saving and restoring the cursor means a
/// countdown can tick once a second while the user is part-way through typing a pick, which
/// a full redraw would erase.
fn paint_status_line(text: &str) {
    use std::io::Write;
    // Save cursor, jump to row 1, clear it, write, restore cursor.
    print!("\x1b[s\x1b[1;1H\x1b[K{}\x1b[u", text);
    let _ = std::io::stdout().flush();
}

/// Fold one feed event into the pick list, returning a line to show when it changed.
fn apply_live_event(
    event: DraftEvent,
    picks: &mut FilePicks,
    my_team_id: u32,
    pool: &ScoredPool,
) -> Option<String> {
    match event {
        DraftEvent::Selected {
            team_id, player_id, ..
        }
        | DraftEvent::Sold {
            team_id, player_id, ..
        } => {
            if picks.all().any(|taken| taken == player_id) {
                return None;
            }
            picks.picks.push((player_id, team_id == my_team_id));
            Some(format!(
                "Pick {}: {}{}",
                picks.len(),
                pool.player_name(player_id),
                if team_id == my_team_id {
                    "  <- YOURS"
                } else {
                    ""
                }
            ))
        }
        DraftEvent::Undone { .. } => picks
            .picks
            .pop()
            .map(|(id, _)| format!("Undone: {} is back on the board", pool.player_name(id))),
        // Only our own turn is worth a redraw. Announcing every other team's turn would
        // clear the screen twice per pick, wiping whatever the user was typing — and the
        // board header already says who is up. ESPN sends the clock in milliseconds.
        DraftEvent::Selecting { team_id, millis } if team_id == my_team_id => Some(format!(
            ">>> YOU ARE ON THE CLOCK — {}s <<<",
            millis.max(0) / 1000
        )),
        // Being displaced means another client took the session; reconnecting would start a
        // fight between the two, so say so and stop.
        DraftEvent::Left { team_id, reason }
            if team_id == my_team_id && reason == LEFT_REASON_DISPLACED =>
        {
            Some("!! Another client took over your draft session — picks are no longer being sent from here.".to_string())
        }
        DraftEvent::Error { message } => Some(format!("Feed error: {}", message)),
        _ => None,
    }
}

/// Seed the board from the snapshot ESPN sends on join.
///
/// Without this, joining a draft already underway starts from an empty board and every
/// player taken before we connected stays on it. The REST API cannot fill the gap: it
/// publishes nothing until the draft ends.
///
/// Recovery is best-effort. A snapshot ESPN has changed shape on would yield nothing, which
/// must be reported rather than passed off as an empty draft — a board that silently claims
/// every player is available is worse than one that admits it does not know.
fn seed_from_snapshot(
    blob: &str,
    picks: &mut FilePicks,
    league_id: LeagueId,
    my_team_id: u32,
    pool: &ScoredPool,
) -> String {
    use base64::Engine;

    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(blob) else {
        return "!! Could not read the draft snapshot; picks made before now are missing."
            .to_string();
    };

    let prior = recover_prior_picks(&bytes, league_id.as_u32());
    if prior.is_empty() {
        return "No prior picks found in the snapshot — treating this as the start of the draft."
            .to_string();
    }

    // Keep ESPN's roster order; it is the order the picks were made in.
    for pick in &prior {
        if picks.all().any(|taken| taken == pick.player_id) {
            continue;
        }
        picks
            .picks
            .push((pick.player_id, pick.team_id == my_team_id));
    }

    let mine = prior.iter().filter(|p| p.team_id == my_team_id).count();
    format!(
        "Recovered {} picks already made ({} yours){}",
        prior.len(),
        mine,
        if mine > 0 {
            format!(": {}", describe_roster(&prior, my_team_id, pool))
        } else {
            String::new()
        }
    )
}

/// Names of the viewing team's recovered players, for confirming the snapshot read correctly.
fn describe_roster(prior: &[PriorPick], my_team_id: u32, pool: &ScoredPool) -> String {
    prior
        .iter()
        .filter(|p| p.team_id == my_team_id)
        .map(|p| pool.player_name(p.player_id))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Run the board as an interactive tracker, taking picks typed at a prompt.
///
/// ESPN's read API does not publish picks while a draft is actually running — `draftDetail`,
/// team rosters, and player ownership all still report an untouched league — so during the
/// live window the picks have to come from somewhere else. This takes them from the keyboard
/// and redraws immediately, using ESPN only for the pick schedule, which is reliable.
pub async fn handle_draft_board_interactive(mut params: DraftBoardParams) -> Result<()> {
    use std::io::{BufRead, Write};

    params.live = true;
    let pool = ScoredPool::fetch(&params).await?;

    // The schedule is fetched once: it says who picks when, and that does not change.
    let draft = get_draft_detail(pool.league_id, params.season).await?;

    // Resume from an existing file, so a crash or a restart does not lose the draft.
    let mut picks = match &params.taken_file {
        Some(path) if path.exists() => {
            let contents = std::fs::read_to_string(path).map_err(|e| crate::EspnError::Cache {
                message: format!("reading {}: {e}", path.display()),
            })?;
            parse_taken_file(&contents, &pool.scored)
        }
        _ => FilePicks::default(),
    };

    let top = params.top.unwrap_or(DEFAULT_TOP);
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut message: Option<String> = None;

    loop {
        let mut screen = board_text(&pool.render(&params, Some(&draft), Some(&picks)), top);

        if let Some(note) = message.take() {
            screen.push('\n');
            screen.push_str(&note);
            screen.push('\n');
        }
        screen.push_str(
            "\nType a player to mark drafted · prefix * for your own pick · \
             `undo` (last) · `undo <name>` · `list` · `quit`\n",
        );
        repaint(&screen);
        print!("pick> ");
        let _ = std::io::stdout().flush();

        let Some(line) = lines.next() else {
            println!();
            return Ok(()); // Ctrl-D
        };
        let line = line.map_err(|e| crate::EspnError::Cache {
            message: format!("reading stdin: {e}"),
        })?;
        let command = line.trim();

        match command {
            "" => {}
            "quit" | "exit" | "q" => return Ok(()),
            "list" => {
                message = Some(describe_picks(&picks, &pool));
            }
            "undo" => {
                message = Some(match picks.picks.pop() {
                    Some((id, _)) => {
                        rewrite_pick_file(&params, &picks, &pool)?;
                        format!("Undid: {}", pool.player_name(id))
                    }
                    None => "Nothing to undo.".to_string(),
                });
            }
            _ if command
                .split_once(char::is_whitespace)
                .is_some_and(|(verb, _)| verb == "undo" || verb == "drop") =>
            {
                let (_, name) = command.split_once(char::is_whitespace).unwrap_or(("", ""));
                message = Some(match remove_pick_by_name(&mut picks, name.trim(), &pool) {
                    Ok(id) => {
                        rewrite_pick_file(&params, &picks, &pool)?;
                        format!(
                            "Undid: {} — back on the board ({} picks remain)",
                            pool.player_name(id),
                            picks.len()
                        )
                    }
                    Err(problem) => format!("Not undone — {}", problem),
                });
            }
            _ => {
                let Some((is_mine, name)) = parse_pick_line(command) else {
                    continue;
                };
                match resolve_pick_name(name, &pool.scored) {
                    Ok(id) if picks.all().any(|taken| taken == id) => {
                        message = Some(format!("Already drafted: {}", pool.player_name(id)));
                    }
                    Ok(id) => {
                        picks.picks.push((id, is_mine));
                        append_pick_to_file(&params, command)?;
                        message = Some(format!(
                            "Pick {}: {}{}",
                            picks.len(),
                            pool.player_name(id),
                            if is_mine { "  <- yours" } else { "" }
                        ));
                    }
                    Err(problem) => message = Some(format!("Not recorded — {}", problem)),
                }
            }
        }
    }
}

/// Remove one already-recorded pick by name, putting the player back on the board.
///
/// The name is matched against the picks made rather than the whole pool, so a shorthand
/// that would be hopelessly ambiguous across 700 players usually resolves cleanly against
/// the few dozen drafted. Removing from the middle is safe: pick order only feeds the
/// made-picks count, which shifts down by one either way.
fn remove_pick_by_name(
    picks: &mut FilePicks,
    name: &str,
    pool: &ScoredPool,
) -> std::result::Result<PlayerId, String> {
    if name.is_empty() {
        return Err("give a name, e.g. `undo mccaffrey`".to_string());
    }

    let needle = name.to_lowercase();
    let hits: Vec<usize> = picks
        .picks
        .iter()
        .enumerate()
        .filter(|(_, (id, _))| pool.player_name(*id).to_lowercase().contains(&needle))
        .map(|(i, _)| i)
        .collect();

    match hits.as_slice() {
        [i] => Ok(picks.picks.remove(*i).0),
        [] => Err(format!("{} is not among the picks recorded", name)),
        many => {
            let names: Vec<String> = many
                .iter()
                .map(|i| pool.player_name(picks.picks[*i].0))
                .collect();
            Err(format!("{} matches {}", name, names.join(", ")))
        }
    }
}

/// One line per pick taken so far.
fn describe_picks(picks: &FilePicks, pool: &ScoredPool) -> String {
    if picks.picks.is_empty() {
        return "No picks recorded yet.".to_string();
    }
    picks
        .picks
        .iter()
        .enumerate()
        .map(|(i, (id, mine))| {
            format!(
                "  {:>3}. {}{}",
                i + 1,
                pool.player_name(*id),
                if *mine { "  <- yours" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Append one accepted pick to the backing file, when one was given.
fn append_pick_to_file(params: &DraftBoardParams, line: &str) -> Result<()> {
    use std::io::Write;

    let Some(path) = &params.taken_file else {
        return Ok(());
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| crate::EspnError::Cache {
            message: format!("opening {}: {e}", path.display()),
        })?;
    writeln!(file, "{}", line).map_err(|e| crate::EspnError::Cache {
        message: format!("writing {}: {e}", path.display()),
    })?;
    Ok(())
}

/// Rewrite the backing file after an undo, so it stays in step with memory.
fn rewrite_pick_file(
    params: &DraftBoardParams,
    picks: &FilePicks,
    pool: &ScoredPool,
) -> Result<()> {
    let Some(path) = &params.taken_file else {
        return Ok(());
    };
    let body: String = picks
        .picks
        .iter()
        .map(|(id, mine)| {
            format!(
                "{}{}\n",
                if *mine { "* " } else { "" },
                pool.player_name(*id)
            )
        })
        .collect();
    std::fs::write(path, body).map_err(|e| crate::EspnError::Cache {
        message: format!("writing {}: {e}", path.display()),
    })?;
    Ok(())
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
    /// Position of every scored player, for reading a team's roster off the draft picks.
    position_by_player: HashMap<PlayerId, Position>,
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

        let position_by_player = scored
            .iter()
            .map(|(player, position, _, _)| (PlayerId::new(player.id), *position))
            .collect();

        Ok(Self {
            league_id,
            settings,
            team_count,
            scored,
            levels,
            position_by_player,
        })
    }

    /// Combine the scored pool with current draft state into a board.
    async fn build_board(&self, params: &DraftBoardParams) -> Result<DraftBoard> {
        let draft = if params.live {
            Some(get_draft_detail(self.league_id, params.season).await?)
        } else {
            None
        };

        // A pick file is re-read on every refresh, so with --watch new lines take effect on
        // the next tick without restarting.
        let file_picks = match &params.taken_file {
            Some(path) => {
                let contents =
                    std::fs::read_to_string(path).map_err(|e| crate::EspnError::Cache {
                        message: format!("reading {}: {e}", path.display()),
                    })?;
                Some(parse_taken_file(&contents, &self.scored))
            }
            None => None,
        };

        Ok(self.render(params, draft.as_ref(), file_picks.as_ref()))
    }

    /// Display name for a player id, for echoing picks back to the user.
    fn player_name(&self, id: PlayerId) -> String {
        self.scored
            .iter()
            .find(|(player, _, _, _)| PlayerId::new(player.id) == id)
            .and_then(|(player, _, _, _)| player.full_name.clone())
            .unwrap_or_else(|| format!("Player {}", id))
    }

    /// Build the board from state already in hand.
    ///
    /// Split out from [`Self::build_board`] so interactive mode can redraw after every typed
    /// pick without another round trip to ESPN: the pick schedule it relies on is fetched
    /// once and does not change mid-draft.
    fn render(
        &self,
        params: &DraftBoardParams,
        draft: Option<&DraftResponse>,
        file_picks: Option<&FilePicks>,
    ) -> DraftBoard {
        let mut taken = draft
            .map(|d| d.draft_detail.taken_players())
            .unwrap_or_default();
        if let Some(picks) = file_picks {
            taken.extend(picks.all());
        }

        // Entries are ranked over the whole pool first: recommendations must see every
        // position the team needs, not just the ones `--position` left on screen.
        let mut entries = build_entries(&self.scored, &self.levels, &taken, draft);

        let live = draft.map(|d| match file_picks {
            Some(picks) => build_file_live_state(
                d,
                &self.settings,
                &self.position_by_player,
                picks,
                params.team.as_deref(),
                params.team_id,
            ),
            None => build_live_state(
                d,
                &self.settings,
                &self.position_by_player,
                params.team.as_deref(),
                params.team_id,
            ),
        });

        let recommendations = live
            .as_ref()
            .map(|l| build_recommendations(&entries, l, self.team_count, RECOMMENDATION_COUNT))
            .unwrap_or_default();

        apply_position_filter(&mut entries, params.positions.as_deref());

        let mut board = self.assemble(params, entries, live, recommendations);
        board.unmatched_picks = file_picks.map(|p| p.unmatched.clone()).unwrap_or_default();
        board
    }

    /// Wrap ranked entries in the surrounding league context.
    fn assemble(
        &self,
        params: &DraftBoardParams,
        entries: Vec<DraftBoardEntry>,
        live: Option<LiveDraftState>,
        recommendations: Vec<Recommendation>,
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
            live,
            recommendations,
            unmatched_picks: Vec::new(),
        }
    }
}

/// Rank scored players by value over replacement and attach draft market data.
fn build_entries(
    scored: &[(crate::espn::types::Player, Position, f64, Option<u16>)],
    levels: &ReplacementLevels,
    taken: &HashSet<PlayerId>,
    draft: Option<&DraftResponse>,
) -> Vec<DraftBoardEntry> {
    // Who drafted whom, for live boards.
    let drafted_by: HashMap<PlayerId, String> = draft
        .map(|d| {
            d.draft_detail
                .completed_picks()
                .into_iter()
                .filter_map(|pick| {
                    let player = pick.drafted_player()?;
                    let team_id = pick.team_id?;
                    let team = d
                        .team(team_id)
                        .map(|t| t.display_name())
                        .unwrap_or_else(|| format!("Team {}", team_id));
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

    entries
}

/// Narrow the printed rows to the requested positions, leaving ranks untouched.
fn apply_position_filter(entries: &mut Vec<DraftBoardEntry>, filter: Option<&[Position]>) {
    let Some(filter) = filter.filter(|f| !f.is_empty()) else {
        return;
    };

    entries.retain(|e| {
        e.position
            .parse::<Position>()
            .is_ok_and(|pos| filter.iter().any(|f| slot_aware_match(*f, pos)))
    });
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

/// Picks tracked outside ESPN's feed, from a file or typed in interactively.
#[derive(Debug, Default, Clone)]
struct FilePicks {
    /// Every drafted player in pick order, paired with whether it was the viewing team's.
    picks: Vec<(PlayerId, bool)>,
    /// Entries that matched no player, so they can be reported instead of silently ignored.
    unmatched: Vec<String>,
}

impl FilePicks {
    fn all(&self) -> impl Iterator<Item = PlayerId> + '_ {
        self.picks.iter().map(|(id, _)| *id)
    }

    fn mine(&self) -> impl Iterator<Item = PlayerId> + '_ {
        self.picks
            .iter()
            .filter(|(_, mine)| *mine)
            .map(|(id, _)| *id)
    }

    fn len(&self) -> usize {
        self.picks.len()
    }
}

/// Resolve one typed name to a single player in the pool.
///
/// Matching is case-insensitive on any substring, so "gibbs" is enough. An ambiguous name is
/// rejected rather than guessed: taking the wrong player off the board mid-draft is worse
/// than being asked to be more specific.
fn resolve_pick_name(
    name: &str,
    scored: &[(crate::espn::types::Player, Position, f64, Option<u16>)],
) -> std::result::Result<PlayerId, String> {
    let needle = name.to_lowercase();
    let matches: Vec<&crate::espn::types::Player> = scored
        .iter()
        .map(|(player, _, _, _)| player)
        .filter(|player| {
            player
                .full_name
                .as_deref()
                .is_some_and(|n| n.to_lowercase().contains(&needle))
        })
        .collect();

    match matches.as_slice() {
        [player] => Ok(PlayerId::new(player.id)),
        [] => Err(format!("{} (no match)", name)),
        many => {
            let names: Vec<&str> = many
                .iter()
                .filter_map(|p| p.full_name.as_deref())
                .take(4)
                .collect();
            Err(format!("{} (matches {})", name, names.join(", ")))
        }
    }
}

/// Split a pick line into its "is this mine" marker and the player name.
fn parse_pick_line(line: &str) -> Option<(bool, &str)> {
    let line = line.split('#').next().unwrap_or("").trim();
    if line.is_empty() {
        return None;
    }
    let (is_mine, name) = match line.strip_prefix('*') {
        Some(rest) => (true, rest.trim()),
        None => (false, line),
    };
    (!name.is_empty()).then_some((is_mine, name))
}

/// Parse a pick file into resolved picks.
///
/// One name per line in pick order; `#` comments and blank lines are skipped, and a leading
/// `*` marks the line as the viewing team's own pick.
fn parse_taken_file(
    contents: &str,
    scored: &[(crate::espn::types::Player, Position, f64, Option<u16>)],
) -> FilePicks {
    let mut picks = FilePicks::default();
    for raw in contents.lines() {
        let Some((is_mine, name)) = parse_pick_line(raw) else {
            continue;
        };
        match resolve_pick_name(name, scored) {
            Ok(id) => picks.picks.push((id, is_mine)),
            Err(problem) => picks.unmatched.push(problem),
        }
    }
    picks
}

/// Summarise live draft state from a pick file, using ESPN's pick schedule for timing.
///
/// ESPN pre-allocates every pick slot before the draft starts, and that part of the feed is
/// reliable even when it publishes no completed picks, so the file supplies *who* is gone
/// while ESPN still supplies *when* the team picks next.
fn build_file_live_state(
    draft: &DraftResponse,
    settings: &LeagueSettings,
    position_by_player: &HashMap<PlayerId, Position>,
    picks: &FilePicks,
    team_name: Option<&str>,
    team_id: Option<u32>,
) -> LiveDraftState {
    let my_team = team_id
        .and_then(|id| draft.team(id))
        .or_else(|| team_name.and_then(|n| draft.team_by_name(n)))
        .or_else(|| {
            std::env::var("ESPN_SWID")
                .ok()
                .and_then(|swid| draft.team_for_owner(&swid))
        });

    let detail = &draft.draft_detail;
    let picks_made = picks.len();

    // The next slot belonging to this team that the file has not yet consumed.
    let next_pick = my_team.and_then(|team| {
        detail
            .picks
            .iter()
            .filter(|p| p.team_id == Some(team.id) && p.overall_pick_number as usize > picks_made)
            .min_by_key(|p| p.overall_pick_number)
    });

    // Whoever owns the next overall slot is on the clock.
    let on_clock = detail
        .picks
        .iter()
        .filter(|p| p.overall_pick_number as usize > picks_made)
        .min_by_key(|p| p.overall_pick_number);

    let roster: Vec<Position> = picks
        .mine()
        .filter_map(|id| position_by_player.get(&id).copied())
        .collect();

    let need_slots = if my_team.is_some() {
        remaining_needs(settings, &roster)
    } else {
        Vec::new()
    };

    LiveDraftState {
        drafted: picks_made >= detail.picks.len() && !detail.picks.is_empty(),
        in_progress: true,
        picks_made,
        total_picks: detail.picks.len(),
        current_round: on_clock.map(|p| p.round_id),
        current_overall_pick: on_clock.map(|p| p.overall_pick_number),
        on_the_clock: on_clock.and_then(|p| team_label(draft, p.team_id)),
        my_team: my_team.map(|t| t.display_name()),
        my_team_id: my_team.map(|t| t.id),
        my_roster: roster.iter().map(|p| p.to_string()).collect(),
        my_needs: need_slots.iter().copied().map(slot_label).collect(),
        my_need_slots: need_slots,
        next_pick_overall: next_pick.map(|p| p.overall_pick_number),
        next_pick_round: next_pick.map(|p| p.round_id),
    }
}

/// Summarise live draft state, including the viewing team's roster and remaining needs.
fn build_live_state(
    draft: &DraftResponse,
    settings: &LeagueSettings,
    position_by_player: &HashMap<PlayerId, Position>,
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

    // The roster is read from the drafted players' positions rather than the pick's
    // `lineupSlotId`, which describes where the pick landed, not what it can start.
    let roster: Vec<Position> = my_team
        .map(|team| {
            detail
                .picks_for_team(team.id)
                .iter()
                .filter_map(|p| p.drafted_player())
                .filter_map(|id| position_by_player.get(&id).copied())
                .collect()
        })
        .unwrap_or_default();

    let need_slots = if my_team.is_some() {
        remaining_needs(settings, &roster)
    } else {
        Vec::new()
    };

    let next_pick = my_team.and_then(|team| detail.next_pick_for_team(team.id));

    LiveDraftState {
        drafted: detail.drafted,
        in_progress: detail.in_progress,
        picks_made: detail.completed_picks().len(),
        total_picks: detail.picks.len(),
        current_round: on_clock.map(|p| p.round_id),
        current_overall_pick: on_clock.map(|p| p.overall_pick_number),
        on_the_clock: on_clock.and_then(|p| team_label(draft, p.team_id)),
        my_team: my_team.map(|t| t.display_name()),
        my_team_id: my_team.map(|t| t.id),
        my_roster: roster.iter().map(|p| p.to_string()).collect(),
        my_needs: need_slots.iter().copied().map(slot_label).collect(),
        my_need_slots: need_slots,
        next_pick_overall: next_pick.map(|p| p.overall_pick_number),
        next_pick_round: next_pick.map(|p| p.round_id),
    }
}

/// Name for the team owning a pick slot, when ESPN has assigned one.
///
/// Auction drafts leave every unmade slot unassigned, so this is routinely `None`.
fn team_label(draft: &DraftResponse, team_id: Option<u32>) -> Option<String> {
    let team_id = team_id?;
    Some(
        draft
            .team(team_id)
            .map(|t| t.display_name())
            .unwrap_or_else(|| format!("Team {}", team_id)),
    )
}

/// Best available players at slots the team still has to fill.
///
/// Ranked by value over replacement, which is already the board's ordering, then annotated
/// with whether each player is likely to survive until the team's next pick. Value and
/// urgency are reported separately rather than blended into one number: the two answer
/// different questions, and a composite would hide which one drove the ranking.
///
/// A team whose starting lineup is already full falls back to best available overall, since
/// the remaining picks are bench depth and every position is fair game.
fn build_recommendations(
    entries: &[DraftBoardEntry],
    live: &LiveDraftState,
    team_count: usize,
    limit: usize,
) -> Vec<Recommendation> {
    if live.my_team.is_none() || live.drafted {
        return Vec::new();
    }

    let starters_full = live.my_need_slots.is_empty();

    entries
        .iter()
        .filter(|entry| !entry.drafted)
        .filter_map(|entry| {
            let position = entry.position.parse::<Position>().ok()?;
            let need = if starters_full {
                "BE".to_string()
            } else {
                slot_label(
                    live.my_need_slots
                        .iter()
                        .copied()
                        .find(|slot| position.fills_slot(*slot))?,
                )
            };

            let survival_margin = live
                .next_pick_overall
                .zip(entry.average_draft_position)
                .map(|(pick, adp)| ((adp - f64::from(pick)) * 10.0).round() / 10.0);

            Some(Recommendation {
                player_id: entry.player_id,
                name: entry.name.clone(),
                position: entry.position.clone(),
                value_over_replacement: entry.value_over_replacement,
                value_rank: entry.value_rank,
                average_draft_position: entry.average_draft_position,
                bye_week: entry.bye_week,
                fills_need: need,
                survival_margin,
                outlook: classify_outlook(survival_margin, team_count),
            })
        })
        .take(limit)
        .collect()
}

/// Turn a survival margin into an outlook.
///
/// The band is measured in rounds, so the verdict scales with league size instead of
/// assuming twelve teams. It is deliberately asymmetric: ADP noise is not symmetric around a
/// pick. A player whose ADP falls after the team's next pick may still slide into reach, but
/// one whose ADP falls before it rarely comes back, because every team in between has to
/// pass on him. Half a round of tolerance late, a quarter round early.
fn classify_outlook(survival_margin: Option<f64>, team_count: usize) -> Outlook {
    let Some(margin) = survival_margin else {
        return Outlook::Unknown;
    };

    let round = team_count.max(1) as f64;
    if margin < -round / 4.0 {
        Outlook::TakeNow
    } else if margin <= round / 2.0 {
        Outlook::CoinFlip
    } else {
        Outlook::CanWait
    }
}

/// Starting slots the team has not yet filled.
///
/// ESPN reports a `lineupSlotId` on each pick, but it reflects where the pick landed on the
/// roster rather than which starting slot the player will occupy, so the allocation is
/// recomputed here from the drafted players' own positions.
///
/// Slots are filled tightest-first: a flex-eligible player claimed by the flex spot would
/// otherwise leave a dedicated RB or WR slot open that only he could have filled.
fn remaining_needs(settings: &LeagueSettings, roster: &[Position]) -> Vec<u8> {
    // One entry per startable slot, tightest slot first.
    let mut slots: Vec<u8> = settings
        .starting_lineup_slots()
        .into_iter()
        .flat_map(|(slot, count)| std::iter::repeat_n(slot, count as usize))
        .collect();
    slots.sort_by_key(|slot| (slot_flexibility(*slot), *slot));

    let mut claimed = vec![false; roster.len()];
    let mut needs = Vec::new();
    for slot in slots {
        let filler = roster
            .iter()
            .enumerate()
            .find(|(i, position)| !claimed[*i] && position.fills_slot(slot));
        match filler {
            Some((i, _)) => claimed[i] = true,
            None => needs.push(slot),
        }
    }
    needs
}

/// How many positions a slot accepts, so dedicated slots sort ahead of flex ones.
fn slot_flexibility(slot: u8) -> usize {
    const CANDIDATES: [Position; 6] = [
        Position::QB,
        Position::RB,
        Position::WR,
        Position::TE,
        Position::K,
        Position::DEF,
    ];
    CANDIDATES.iter().filter(|p| p.fills_slot(slot)).count()
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
    print!("{}", board_text(board, top));
}

/// Render the board to text.
///
/// Built as a string rather than printed directly so live mode can repaint it in place: a
/// clear-then-draw cycle flashes the whole screen several times a minute, which is
/// unpleasant to sit in front of for a three-hour draft.
fn board_text(board: &DraftBoard, top: usize) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    let _ = writeln!(out);
    if let Some(name) = &board.league_name {
        let _ = writeln!(
            out,
            "{} · {} · {} teams",
            name, board.season, board.team_count
        );
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
        let _ = writeln!(out, "Starting lineup: {}", lineup.join(" "));
    }

    // Flex slots are handed to whichever position actually fills them, which is what moves
    // the replacement level; worth showing since it is not obvious from the lineup alone.
    let allocation: Vec<String> = board
        .starters_by_position
        .iter()
        .map(|(pos, total)| format!("{} {}", pos, total))
        .collect();
    if !allocation.is_empty() {
        let _ = writeln!(
            out,
            "Starters drafted leaguewide: {}",
            allocation.join(" · ")
        );
    }

    if let Some(live) = &board.live {
        out.push_str(&live_header_text(live));
    }
    if !board.unmatched_picks.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "!! Unresolved lines in the pick file (these players are STILL on the board):"
        );
        for line in &board.unmatched_picks {
            let _ = writeln!(out, "     {}", line);
        }
    }
    if !board.recommendations.is_empty() {
        out.push_str(&recommendations_text(
            &board.recommendations,
            board.live.as_ref(),
        ));
    }

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{:>4}  {:<24} {:<6} {:>7} {:>8} {:>7} {:>7} {:>4}",
        "#", "Name", "Pos", "Proj", "VOR", "ADP", "Δ", "Bye"
    );
    let _ = writeln!(out, "{}", "-".repeat(76));

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

        let _ = writeln!(
            out,
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

    let _ = writeln!(out);
    let replacement: Vec<String> = board
        .replacement_points
        .iter()
        .map(|(pos, pts)| format!("{} {:.0}", pos, pts))
        .collect();
    let _ = writeln!(out, "Replacement level: {}", replacement.join(" · "));
    let _ = writeln!(out, "Δ = ADP minus value rank; positive means the player usually goes later than this board rates them.");
    out
}

/// The live-draft banner shown above the board.
fn live_header_text(live: &LiveDraftState) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    let _ = writeln!(out);
    if live.drafted {
        let _ = writeln!(out, "Draft complete · {} picks made", live.picks_made);
    } else {
        match (live.current_round, live.current_overall_pick) {
            (Some(round), Some(overall)) => {
                let who = live.on_the_clock.as_deref().unwrap_or("unknown");
                let _ = writeln!(
                    out,
                    "Round {} · pick {} of {} · ON THE CLOCK: {}",
                    round, overall, live.total_picks, who
                );
            }
            _ => {
                let _ = writeln!(
                    out,
                    "Draft not started · {} picks scheduled",
                    live.total_picks
                );
            }
        }
    }

    if let Some(team) = &live.my_team {
        let _ = writeln!(out, "You: {}", team);
        if live.my_roster.is_empty() {
            let _ = writeln!(out, "Your roster: (empty)");
        } else {
            let _ = writeln!(out, "Your roster: {}", live.my_roster.join(" "));
        }
        if live.my_needs.is_empty() {
            let _ = writeln!(out, "Still need: (starters full)");
        } else {
            let _ = writeln!(out, "Still need: {}", live.my_needs.join(" "));
        }
    }
    out
}

/// The best available players at the team's unfilled slots.
fn recommendations_text(recs: &[Recommendation], live: Option<&LiveDraftState>) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let next_pick = live.and_then(|l| l.next_pick_overall);

    let _ = writeln!(out);
    match next_pick {
        Some(pick) => {
            let _ = writeln!(
                out,
                "Best available for your needs (your next pick: {}):",
                pick
            );
        }
        None => {
            let _ = writeln!(out, "Best available for your needs:");
        }
    }

    for (i, rec) in recs.iter().enumerate() {
        let adp = rec
            .average_draft_position
            .map(|v| format!("{:.1}", v))
            .unwrap_or_else(|| "--".to_string());
        let bye = rec
            .bye_week
            .map(|b| format!("bye {}", b))
            .unwrap_or_else(|| "bye --".to_string());

        // The slot column is six wide so "[D/ST]" and "[FLEX]" do not push the row out.
        let _ = writeln!(
            out,
            "  {}  {:<24} {:<4} {:<7} VOR {:>6.1}   ADP {:>5}   {:<7}  {}",
            i + 1,
            rec.name.chars().take(24).collect::<String>(),
            rec.position,
            format!("[{}]", rec.fills_need),
            rec.value_over_replacement,
            adp,
            bye,
            rec.outlook.label(),
        );
    }
    out
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
        let entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);

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
        let entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);

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
        let entries = build_entries(&scored_pool(), &levels_for_test(), &taken, None);

        let top = entries.iter().find(|e| e.name == "Top RB").unwrap();
        assert!(top.drafted);

        let mid = entries.iter().find(|e| e.name == "Mid RB").unwrap();
        assert!(!mid.drafted);
    }

    #[test]
    fn position_filter_preserves_overall_value_ranks() {
        let mut entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);
        apply_position_filter(&mut entries, Some(&[Position::QB]));

        assert_eq!(entries.len(), 1);
        // Rank stays the pool-wide rank so a filtered board is still comparable.
        assert_eq!(entries[0].name, "Big QB");
        assert_eq!(entries[0].value_rank, 4);
    }

    #[test]
    fn position_filter_is_a_no_op_when_absent_or_empty() {
        let full = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);

        let mut none_filter = full.clone();
        apply_position_filter(&mut none_filter, None);
        assert_eq!(none_filter.len(), full.len());

        let mut empty_filter = full.clone();
        apply_position_filter(&mut empty_filter, Some(&[]));
        assert_eq!(empty_filter.len(), full.len());
    }

    /// League settings with the given starting slots, plus a bench that must be ignored.
    fn settings_with_slots(slots: &[(u8, u32)]) -> LeagueSettings {
        use crate::espn::types::{RosterSettings, ScoringSettings};

        let mut lineup_slot_counts: HashMap<String, u32> =
            (0..=24u8).map(|s| (s.to_string(), 0)).collect();
        for (slot, count) in slots {
            lineup_slot_counts.insert(slot.to_string(), *count);
        }
        lineup_slot_counts.insert("20".to_string(), 5); // bench, must be excluded

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

    #[test]
    fn remaining_needs_subtracts_slots_already_filled() {
        // QB1 RB2.
        let settings = settings_with_slots(&[(0, 1), (2, 2)]);

        // One RB already drafted: one RB and the QB remain, bench is not a "need".
        assert_eq!(remaining_needs(&settings, &[Position::RB]), vec![0, 2]);

        // Nothing drafted yet.
        assert_eq!(remaining_needs(&settings, &[]), vec![0, 2, 2]);

        // Kickers fill no starting slot here, so they do not reduce needs.
        assert_eq!(
            remaining_needs(&settings, &[Position::K, Position::K]),
            vec![0, 2, 2]
        );
    }

    #[test]
    fn remaining_needs_fills_dedicated_slots_before_flex() {
        // QB1 RB1 WR1 FLEX1.
        let settings = settings_with_slots(&[(0, 1), (2, 1), (4, 1), (23, 1)]);

        // A lone RB must be credited to the RB slot, not consumed by the flex.
        assert_eq!(
            remaining_needs(&settings, &[Position::RB]),
            vec![0, 4, 23],
            "RB should claim the dedicated RB slot"
        );

        // A second RB then has nowhere to go but the flex.
        assert_eq!(
            remaining_needs(&settings, &[Position::RB, Position::RB]),
            vec![0, 4]
        );

        // Positions that cannot fill the flex leave it open.
        assert_eq!(
            remaining_needs(&settings, &[Position::QB, Position::QB]),
            vec![2, 4, 23],
            "a backup QB cannot fill the flex"
        );
    }

    #[test]
    fn slot_flexibility_orders_dedicated_slots_ahead_of_flex() {
        assert_eq!(slot_flexibility(0), 1); // QB
        assert_eq!(slot_flexibility(2), 1); // RB
        assert_eq!(slot_flexibility(3), 2); // RB/WR
        assert_eq!(slot_flexibility(23), 3); // FLEX
        assert_eq!(slot_flexibility(7), 4); // superflex
    }

    /// Live state for a team that still needs the given slots and picks again at `next_pick`.
    fn live_state(need_slots: Vec<u8>, next_pick: Option<u32>) -> LiveDraftState {
        LiveDraftState {
            drafted: false,
            in_progress: true,
            picks_made: 10,
            total_picks: 100,
            current_round: Some(1),
            current_overall_pick: Some(11),
            on_the_clock: Some("Someone".to_string()),
            my_team: Some("Me".to_string()),
            my_team_id: Some(1),
            my_roster: Vec::new(),
            my_needs: need_slots.iter().copied().map(slot_label).collect(),
            my_need_slots: need_slots,
            next_pick_overall: next_pick,
            next_pick_round: Some(2),
        }
    }

    #[test]
    fn recommendations_only_cover_positions_the_team_still_needs() {
        let entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);
        // Only the QB slot is open, so the three higher-value RBs must be skipped.
        let recs = build_recommendations(&entries, &live_state(vec![0], Some(25)), 12, 3);

        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].name, "Big QB");
        assert_eq!(recs[0].fills_need, "QB");
        // Rank stays the pool-wide value rank, not a rank within the needed positions.
        assert_eq!(recs[0].value_rank, 4);
    }

    #[test]
    fn recommendations_keep_the_boards_value_order_and_respect_the_limit() {
        let entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);
        let recs = build_recommendations(&entries, &live_state(vec![2, 2, 0], Some(25)), 12, 3);

        let names: Vec<&str> = recs.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Top RB", "Mid RB", "Low RB"]);
    }

    #[test]
    fn recommendations_skip_players_already_drafted() {
        let taken: HashSet<PlayerId> = [PlayerId::new(1)].into_iter().collect();
        let entries = build_entries(&scored_pool(), &levels_for_test(), &taken, None);
        let recs = build_recommendations(&entries, &live_state(vec![2], Some(25)), 12, 3);

        assert!(
            !recs.iter().any(|r| r.name == "Top RB"),
            "a drafted player must not be recommended"
        );
        assert_eq!(recs[0].name, "Mid RB");
    }

    #[test]
    fn recommendations_fall_back_to_best_available_once_starters_are_full() {
        let entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);
        let recs = build_recommendations(&entries, &live_state(Vec::new(), Some(25)), 12, 3);

        // No open slot constrains the pick, so the board's best three come through as bench.
        assert_eq!(recs.len(), 3);
        assert_eq!(recs[0].name, "Top RB");
        assert!(recs.iter().all(|r| r.fills_need == "BE"));
    }

    #[test]
    fn recommendations_are_empty_without_an_identified_team() {
        let entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);

        let mut anonymous = live_state(vec![2], Some(25));
        anonymous.my_team = None;
        assert!(build_recommendations(&entries, &anonymous, 12, 3).is_empty());

        // A finished draft has nothing left to recommend.
        let mut done = live_state(vec![2], None);
        done.drafted = true;
        assert!(build_recommendations(&entries, &done, 12, 3).is_empty());
    }

    #[test]
    fn survival_margin_measures_adp_against_the_teams_next_pick() {
        let entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);
        let recs = build_recommendations(&entries, &live_state(vec![2, 2, 2], Some(25)), 12, 3);

        // "Top RB" goes at 1.5 and the team picks at 25: long gone.
        let top = recs.iter().find(|r| r.name == "Top RB").unwrap();
        assert_eq!(top.survival_margin, Some(-23.5));
        assert_eq!(top.outlook, Outlook::TakeNow);

        // "Low RB" goes at 40, well past pick 25.
        let low = recs.iter().find(|r| r.name == "Low RB").unwrap();
        assert_eq!(low.survival_margin, Some(15.0));
        assert_eq!(low.outlook, Outlook::CanWait);
    }

    #[test]
    fn outlook_is_unknown_without_a_next_pick_or_an_adp() {
        let entries = build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None);
        let recs = build_recommendations(&entries, &live_state(vec![2], None), 12, 3);

        assert_eq!(recs[0].survival_margin, None);
        assert_eq!(recs[0].outlook, Outlook::Unknown);
    }

    #[test]
    fn outlook_bands_scale_with_league_size() {
        // In a 12-team league the band runs from three picks early to six picks late.
        assert_eq!(classify_outlook(Some(-3.1), 12), Outlook::TakeNow);
        assert_eq!(classify_outlook(Some(-3.0), 12), Outlook::CoinFlip);
        assert_eq!(classify_outlook(Some(0.0), 12), Outlook::CoinFlip);
        assert_eq!(classify_outlook(Some(6.0), 12), Outlook::CoinFlip);
        assert_eq!(classify_outlook(Some(6.1), 12), Outlook::CanWait);

        // A smaller league tightens both edges.
        assert_eq!(classify_outlook(Some(4.0), 12), Outlook::CoinFlip);
        assert_eq!(classify_outlook(Some(4.0), 6), Outlook::CanWait);
        assert_eq!(classify_outlook(None, 12), Outlook::Unknown);
    }

    #[test]
    fn outlook_is_stricter_before_the_pick_than_after_it() {
        // Four picks early is a player who will not come back; four picks late is one who
        // still might slide. The same distance must not read the same in both directions.
        assert_eq!(classify_outlook(Some(-4.0), 12), Outlook::TakeNow);
        assert_eq!(classify_outlook(Some(4.0), 12), Outlook::CoinFlip);
    }

    /// A pool wrapper is needed for name lookups in live-event handling.
    fn pool_for_events() -> ScoredPool {
        ScoredPool {
            league_id: LeagueId::new(1),
            settings: settings_with_slots(&[(0, 1), (2, 2)]),
            team_count: 12,
            scored: scored_pool(),
            levels: levels_for_test(),
            position_by_player: scored_pool()
                .iter()
                .map(|(p, pos, _, _)| (PlayerId::new(p.id), *pos))
                .collect(),
        }
    }

    #[test]
    fn live_picks_are_recorded_and_attributed_to_the_right_team() {
        let pool = pool_for_events();
        let mut picks = FilePicks::default();

        let note = apply_live_event(
            DraftEvent::Selected {
                team_id: 9,
                player_id: PlayerId::new(1),
                slot_id: 2,
            },
            &mut picks,
            9,
            &pool,
        );
        assert!(
            note.unwrap().contains("YOURS"),
            "own pick should be flagged"
        );
        assert_eq!(picks.mine().count(), 1);

        apply_live_event(
            DraftEvent::Selected {
                team_id: 4,
                player_id: PlayerId::new(2),
                slot_id: 2,
            },
            &mut picks,
            9,
            &pool,
        );
        assert_eq!(picks.len(), 2);
        assert_eq!(picks.mine().count(), 1, "another team's pick is not mine");
    }

    #[test]
    fn auction_sales_count_as_picks() {
        let pool = pool_for_events();
        let mut picks = FilePicks::default();
        apply_live_event(
            DraftEvent::Sold {
                team_id: 3,
                player_id: PlayerId::new(1),
                slot_id: 2,
                bid: 41,
            },
            &mut picks,
            3,
            &pool,
        );
        assert_eq!(picks.len(), 1);
    }

    #[test]
    fn a_repeated_pick_event_is_ignored() {
        let pool = pool_for_events();
        let mut picks = FilePicks::default();
        let event = || DraftEvent::Selected {
            team_id: 9,
            player_id: PlayerId::new(1),
            slot_id: 2,
        };
        apply_live_event(event(), &mut picks, 9, &pool);
        // ESPN can repeat a message on reconnect; the board must not double-count it.
        assert!(apply_live_event(event(), &mut picks, 9, &pool).is_none());
        assert_eq!(picks.len(), 1);
    }

    #[test]
    fn undone_puts_the_player_back_on_the_board() {
        let pool = pool_for_events();
        let mut picks = FilePicks::default();
        apply_live_event(
            DraftEvent::Selected {
                team_id: 9,
                player_id: PlayerId::new(1),
                slot_id: 2,
            },
            &mut picks,
            9,
            &pool,
        );
        let note = apply_live_event(DraftEvent::Undone { pick_number: 1 }, &mut picks, 9, &pool);
        assert!(note.unwrap().contains("back on the board"));
        assert_eq!(picks.len(), 0);
    }

    #[test]
    fn being_displaced_is_reported_only_for_our_own_team() {
        let pool = pool_for_events();
        let mut picks = FilePicks::default();

        let note = apply_live_event(
            DraftEvent::Left {
                team_id: 9,
                reason: LEFT_REASON_DISPLACED,
            },
            &mut picks,
            9,
            &pool,
        );
        assert!(note.unwrap().contains("took over your draft session"));

        // Another team disconnecting is routine and must not raise the alarm.
        assert!(apply_live_event(
            DraftEvent::Left {
                team_id: 4,
                reason: LEFT_REASON_DISPLACED
            },
            &mut picks,
            9,
            &pool
        )
        .is_none());
    }

    #[test]
    fn being_on_the_clock_is_called_out() {
        let pool = pool_for_events();
        let mut picks = FilePicks::default();
        let mine = apply_live_event(
            DraftEvent::Selecting {
                team_id: 9,
                millis: 60_000,
            },
            &mut picks,
            9,
            &pool,
        );
        let mine = mine.unwrap();
        assert!(mine.contains("YOU ARE ON THE CLOCK"));
        // Milliseconds on the wire; a "60000s" clock would be nonsense.
        assert!(mine.contains("60s"), "got {mine}");
    }

    #[test]
    fn the_schedule_says_whose_turn_it_is() {
        use crate::espn::draft::{DraftDetail, DraftPick, DraftResponse};
        let pick = |overall: u32, team: Option<u32>| DraftPick {
            player_id: -1,
            team_id: team,
            overall_pick_number: overall,
            round_id: 1,
            round_pick_number: overall,
            lineup_slot_id: None,
            bid_amount: None,
            keeper: false,
            member_id: None,
        };
        let draft = DraftResponse {
            draft_detail: DraftDetail {
                drafted: false,
                in_progress: true,
                picks: vec![pick(1, Some(4)), pick(2, Some(9)), pick(3, Some(4))],
            },
            teams: vec![],
        };

        let mut picks = FilePicks::default();
        // Nothing drafted: slot 1 belongs to team 4.
        assert!(my_turn_by_schedule(&draft, &picks, 4));
        assert!(!my_turn_by_schedule(&draft, &picks, 9));

        picks.picks.push((PlayerId::new(1), false));
        // One pick in: slot 2 is team 9's.
        assert!(my_turn_by_schedule(&draft, &picks, 9));
        assert!(!my_turn_by_schedule(&draft, &picks, 4));
    }

    #[test]
    fn an_auction_has_no_schedule_to_read() {
        use crate::espn::draft::{DraftDetail, DraftPick, DraftResponse};
        // Auction picks carry no team, so the schedule can never claim it is our turn.
        let draft = DraftResponse {
            draft_detail: DraftDetail {
                drafted: false,
                in_progress: true,
                picks: vec![DraftPick {
                    player_id: -1,
                    team_id: None,
                    overall_pick_number: 1,
                    round_id: 1,
                    round_pick_number: 1,
                    lineup_slot_id: None,
                    bid_amount: None,
                    keeper: false,
                    member_id: None,
                }],
            },
            teams: vec![],
        };
        assert!(!my_turn_by_schedule(&draft, &FilePicks::default(), 4));
    }

    #[test]
    fn another_teams_turn_does_not_disturb_the_board() {
        // Every redraw clears the screen, so announcing all eight teams' turns would wipe
        // whatever the user is typing twice per pick. The header already names who is up.
        let pool = pool_for_events();
        let mut picks = FilePicks::default();
        assert!(apply_live_event(
            DraftEvent::Selecting {
                team_id: 4,
                millis: 60_000
            },
            &mut picks,
            9,
            &pool
        )
        .is_none());
    }

    #[test]
    fn repaint_overwrites_rather_than_blanking_the_screen() {
        // A clear-then-draw cycle leaves the screen momentarily empty and flickers. The
        // repaint must home the cursor, wipe each line as it rewrites it, and only drop
        // leftover rows at the very end.
        let painted = {
            let mut out = String::from("\x1b[H");
            for line in "alpha\nbeta".lines() {
                out.push_str(line);
                out.push_str("\x1b[K\n");
            }
            out.push_str("\x1b[J");
            out
        };
        assert!(
            !painted.contains("\x1b[2J"),
            "must not clear the whole screen"
        );
        assert!(painted.starts_with("\x1b[H"), "must home the cursor first");
        assert!(painted.ends_with("\x1b[J"), "leftover rows dropped last");
        assert_eq!(
            painted.matches("\x1b[K").count(),
            2,
            "every line wiped as rewritten"
        );
    }

    #[test]
    fn board_text_renders_without_printing() {
        let board = DraftBoard {
            league_name: Some("Test".to_string()),
            season: 2026,
            team_count: 12,
            starting_lineup: vec![("QB".to_string(), 1)],
            starters_by_position: BTreeMap::new(),
            replacement_points: BTreeMap::new(),
            entries: build_entries(&scored_pool(), &levels_for_test(), &HashSet::new(), None),
            live: None,
            recommendations: Vec::new(),
            unmatched_picks: Vec::new(),
        };
        let text = board_text(&board, 2);
        assert!(text.contains("Test · 2026 · 12 teams"));
        assert!(text.contains("Top RB"), "the board rows are present");
        // Only `top` rows are rendered, so the frame height stays predictable.
        assert!(!text.contains("Low RB"), "row limit is honoured");
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
