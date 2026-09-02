# CLI Reference

Complete command reference for ESPN Fantasy Football CLI.

## Commands

### `espn-ffl draft-board`

Rank the draft pool by value over replacement in your league's scoring.

Recomputes each player's season projection with your league's scoring settings, measures it
against the replacement level implied by your starting lineup, and compares that ranking to
ESPN's average draft position.

**Core Options:**
- `-l, --league-id <ID>` - League ID (or set `ESPN_FFL_LEAGUE_ID` env var)
- `-s, --season <YEAR>` - Season year (defaults to the NFL season in progress)
- `-p, --position <POS>` - Show only these positions (repeatable). Narrows the printed rows
  only; replacement levels and value ranks are always computed across the whole pool
- `--top <N>` - Number of players to display (default: 40)
- `--pool-size <N>` - How many players to pull from ESPN before ranking (default: 700)
- `--rank-type <TYPE>` - ESPN ranking used to select the pool: `PPR`, `STANDARD`, `SUPERFLEX` (default: PPR)

**Data Options:**
- `--refresh` - Refetch projections and ADP instead of using the cached pool (cached for 6 hours)

**Live Draft Options:**
- `--live-draft` - Follow ESPN's live draft feed and draft from this tool (see below)
- `-i, --interactive` - Type each pick at a prompt; the board redraws immediately
- `--taken-file <PATH>` - Track picks from a file; re-read on every refresh
- `--live` - Read live draft state: hide drafted players, show your remaining needs
- `--watch <SECONDS>` - Re-read the draft on an interval until it completes (implies `--live`)
- `--team <NAME>` - Your fantasy team (partial match). Defaults to the team owned by `ESPN_SWID`
- `--team-id <ID>` - Your fantasy team ID

**Output Options:**
- `--json` - Output as JSON
- `--debug` - Print the request URL and fantasy filter

**Columns:**
- `Proj` - season projection under your league's scoring
- `VOR` - points above the replacement-level player at that position
- `ADP` - ESPN's average draft position
- `Δ` - ADP minus value rank; positive means the player usually falls past his value
- `Bye` - inferred from the gap in ESPN's weekly projections

**Best available (live drafts):**

With `--live` or `--watch` and an identified team, the board leads with the three most
valuable players who fill a starting slot you have not yet filled. Your roster and remaining
slots are recomputed from the positions you have drafted, not from ESPN's `lineupSlotId`,
and dedicated slots are filled before flex ones so a flex-eligible pick does not mask a real
need. Once every starter is accounted for, the suggestions fall back to best available
overall and are marked `[BE]`.

Each suggestion is annotated with whether it will survive until your next pick, based on ADP
against your next overall pick number:
- `take now` - usually drafted more than a quarter round before you pick again
- `coin flip` - from a quarter round before your next pick to half a round after it
- `can wait` - usually still on the board more than half a round past your next pick
- `unknown` - no published ADP, or no remaining pick to measure against

The band is asymmetric on purpose. A player whose ADP lands after your next pick may still
slide far enough to reach you, but one whose ADP lands before it rarely comes back, since
every team in between has to pass on him. Being wrong early costs more than being wrong late.

Value and urgency stay separate columns rather than one blended score, so it is always clear
which one is driving a suggestion.

**Output Format:**
```text
My League · 2026 · 12 teams
Starting lineup: QB RB2 WR2 TE D/ST K FLEX
Starters drafted leaguewide: D/ST 12 · K 12 · QB 12 · RB 28 · TE 12 · WR 32

Round 3 · pick 29 of 192 · ON THE CLOCK: Team Alpha
You: Team Alpha
Your roster: RB WR
Still need: QB RB WR FLEX K

Best available for your needs (your next pick: 41):
  1  Bucky Irving             RB   [RB]    VOR  121.4   ADP  33.2   bye 11   take now
  2  DK Metcalf               WR   [WR]    VOR  118.0   ADP  38.9   bye 5    coin flip
  3  Jaxon Smith-Njigba       WR   [FLEX]  VOR  112.7   ADP  52.1   bye --   can wait

   #  Name                     Pos       Proj      VOR     ADP       Δ  Bye
----------------------------------------------------------------------------
   1  Jahmyr Gibbs             RB       364.9    178.8     1.4      +0    6
  20  Josh Allen               QB       369.7     82.8    22.0      +2    7

Replacement level: D/ST 93 · K 143 · QB 287 · RB 186 · TE 167 · WR 187
```

### Live drafts

`--live` reads ESPN's REST API, which **does not publish picks while a draft is running**:
every pick reports as undrafted until the draft completes, then the whole thing backfills at
once. During the draft itself, `--live` shows an untouched board.

`--live-draft` connects to ESPN's actual draft room feed instead, so picks appear as they
happen and `draft <name>` sends your pick.

```bash
espn-ffl draft-board --live-draft
```

```text
LIVE · connected as Your Team · picks arrive automatically
`draft <name>` to pick · `list` · `quit`
draft> draft gibbs
Sent pick: Jahmyr Gibbs
```

**This takes over your draft session.** ESPN allows one draft connection per team, so
starting this evicts your browser draft room. Nothing but autodraft will pick for you, so
once it is running you must pick from here. If something else takes the session back, the
board says so and stops rather than fighting for it.

**Start it before the draft begins.** Only picks made after you connect are seen — ESPN
sends prior state in a binary blob this tool does not decode, and the REST API cannot fill
the gap because it publishes nothing mid-draft.

Auction drafts are supported for pick tracking (`SOLD` counts as a pick), but there is no
pick order, so on-the-clock and next-pick outlooks read as unknown.

If you would rather keep the browser draft room, use `--interactive` and type picks
yourself; it never connects to the draft service.

### `espn-ffl player-data`

Get player statistics and fantasy points for a specific week.

**Core Options:**
- `-l, --league-id <ID>` - League ID (or set `ESPN_FFL_LEAGUE_ID` env var)
- `-s, --season <YEAR>` - Season year (defaults to the NFL season in progress)
- `-w, --week <WEEK>` - Week number (default: 1)

**Filtering Options:**
- `-n, --player-name <NAME>` - Filter by player name (repeatable)
- `-p, --position <POS>` - Filter by position: QB, RB, WR, TE, K, DEF, FLEX (repeatable)
- `--team <NAME>` - Filter by team name (e.g., "alpha" for partial match)
- `--team-id <ID>` - Filter by exact team ID number
- `--injury-status <STATUS>` - Filter by injury status:
  - `active` - Healthy players (server-side filtered)
  - `injured` - Any injured players (server-side filtered)
  - `out` - Players ruled out (client-side filtered)
  - `doubtful` - Doubtful status (client-side filtered)
  - `questionable` - Questionable status (client-side filtered)
  - `probable` - Probable status (client-side filtered)
  - `day-to-day` - Day-to-day status (client-side filtered)
  - `ir` - Injury Reserve (client-side filtered)
- `--roster-status <STATUS>` - Filter by roster status (client-side filtered):
  - `rostered` - Players on fantasy teams
  - `fa` - Free agents

**Output Options:**
- `--json` - Output as JSON instead of text
- `--debug` - Show API request details
- `--proj` - Use projected points instead of actual

**Data Management:**
- `--refresh` - Force fresh data from ESPN API
- `--clear-db` - Clear local database before fetching

### `espn-ffl projection-analysis`

Analyze ESPN projection accuracy with advanced bias correction algorithms.

**Core Options:**
- `-l, --league-id <ID>` - League ID (or set `ESPN_FFL_LEAGUE_ID` env var)
- `-s, --season <YEAR>` - Season year (defaults to the NFL season in progress)
- `-w, --week <WEEK>` - Week number (default: 1)

**Filtering Options:**
- `-n, --player-name <NAME>` - Filter by player name (repeatable)
- `-p, --position <POS>` - Filter by position (repeatable)
- `--team <NAME>` - Filter by team name
- `--team-id <ID>` - Filter by exact team ID
- `--injury-status <STATUS>` - Filter by injury status (same options as player-data)
- `--roster-status <STATUS>` - Filter by roster status (same options as player-data)

**Analysis Options:**
- `--bias-strength <FLOAT>` - Bias correction strength (0.0-2.0+, default: 1.0)
- `--json` - Output as JSON
- `--refresh` - Force fresh data from ESPN API

**Output Format:**
```text
Name                 Pos      ESPN     Adj      Final    Conf%    Reasoning
----                 ---      ----     ---      -----    ----     ---------
Puka Nacua           WR       21.2     +5.3     26.5     49      % Avg bias: ESPN underestimates by 7.9 pts (4 games, 4.6 std) - adjusted up 5.3 pts (49% confidence)
```

The projection analysis uses a sophisticated algorithm that:
- Calculates player-specific bias patterns from historical data
- Excludes BYE weeks (0-point projections) from analysis
- Bases confidence on pattern consistency (lower std dev = higher confidence)
- Makes aggressive but statistically sound adjustments (2-5+ point corrections)

### `espn-ffl league-data`

Cache league settings for faster subsequent queries.

- `-l, --league-id <ID>` - League ID
- `-s, --season <YEAR>` - Season year
- `--refresh` - Force refresh settings
- `--verbose` - Show detailed output

## Examples

### Draft

```bash
# Full board for your league
espn-ffl draft-board

# Just the tight ends, ranked by value against the rest of the pool
espn-ffl draft-board -p TE --top 15

# Follow the draft live, refreshing every 30 seconds
espn-ffl draft-board --watch 30

# Board for a specific team's perspective
espn-ffl draft-board --live --team "alpha"

# Export the board for a spreadsheet
espn-ffl draft-board --top 300 --json > draft_board.json
```

### Basic Usage

```bash
# Get all players for week 3
espn-ffl player-data --week 3

# Find specific players
espn-ffl player-data -n "Josh Allen" -n "Travis Kelce" --week 1

# Get quarterbacks and wide receivers
espn-ffl player-data -p QB -p WR --week 2

# Get FLEX-eligible players (RB/WR/TE)
espn-ffl player-data -p FLEX --week 1

# Get projected points instead of actual
espn-ffl player-data --week 1 --proj
```

### Advanced Filtering

```bash
# Team filtering
espn-ffl player-data --team alpha --week 1                    # Players on "alpha" team
espn-ffl player-data --team-id 123 --week 1                   # Players on team ID 123

# Combined filtering
espn-ffl player-data -p RB --injury-status active --roster-status rostered --week 1

# Free agent analysis
espn-ffl player-data -p WR --roster-status fa --injury-status active --week 1
```

### Projection Analysis

```bash
# Analyze ESPN's projection accuracy
espn-ffl projection-analysis --week 5

# Filter by position
espn-ffl projection-analysis -p QB --week 5

# Team-specific analysis
espn-ffl projection-analysis --team alpha --week 5

# Custom bias strength
espn-ffl projection-analysis --week 2 --bias-strength 1.5

# Export as JSON for analysis
espn-ffl projection-analysis --week 2 --json > projections.json
```

### Export and Analysis

```bash
# Export all week data
espn-ffl player-data --week 1 --json > week1_stats.json

# Export only your roster
espn-ffl player-data --roster-status rostered --week 1 --json > my_roster.json

# Export team-specific projections
espn-ffl projection-analysis --team alpha --week 2 --json > team_projections.json
```

## Environment Variables

- `ESPN_SWID` - ESPN SWID cookie (required)
- `ESPN_S2` - ESPN S2 cookie (required)
- `ESPN_FFL_LEAGUE_ID` - Default league ID (optional, can use --league-id instead)

## Output Formats

### Player Data (Text)
```text
3918298 Josh Allen (QB) [week 1] 38.76 [Active] (Team Alpha)
4426515 Puka Nacua (WR) [week 1] 15.90 [Active] (FA)
```

### Player Data (JSON)
```json
[
  {
    "id": 3918298,
    "name": "Josh Allen",
    "position": "QB",
    "week": 1,
    "projected": false,
    "points": 38.76,
    "active": true,
    "injured": false,
    "injury_status": "Active",
    "is_rostered": true,
    "team_id": 1,
    "team_name": "Team Alpha"
  }
]
```

### Projection Analysis (JSON)
```json
[
  {
    "player_id": 4426515,
    "name": "Puka Nacua",
    "position": "WR",
    "team": null,
    "espn_projection": 21.2,
    "bias_adjustment": 5.3,
    "estimated_points": 26.5,
    "confidence": 0.49,
    "reasoning": "Avg bias: ESPN underestimates by 7.9 pts (4 games, 4.6 std) - adjusted up 5.3 pts (49% confidence)"
  }
]
```