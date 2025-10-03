# ESPN Fantasy Football CLI

[![CI](https://github.com/MickeyPvX/espn-ffl/workflows/CI/badge.svg)](https://github.com/MickeyPvX/espn-ffl/actions/workflows/ci.yml)

A fast, reliable command-line tool for querying ESPN Fantasy Football player statistics and advanced projection analysis. Built in Rust for performance and type safety.

## What it does

- **Query player stats** by name, position, or both (supports multiple filters)
- **Get actual or projected points** for any week and season
- **Smart filtering** - Server-side filtering by injury status and roster position for efficiency
- **Injury status filtering** - Find active, injured, questionable, out, or IR players
- **Roster status filtering** - Filter by rostered players vs free agents
- **Team filtering** - Filter by specific fantasy teams using flexible team name or ID matching
- **Projection analysis** - Compare ESPN projections vs. actual performance with bias correction
- **FLEX position support** - Filter by FLEX to get RB/WR/TE players
- **Export data** as JSON for analysis or integration
- **Database caching** - Local storage for faster subsequent queries

Perfect for fantasy football analysis, automation, lineup optimization, or projection accuracy research.

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

## Usage

### Basic examples

```bash
# Get all players for week 3 of 2025 season
espn-ffl get player-data --week 3 --season 2025

# Find specific players (repeatable)
espn-ffl get player-data -n "Josh Allen" -n "Travis Kelce" --week 1

# Get all quarterbacks and wide receivers
espn-ffl get player-data -p QB -p WR --week 2

# Get FLEX-eligible players (RB/WR/TE)
espn-ffl get player-data -p FLEX --week 1

# Get projected points instead of actual
espn-ffl get player-data --week 1 --proj
```

### Advanced filtering

```bash
# Get only active (healthy) players
espn-ffl get player-data -p QB --injury-status active --week 1

# Find injured running backs
espn-ffl get player-data -p RB --injury-status injured --week 1

# Get players who are questionable for this week
espn-ffl get player-data --injury-status questionable --week 1

# Find free agent wide receivers
espn-ffl get player-data -p WR --roster-status fa --week 1

# Get rostered quarterbacks only
espn-ffl get player-data -p QB --roster-status rostered --week 1

# Combine filters: active rostered RBs
espn-ffl get player-data -p RB --injury-status active --roster-status rostered --week 1

# Filter by team name (partial match, case-insensitive)
espn-ffl get player-data --team kenny --week 1  # Matches "Kenny Rogers' Toasters"

# Filter by team ID
espn-ffl get player-data --team-id 123 --week 1

# Combine team filter with other filters
espn-ffl get player-data --team mike --injury-status active --week 1
```

### Specify a league

```bash
# Use specific league ID
espn-ffl get player-data --league-id 123456 --week 1

# Or set as environment variable
export ESPN_FFL_LEAGUE_ID=123456
espn-ffl get player-data --week 1
```

### Output formats

**Default output** (sorted by points, highest first):

```text
3918298 Josh Allen (QB) [week 1] 38.76 [Active] (Team Alpha)
3916387 Lamar Jackson (QB) [week 1] 35.44 [Active] (Team Beta)
15847 Travis Kelce (TE) [week 1] 18.20 [Questionable] (Team Gamma)
4426515 Puka Nacua (WR) [week 1] 15.90 [Active] (FA)
```

Output now includes:
- **Injury status**: `[Active]`, `[Questionable]`, `[Out]`, etc.
- **Roster status**: `(Team Name)` for rostered players, `(FA)` for free agents

**Projection analysis** with bias correction:

```bash
espn-ffl get projection-analysis -n "Josh Allen" -n "Travis Kelce" --week 4
```

```text
Projection Analysis & Predictions for Week 5
Season: 2025

Name                 Pos      ESPN     Adj      Final    Conf%    Reasoning
----                 ---      ----     ---      -----    ----     ---------
Josh Allen           QB       22.3     +4.5     26.7     32      % Recent trend shows ESPN underestimates by 5.1 pts (32% confidence), adjusted up 4.5 pts
Travis Kelce         TE       9.2      -1.3     7.9      41      % Recent trend shows ESPN overestimates by 1.5 pts (41% confidence), adjusted down 1.3 pts
```

**JSON output** for scripting/analysis:

```bash
espn-ffl get player-data -n "Josh Allen" --week 1 --json
```

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

### Advanced usage

```bash
# Debug mode - see the actual API request
espn-ffl get player-data --week 1 --debug

# Cache league settings for faster queries
espn-ffl get league-data --league-id 123456 --season 2024

# Projection analysis with custom bias strength
espn-ffl get projection-analysis --week 2 --bias-strength 1.5

# Get projection analysis as JSON for processing
espn-ffl get projection-analysis --week 2 --json
```

## Common workflows

**Check your lineup performance:**

```bash
# Get your starting QB and FLEX players for the week
espn-ffl get player-data -p QB -p FLEX --week 3

# Compare multiple players
espn-ffl get player-data -n "Josh Allen" -n "Lamar Jackson" -n "Patrick Mahomes" --week 1

# Check rostered players' injury status
espn-ffl get player-data --roster-status rostered --injury-status injured --week 1

# Get all players from a specific team
espn-ffl get player-data --team "Kenny Rogers" --week 1

# Check your opponent's active lineup
espn-ffl get player-data --team-id 456 --injury-status active --week 1
```

**Fantasy research and analysis:**

```bash
# Find the best available free agents at RB
espn-ffl get player-data -p RB --roster-status fa --injury-status active --week 1

# Check which rostered WRs are questionable
espn-ffl get player-data -p WR --roster-status rostered --injury-status questionable --week 1

# Find backup options if your starter is out
espn-ffl get player-data -p TE --roster-status fa --week 1
```

**Advanced projection analysis:**

```bash
# Analyze ESPN's projection accuracy for healthy QBs only
espn-ffl get projection-analysis -p QB --injury-status active --week 2

# Check bias correction for rostered players
espn-ffl get projection-analysis --roster-status rostered --week 3

# Compare projections for specific players
espn-ffl get projection-analysis -n "Travis Kelce" -n "Puka Nacua" --week 3
```

**Export for analysis:**

```bash
# Get all week 1 data as JSON with injury/roster info
espn-ffl get player-data --week 1 --json > week1_stats.json

# Export only rostered players for lineup analysis
espn-ffl get player-data --roster-status rostered --week 1 --json > my_roster.json

# Export projection analysis with filtering
espn-ffl get projection-analysis -p QB -p RB --week 2 --json > week2_projections.json
```

## CLI Reference

### Commands

#### `espn-ffl get player-data`

Get player statistics and fantasy points for a specific week.

**Core Options:**
- `-l, --league-id <ID>` - League ID (or set `ESPN_FFL_LEAGUE_ID` env var)
- `-s, --season <YEAR>` - Season year (default: 2025)
- `-w, --week <WEEK>` - Week number (default: 1)

**Filtering Options:**
- `-n, --player-name <NAME>` - Filter by player name (repeatable)
- `-p, --position <POS>` - Filter by position: QB, RB, WR, TE, K, DEF, FLEX (repeatable)
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
- `--team <NAME>` - Filter by team name (partial match, case-insensitive)
- `--team-id <ID>` - Filter by team ID

**Output Options:**
- `--json` - Output as JSON instead of text
- `--debug` - Show API request details
- `--proj` - Use projected points instead of actual

**Data Management:**
- `--refresh` - Force fresh data from ESPN API
- `--clear-db` - Clear local database before fetching
- `--refresh-positions` - Update player position mappings

#### `espn-ffl get projection-analysis`

Analyze ESPN projection accuracy and generate bias-corrected predictions.

**Core Options:**
- `-l, --league-id <ID>` - League ID (or set `ESPN_FFL_LEAGUE_ID` env var)
- `-s, --season <YEAR>` - Season year (default: 2025)
- `-w, --week <WEEK>` - Week number (default: 1)

**Filtering Options:**
- `-n, --player-name <NAME>` - Filter by player name (repeatable)
- `-p, --position <POS>` - Filter by position (repeatable)
- `--injury-status <STATUS>` - Filter by injury status (same options as player-data)
- `--roster-status <STATUS>` - Filter by roster status (same options as player-data)
- `--team <NAME>` - Filter by team name (same options as player-data)
- `--team-id <ID>` - Filter by team ID (same options as player-data)

**Analysis Options:**
- `--bias-strength <FLOAT>` - Bias correction strength (0.0-2.0+, default: 1.0)
- `--json` - Output as JSON
- `--refresh` - Force fresh data from ESPN API

#### `espn-ffl get league-data`

Cache league settings for faster subsequent queries.

- `-l, --league-id <ID>` - League ID
- `-s, --season <YEAR>` - Season year
- `--refresh` - Force refresh settings
- `--verbose` - Show detailed output

## Troubleshooting

**"Missing league ID" error**: Set `ESPN_FFL_LEAGUE_ID` environment variable or use `--league-id`

**Authentication errors**: Double-check your `ESPN_SWID` and `ESPN_S2` cookies are current

**No results**: Verify the week has games scheduled and use `--debug` to see the API request

**Build errors**: Ensure you have the latest stable Rust: `rustup update`

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
