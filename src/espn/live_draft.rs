//! Live draft feed.
//!
//! ESPN's REST API does not publish picks while a draft is running: `mDraftDetail` reports
//! every pick as `playerId: -1`, rosters stay empty, and player ownership reads
//! `FREEAGENT`, right up until the draft completes and the whole thing backfills at once.
//! The draft room itself is served by a separate host, `fantasydraft.espn.com`, speaking a
//! line-oriented text protocol. This module speaks it.
//!
//! Both directions are plain HTTP: inbound picks arrive on a Server-Sent Events stream, and
//! commands go out as ordinary GET requests. No WebSocket is involved, which is why this
//! needs no dependency beyond the `reqwest` the crate already uses.
//!
//! See `docs/espn-live-draft-protocol.md` for how the protocol was recovered and for the
//! parts deliberately left unimplemented.
//!
//! # One session per team
//!
//! The service allows a team exactly one connection. Joining **evicts** whatever client
//! held it, which in practice means the owner's browser draft room: the displaced client
//! receives `LEFT {teamId} {ownerId} 2` and its stream closes. Callers must therefore treat
//! connecting as taking over the draft, not observing it, and must never reconnect blindly
//! after being displaced — two clients doing that would evict each other forever.

use crate::{EspnError, LeagueId, PlayerId, Result, Season};

/// Host serving live draft rooms.
const DRAFT_HOST: &str = "https://fantasydraft.espn.com";

/// ESPN's numeric id for fantasy football on the draft service.
///
/// Not the `ffl` slug the REST API uses. `game-ffl` is rejected with a misleading
/// "LeagueId was either missing or invalid", which is why this constant is spelled out.
const FOOTBALL_GAME_ID: &str = "1";

/// Client identifier the web draft room sends.
const CLIENT_TAG: &str = "KONA";

/// The draft service rejects non-browser user agents with HTTP 403.
const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/128.0.0.0 Safari/537.36";

/// Reason code delivered in `LEFT` when another session takes over the team.
pub const LEFT_REASON_DISPLACED: i64 = 2;

/// Something that happened in the draft room.
#[derive(Debug, Clone, PartialEq)]
pub enum DraftEvent {
    /// A pick was made. The event that matters in a snake draft.
    Selected {
        team_id: u32,
        player_id: PlayerId,
        slot_id: i64,
    },
    /// A team is on the clock.
    Selecting {
        team_id: u32,
        seconds: i64,
    },
    /// A player was won at auction.
    Sold {
        team_id: u32,
        player_id: PlayerId,
        slot_id: i64,
        bid: i64,
    },
    /// A pick was rolled back.
    Undone {
        pick_number: u32,
    },
    /// Clock tick: phase, remaining time, and who/what it refers to.
    Clock {
        phase: i64,
        time: i64,
        team_id: i64,
        player_id: i64,
        amount: i64,
    },
    Joined {
        team_id: u32,
    },
    /// A client disconnected. `reason == LEFT_REASON_DISPLACED` for our own team means we
    /// were kicked by another session.
    Left {
        team_id: u32,
        reason: i64,
    },
    /// The server rejected something. Arrives in-band on an HTTP 200 stream.
    Error {
        message: String,
    },
    /// Anything not modelled: chat, bids, nominations, state, pings, init blobs.
    Other {
        verb: String,
        raw: String,
    },
}

/// Parse one protocol message: a verb followed by space-delimited positional fields.
pub fn parse_event(message: &str) -> Option<DraftEvent> {
    let mut fields = message.split_whitespace();
    let verb = fields.next()?;
    let rest: Vec<&str> = fields.collect();

    let num = |i: usize| -> Option<i64> { rest.get(i)?.parse().ok() };
    let team = |i: usize| -> Option<u32> { rest.get(i)?.parse().ok() };

    let event = match verb {
        "SELECTED" => DraftEvent::Selected {
            team_id: team(0)?,
            player_id: PlayerId::new(num(1)?),
            slot_id: num(2).unwrap_or(-1),
        },
        "SELECTING" => DraftEvent::Selecting {
            team_id: team(0)?,
            seconds: num(1).unwrap_or(-1),
        },
        "SOLD" => DraftEvent::Sold {
            team_id: team(0)?,
            player_id: PlayerId::new(num(1)?),
            slot_id: num(2).unwrap_or(-1),
            bid: num(3).unwrap_or(0),
        },
        "UNDONE" => DraftEvent::Undone {
            pick_number: u32::try_from(num(0)?).ok()?,
        },
        "CLOCK" => DraftEvent::Clock {
            phase: num(0).unwrap_or(-1),
            time: num(1).unwrap_or(-1),
            team_id: num(2).unwrap_or(-1),
            player_id: num(3).unwrap_or(-1),
            amount: num(4).unwrap_or(-1),
        },
        "JOINED" => DraftEvent::Joined { team_id: team(0)? },
        "LEFT" => DraftEvent::Left {
            team_id: team(0)?,
            reason: num(2).unwrap_or(-1),
        },
        // ESPN URL-encodes free text and uses `+` for spaces.
        "ERROR" => DraftEvent::Error {
            message: rest.join(" ").replace('+', " "),
        },
        other => DraftEvent::Other {
            verb: other.to_string(),
            raw: message.to_string(),
        },
    };
    Some(event)
}

/// The SWID in the form the draft service expects: braces kept, trailing separator removed.
///
/// Deliberately different from [`crate::espn::draft`]'s normalisation, which strips braces
/// for comparison against API payloads. Here the braces are part of the value, and the
/// security token is colon-delimited — an `ESPN_SWID` stored as `{GUID}:` would inject an
/// empty field and shift every position in the token.
fn swid_for_draft(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches(':').trim();
    let bare = trimmed.trim_matches(|c| c == '{' || c == '}');
    format!("{{{}}}", bare)
}

/// A connection to one team's draft room.
pub struct LiveDraftSession {
    league_id: LeagueId,
    team_id: u32,
    swid: String,
    /// `{gameId}:{leagueId}:{teamId}:{swid}:{draftSecurity}`.
    security_token: String,
    client: reqwest::Client,
}

impl LiveDraftSession {
    /// Authenticate against the draft service for one team.
    pub async fn open(league_id: LeagueId, season: Season, team_id: u32) -> Result<Self> {
        let swid = swid_for_draft(&std::env::var("ESPN_SWID").map_err(|_| EspnError::Cache {
            message: "ESPN_SWID must be set to join a live draft".to_string(),
        })?);
        let s2 = std::env::var("ESPN_S2").map_err(|_| EspnError::Cache {
            message: "ESPN_S2 must be set to join a live draft".to_string(),
        })?;
        let cookie = format!("SWID={}; espn_s2={}", swid, s2.trim().trim_end_matches(':'));

        let client = reqwest::Client::builder()
            .build()
            .map_err(EspnError::from)?;

        // The draft service will not issue its own token; this comes from the REST API.
        let url = format!(
            "{}/seasons/{}/segments/0/leagues/{}/teams/{}/draftSecurity",
            crate::espn::http::FFL_BASE_URL,
            season.as_u16(),
            league_id.as_u32(),
            team_id
        );
        let draft_security = client
            .get(&url)
            .header("Cookie", &cookie)
            .header("Accept", "application/json")
            .header("X-Fantasy-Source", "kona")
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?
            .trim()
            .to_string();

        let security_token = format!(
            "{}:{}:{}:{}:{}",
            FOOTBALL_GAME_ID,
            league_id.as_u32(),
            team_id,
            swid,
            draft_security
        );

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&cookie)?,
        );
        // The draft service answers 403 to a non-browser agent, so it has to look like the
        // draft room it is replacing.
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(BROWSER_USER_AGENT),
        );
        headers.insert(
            reqwest::header::REFERER,
            reqwest::header::HeaderValue::from_static("https://fantasy.espn.com/"),
        );
        headers.insert(
            reqwest::header::ORIGIN,
            reqwest::header::HeaderValue::from_static("https://fantasy.espn.com"),
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(EspnError::from)?;

        Ok(Self {
            league_id,
            team_id,
            swid,
            security_token,
            client,
        })
    }

    /// Base URL for this league's draft room.
    fn room_url(&self) -> String {
        format!(
            "{}/game-{}/league-{}",
            DRAFT_HOST,
            FOOTBALL_GAME_ID,
            self.league_id.as_u32()
        )
    }

    /// Join the room and start receiving events.
    ///
    /// This takes over the team's session; see the module docs.
    pub async fn subscribe(&self) -> Result<DraftStream> {
        let url = format!("{}/sse/JOIN", self.room_url());
        let team = self.team_id.to_string();
        let nocache = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
            .to_string();

        // The query is assembled by hand rather than with `query()`: the service rejects a
        // percent-encoded SWID or security token, returning an empty 200 stream that looks
        // like a successful connection but never delivers a frame. Braces and colons must
        // reach it literally, which `Url::parse` preserves and form-encoding does not.
        let query = format!(
            "1={}&2={}&3={}&4={}&5={}&6=false&7=false&8={}&nocache={}",
            FOOTBALL_GAME_ID,
            self.league_id.as_u32(),
            team,
            self.swid,
            self.security_token,
            CLIENT_TAG,
            nocache
        );

        let response = self
            .client
            .get(format!("{}?{}", url, query))
            .header("Accept", "text/event-stream")
            .send()
            .await?
            .error_for_status()?;

        Ok(DraftStream {
            response,
            buffer: Vec::new(),
        })
    }

    /// Send one command. Arguments are positional, numbered from 1.
    async fn command(&self, verb: &str, args: &[&str]) -> Result<()> {
        // Hand-built for the same reason as `subscribe`: the token must not be encoded.
        let mut query: String = args
            .iter()
            .enumerate()
            .map(|(i, a)| format!("{}={}&", i + 1, a))
            .collect();
        query.push_str(&format!("token={}", self.security_token));

        self.client
            .get(format!("{}/{}?{}", self.room_url(), verb, query))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Draft a player.
    pub async fn select(&self, player_id: PlayerId) -> Result<()> {
        self.command("SELECT", &[&player_id.to_string()]).await
    }

    /// Keep the session alive. The web client pings every 15 seconds.
    pub async fn ping(&self) -> Result<()> {
        self.command("PING", &["keepalive"]).await
    }

    /// Leave cleanly, releasing the team's session.
    pub async fn leave(&self) -> Result<()> {
        self.command("LEAVE", &[]).await
    }

    pub fn team_id(&self) -> u32 {
        self.team_id
    }
}

/// An open event stream.
pub struct DraftStream {
    response: reqwest::Response,
    buffer: Vec<u8>,
}

impl DraftStream {
    /// Next event, or `None` once the server closes the stream.
    ///
    /// Frames arrive as SSE `data:` lines; anything else in the stream is skipped.
    pub async fn next_event(&mut self) -> Result<Option<DraftEvent>> {
        loop {
            if let Some(newline) = self.buffer.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=newline).collect();
                let text = String::from_utf8_lossy(&line);
                if let Some(payload) = text.trim().strip_prefix("data:") {
                    if let Some(event) = parse_event(payload.trim()) {
                        return Ok(Some(event));
                    }
                }
                continue;
            }

            match self.response.chunk().await? {
                Some(bytes) => self.buffer.extend_from_slice(&bytes),
                None => return Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swid_keeps_braces_and_drops_the_trailing_separator() {
        let want = "{ABC-123}";
        assert_eq!(swid_for_draft("{ABC-123}"), want);
        // ESPN_SWID is commonly stored with a trailing colon, which would otherwise inject
        // an empty field into the colon-delimited security token.
        assert_eq!(swid_for_draft("{ABC-123}:"), want);
        assert_eq!(swid_for_draft("  {ABC-123}: "), want);
        // Braces are added when missing.
        assert_eq!(swid_for_draft("ABC-123"), want);
    }

    #[test]
    fn parses_a_pick() {
        assert_eq!(
            parse_event("SELECTED 8 4429795 2"),
            Some(DraftEvent::Selected {
                team_id: 8,
                player_id: PlayerId::new(4429795),
                slot_id: 2,
            })
        );
    }

    #[test]
    fn parses_the_clock_and_who_is_up() {
        assert_eq!(
            parse_event("SELECTING 3 90"),
            Some(DraftEvent::Selecting {
                team_id: 3,
                seconds: 90
            })
        );
        assert_eq!(
            parse_event("CLOCK 2 6549 8 4429160 47"),
            Some(DraftEvent::Clock {
                phase: 2,
                time: 6549,
                team_id: 8,
                player_id: 4429160,
                amount: 47
            })
        );
    }

    #[test]
    fn parses_eviction() {
        // The message that arrives when another client takes over this team.
        assert_eq!(
            parse_event("LEFT 1 {00000000-1111-2222-3333-444444444444} 2"),
            Some(DraftEvent::Left {
                team_id: 1,
                reason: LEFT_REASON_DISPLACED
            })
        );
    }

    #[test]
    fn parses_auction_messages() {
        assert_eq!(
            parse_event("SOLD 4 3139477 6 42"),
            Some(DraftEvent::Sold {
                team_id: 4,
                player_id: PlayerId::new(3139477),
                slot_id: 6,
                bid: 42
            })
        );
    }

    #[test]
    fn decodes_in_band_errors() {
        // A rejected command comes back on an HTTP 200 stream, so a 200 is not success.
        assert_eq!(
            parse_event("ERROR 1 Invalid+arguments+for+command."),
            Some(DraftEvent::Error {
                message: "1 Invalid arguments for command.".to_string()
            })
        );
    }

    #[test]
    fn unmodelled_verbs_are_preserved_rather_than_dropped() {
        match parse_event("NOMINATION 7 30") {
            Some(DraftEvent::Other { verb, raw }) => {
                assert_eq!(verb, "NOMINATION");
                assert_eq!(raw, "NOMINATION 7 30");
            }
            other => panic!("expected Other, got {:?}", other),
        }
        assert_eq!(parse_event(""), None);
    }
}
