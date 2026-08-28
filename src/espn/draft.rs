//! Draft state from ESPN's `mDraftDetail` view.
//!
//! ESPN pre-allocates every pick slot before a draft starts and fills in `playerId` as picks
//! are made, so the same endpoint serves both "what is the pick order" before the draft and
//! "who is off the board" during it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{espn::types::Team, LeagueId, PlayerId, Result, Season};

/// Sentinel ESPN uses for a pick slot that has not been made yet.
const UNDRAFTED_PLAYER_ID: i64 = -1;

/// A single draft pick slot.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DraftPick {
    /// Player taken, or [`UNDRAFTED_PLAYER_ID`] if the slot is still open.
    #[serde(rename = "playerId")]
    pub player_id: i64,
    #[serde(rename = "teamId")]
    pub team_id: u32,
    #[serde(rename = "overallPickNumber")]
    pub overall_pick_number: u32,
    #[serde(rename = "roundId")]
    pub round_id: u32,
    #[serde(rename = "roundPickNumber")]
    pub round_pick_number: u32,
    #[serde(rename = "lineupSlotId", default)]
    pub lineup_slot_id: Option<u8>,
    #[serde(rename = "bidAmount", default)]
    pub bid_amount: Option<u32>,
    #[serde(default)]
    pub keeper: bool,
    /// Owner who made the pick; matches a team's `owners` entry.
    #[serde(rename = "memberId", default)]
    pub member_id: Option<String>,
}

impl DraftPick {
    /// Whether this slot has actually been used.
    pub fn is_made(&self) -> bool {
        self.player_id != UNDRAFTED_PLAYER_ID
    }

    /// The player taken, if the pick has been made.
    pub fn drafted_player(&self) -> Option<PlayerId> {
        self.is_made().then(|| PlayerId::new(self.player_id))
    }
}

/// The `draftDetail` object.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DraftDetail {
    #[serde(default)]
    pub drafted: bool,
    #[serde(rename = "inProgress", default)]
    pub in_progress: bool,
    #[serde(default)]
    pub picks: Vec<DraftPick>,
}

impl DraftDetail {
    /// Player ids already taken.
    pub fn taken_players(&self) -> std::collections::HashSet<PlayerId> {
        self.picks
            .iter()
            .filter_map(|p| p.drafted_player())
            .collect()
    }

    /// Picks made so far, in draft order.
    pub fn completed_picks(&self) -> Vec<&DraftPick> {
        let mut made: Vec<&DraftPick> = self.picks.iter().filter(|p| p.is_made()).collect();
        made.sort_by_key(|p| p.overall_pick_number);
        made
    }

    /// Players taken by one team, in the order they were drafted.
    pub fn picks_for_team(&self, team_id: u32) -> Vec<&DraftPick> {
        self.completed_picks()
            .into_iter()
            .filter(|p| p.team_id == team_id)
            .collect()
    }

    /// The next pick slot still waiting to be made.
    pub fn on_the_clock(&self) -> Option<&DraftPick> {
        self.picks
            .iter()
            .filter(|p| !p.is_made())
            .min_by_key(|p| p.overall_pick_number)
    }

    /// Total number of rounds the draft is configured for.
    pub fn total_rounds(&self) -> u32 {
        self.picks.iter().map(|p| p.round_id).max().unwrap_or(0)
    }
}

/// League response carrying both draft state and teams.
#[derive(Debug, Clone, Deserialize)]
pub struct DraftResponse {
    #[serde(rename = "draftDetail", default)]
    pub draft_detail: DraftDetail,
    #[serde(default)]
    pub teams: Vec<Team>,
}

impl DraftResponse {
    /// Find the team owned by the given SWID.
    ///
    /// ESPN wraps the SWID in braces and the environment variable is sometimes stored with a
    /// trailing separator, so both sides are normalised before comparing.
    pub fn team_for_owner(&self, swid: &str) -> Option<&Team> {
        let wanted = normalize_swid(swid);
        self.teams.iter().find(|team| {
            team.owners
                .iter()
                .flatten()
                .any(|owner| normalize_swid(owner) == wanted)
        })
    }

    /// Look up a team by id.
    pub fn team(&self, team_id: u32) -> Option<&Team> {
        self.teams.iter().find(|t| t.id == team_id)
    }

    /// Find a team whose name contains the given text, case-insensitively.
    pub fn team_by_name(&self, needle: &str) -> Option<&Team> {
        let needle = needle.to_lowercase();
        self.teams.iter().find(|t| {
            t.name
                .as_deref()
                .is_some_and(|n| n.to_lowercase().contains(&needle))
                || t.abbrev
                    .as_deref()
                    .is_some_and(|a| a.to_lowercase().contains(&needle))
        })
    }
}

/// Strip braces, case and stray separators from a SWID so two spellings compare equal.
fn normalize_swid(swid: &str) -> String {
    swid.trim()
        .trim_end_matches(':')
        .trim_matches(|c| c == '{' || c == '}')
        .to_ascii_lowercase()
}

/// Fetch current draft state and teams for a league.
///
/// Deliberately uncached: during a live draft the whole point is to see the latest picks.
pub async fn get_draft_detail(league_id: LeagueId, season: Season) -> Result<DraftResponse> {
    let raw: Value =
        super::http::get_league_view(league_id, season, &["mDraftDetail", "mTeam"]).await?;
    Ok(serde_json::from_value(raw)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(overall: u32, round: u32, team: u32, player: i64) -> DraftPick {
        DraftPick {
            player_id: player,
            team_id: team,
            overall_pick_number: overall,
            round_id: round,
            round_pick_number: overall,
            lineup_slot_id: None,
            bid_amount: None,
            keeper: false,
            member_id: None,
        }
    }

    fn detail() -> DraftDetail {
        DraftDetail {
            drafted: false,
            in_progress: true,
            picks: vec![
                pick(1, 1, 5, 3000),
                pick(2, 1, 6, 3001),
                pick(3, 1, 7, UNDRAFTED_PLAYER_ID),
                pick(4, 1, 8, UNDRAFTED_PLAYER_ID),
            ],
        }
    }

    #[test]
    fn taken_players_excludes_open_slots() {
        let taken = detail().taken_players();
        assert_eq!(taken.len(), 2);
        assert!(taken.contains(&PlayerId::new(3000)));
        assert!(!taken.contains(&PlayerId::new(-1)));
    }

    #[test]
    fn on_the_clock_is_the_lowest_unmade_pick() {
        let d = detail();
        assert_eq!(d.on_the_clock().unwrap().overall_pick_number, 3);
    }

    #[test]
    fn on_the_clock_is_none_when_draft_is_complete() {
        let mut d = detail();
        for p in &mut d.picks {
            p.player_id = 999;
        }
        assert!(d.on_the_clock().is_none());
    }

    #[test]
    fn picks_for_team_filters_and_orders() {
        let mut d = detail();
        d.picks.push(pick(5, 2, 5, 3005));
        let team_five = d.picks_for_team(5);
        assert_eq!(team_five.len(), 2);
        assert_eq!(team_five[0].overall_pick_number, 1);
        assert_eq!(team_five[1].overall_pick_number, 5);
    }

    #[test]
    fn normalize_swid_handles_braces_and_trailing_separator() {
        let canonical = normalize_swid("{00000000-1111-2222-3333-444444444444}");
        // The environment variable is sometimes stored with a trailing colon.
        assert_eq!(
            normalize_swid("{00000000-1111-2222-3333-444444444444}:"),
            canonical
        );
        // ESPN is inconsistent about braces and case between the cookie and the API payload.
        assert_eq!(
            normalize_swid("00000000-1111-2222-3333-444444444444"),
            canonical
        );
        assert_eq!(
            normalize_swid("{00000000-1111-2222-3333-AAAAAAAAAAAA}"),
            normalize_swid("00000000-1111-2222-3333-aaaaaaaaaaaa")
        );
    }
}
