# ESPN Fantasy Football CLI

[![CI](https://github.com/MickeyPvX/espn-ffl/workflows/CI/badge.svg)](https://github.com/MickeyPvX/espn-ffl/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/MickeyPvX/espn-ffl/branch/main/graph/badge.svg)](https://codecov.io/gh/MickeyPvX/espn-ffl)

A fast, reliable command-line tool for querying ESPN Fantasy Football player statistics and advanced projection analysis. Built in Rust for performance and type safety.

## What it does

- **Query player stats** by name, position, or both (supports multiple filters)
- **Get actual or projected points** for any week and season
- **Projection analysis** - Compare ESPN projections vs. actual performance with bias correction
- **FLEX position support** - Filter by FLEX to get RB/WR/TE players
- **Export data** as JSON for analysis or integration
- **Cache league settings** for faster subsequent queries

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
# Get all players for week 3 of 2024 season
espn-ffl get player-data --week 3 --season 2024

# Find specific players (repeatable)
espn-ffl get player-data -n "Josh Allen" -n "Travis Kelce" --week 1

# Get all quarterbacks and wide receivers
espn-ffl get player-data -p QB -p WR --week 2

# Get FLEX-eligible players (RB/WR/TE)
espn-ffl get player-data -p FLEX --week 1

# Get projected points instead of actual
espn-ffl get player-data --week 1 --proj
```

### Specify a league

```bash
# Use specific league ID
espn-ffl get player-data --league-id 123456 --week 1

# Or set as environment variable
export ESPN_LEAGUE_ID=123456
espn-ffl get player-data --week 1
```

### Output formats

**Default output** (sorted by points, highest first):

```text
3918298 Josh Allen (QB) [week 1] 38.76
3916387 Lamar Jackson (QB) [week 1] 35.44
15847 Travis Kelce (TE) [week 1] 18.20
4426515 Puka Nacua (WR) [week 1] 15.90
```

**Projection analysis** with bias correction:

```bash
espn-ffl get projection-analysis -n "Josh Allen" -n "Travis Kelce" --week 4
```

```text
Name                 Pos      ESPN     Adj      Final    Conf%
----                 ---      ----     ---      -----    ----
Josh Allen           QB       22.7     +1.9     24.5     19      %
Travis Kelce         TE       11.3     -1.7     9.6      29      %
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
    "points": 38.76
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
```

**Advanced projection analysis:**

```bash
# Analyze ESPN's projection accuracy for top QBs
espn-ffl get projection-analysis -p QB --week 2

# Check bias correction for specific players
espn-ffl get projection-analysis -n "Travis Kelce" -n "Puka Nacua" --week 3
```

**Export for analysis:**

```bash
# Get all week 1 data as JSON
espn-ffl get player-data --week 1 --json > week1_stats.json

# Export projection analysis
espn-ffl get projection-analysis --week 2 --json > week2_projections.json
```

## Troubleshooting

**"Missing league ID" error**: Set `ESPN_LEAGUE_ID` environment variable or use `--league-id`

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
