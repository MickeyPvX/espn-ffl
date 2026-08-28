# ESPN Fantasy Football CLI

[![CI](https://github.com/MickeyPvX/espn-ffl/workflows/CI/badge.svg)](https://github.com/MickeyPvX/espn-ffl/actions/workflows/ci.yml)

A fast, reliable command-line tool for querying ESPN Fantasy Football player statistics and advanced projection analysis. Built in Rust for performance and type safety.

## What it does

- **Draft board** - Value over replacement computed from *your* league's scoring and starting lineup, cross-referenced against ESPN's ADP
- **Live draft mode** - Track picks as they happen, hide drafted players, see what your roster still needs
- **Query player stats** by name, position, team, injury/roster status
- **Get actual or projected points** for any week and season
- **Projection analysis** - ESPN projection accuracy with bias correction
- **Smart filtering** - Server-side filtering for performance
- **Export data** as JSON for analysis or integration
- **Database caching** - Local storage for faster queries

Perfect for drafting, fantasy football analysis, lineup optimization, and projection research.

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable version)

### Build from source

```bash
git clone https://github.com/MickeyPvX/espn-ffl.git
cd espn-ffl
cargo build --release
```

The executable will be at `target/release/espn-ffl`. Add it to your PATH or run it directly.

## Setup

### 1. Get your ESPN cookies

ESPN's fantasy API requires authentication for private leagues. You'll need two cookies: `SWID` and `espn_s2`.

1. **Log into ESPN** - Go to your fantasy football league page
2. **Open Developer Tools** - Press F12 or right-click → Inspect
3. **Find the cookies**:
   - Go to the **Network** tab
   - Refresh the page
   - Click any request to `fantasy.espn.com/apis/v3/games/ffl/...`
   - In the **Headers** section, find the `cookie` field
   - Copy the values for `SWID={...}` and `espn_s2={...}`

### 2. Set environment variables

```bash
export ESPN_SWID="{your-swid-value}"
export ESPN_S2="{your-espn_s2-value}"
```

Add these to your `.bashrc`, `.zshrc`, or equivalent to make them permanent.

### 3. Find your league ID

Your league ID is in the URL when viewing your league:
`https://fantasy.espn.com/football/league?leagueId=123456` → League ID is `123456`

The season defaults to whichever NFL season is currently in progress, so `--season` is only needed to look at past years.

## Usage

### Quick Start

```bash
# Build a draft board for your league
espn-ffl draft-board

# Follow your draft live, refreshing every 30 seconds
espn-ffl draft-board --watch 30

# Get all players for week 3
espn-ffl player-data --week 3

# Find specific players
espn-ffl player-data -n "Josh Allen" -n "Travis Kelce" --week 1

# Get quarterbacks and wide receivers
espn-ffl player-data -p QB -p WR --week 2

# Get projection analysis with bias correction
espn-ffl projection-analysis --week 5
```

### Draft board

ESPN's draft room ranks players with generic, league-agnostic rankings. This command instead
recomputes every player's season projection using your league's own scoring settings, then
measures each one against the *replacement level* implied by your actual starting lineup —
the points you could get for free at that position after every team has filled its starters.

```text
My League · 2026 · 12 teams
Starting lineup: QB RB2 WR2 TE D/ST K FLEX
Starters drafted leaguewide: D/ST 12 · K 12 · QB 12 · RB 28 · TE 12 · WR 32

   #  Name                     Pos       Proj      VOR     ADP       Δ  Bye
----------------------------------------------------------------------------
   1  Jahmyr Gibbs             RB       364.9    178.8     1.4      +0    6
   2  Puka Nacua               WR       356.3    169.0     5.2      +3   11
  17  Breece Hall              RB       274.0     87.9    33.7     +17   13
  20  Josh Allen               QB       369.7     82.8    22.0      +2    7

Replacement level: D/ST 93 · K 143 · QB 287 · RB 186 · TE 167 · WR 187
```

Josh Allen has the highest raw projection on the board (369.7) but ranks 20th, because the
12th-best QB still scores 287 — so drafting him buys only 83 points over what you could
have had for nothing. Breece Hall's `+17` means he typically goes 17 picks later than this
board values him.

- **Proj** — season projection under your league's scoring
- **VOR** — points above the replacement-level player at that position
- **ADP** — ESPN's average draft position
- **Δ** — ADP minus value rank; positive means the player usually falls past his value
- **Bye** — inferred from the gap in ESPN's weekly projections

`--position` narrows which rows print but never changes the arithmetic: replacement levels
and value ranks are always computed across the whole pool so the numbers stay comparable.

### Live draft

With `--live` the board reads your draft as it happens: drafted players drop off, and it
identifies your team (via the `ESPN_SWID` cookie, or `--team` / `--team-id`) to show what
you still need.

```text
Round 3 · pick 28 of 192 · ON THE CLOCK: Team Alpha
You: Team Alpha
Your roster: RB WR
Still need: QB RB WR TE D/ST K FLEX
```

`--watch <SECONDS>` re-reads the draft on an interval until it completes. The player pool is
fetched once up front, so each refresh is a single small request.

### Being a good API citizen

ESPN publishes no documented rate limit, so the tool stays deliberately conservative:

- Requests are spaced at least 250 ms apart, so bulk operations never burst.
- A `429` or `5xx` is retried with exponential backoff, honouring `Retry-After` when sent.
- Responses are cached on disk, with a lifetime matched to how fast the data moves. A
  finished season is final, so its cached responses never expire. The season in progress
  ages out after 30 minutes, since projections, rosters and live scores all shift. The
  draft pool (~10 MB) is reused for 6 hours. `--refresh` overrides any of it.
- Settled weeks land in the local database, which is consulted before the network, so the
  cache lifetime above mostly governs weeks still in motion.
- Player requests are narrowed server-side to the lineup slots your league can actually
  roster, which roughly halves every response.
- `update-all-data` fetches each week once. Actual and projected points come from the same
  ESPN response, so the projected pass reuses it rather than downloading it again.

### Team Filtering

```bash
# Filter by team name (partial matching)
espn-ffl player-data --team alpha --week 1

# Filter by exact team ID
espn-ffl player-data --team-id 123 --week 1

# Works with projection analysis too
espn-ffl projection-analysis --team alpha --week 5
```

### Output Formats

**Default text output:**
```text
3918298 Josh Allen (QB) [week 1] 38.76 [Active] (Team Alpha)
4426515 Puka Nacua (WR) [week 1] 15.90 [Active] (FA)
```

**Projection analysis:**
```text
Name                 Pos      ESPN     Adj      Final    Conf%    Reasoning
----                 ---      ----     ---      -----    ----     ---------
Josh Allen           QB       22.3     +4.5     26.7     32      % Avg bias: ESPN underestimates by 5.1 pts (3 games, 2.1 std) - adjusted up 4.5 pts (32% confidence)
```

**JSON export:**
```bash
espn-ffl player-data --week 1 --json > week1_stats.json
espn-ffl projection-analysis --week 2 --json > projections.json
```

For complete command reference, see [CLI_REFERENCE.md](CLI_REFERENCE.md).

## Troubleshooting

- **"Missing league ID" error**: Set `ESPN_FFL_LEAGUE_ID` environment variable or use `--league-id`
- **Empty draft board**: ESPN only publishes preseason projections and ADP for the upcoming season; check `--season` matches a season ESPN has populated
- **Authentication errors**: Double-check your `ESPN_SWID` and `ESPN_S2` cookies are current
- **No results**: Verify the week has games scheduled and use `--debug` to see the API request
- **Build errors**: Ensure you have the latest stable Rust: `rustup update`

## Development

```bash
# Run tests
cargo test

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# Format code
cargo fmt

# Lint code
cargo clippy
```

## License

MIT License - see LICENSE file for details.
