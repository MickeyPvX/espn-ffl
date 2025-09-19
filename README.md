# ESPN Fantasy Football CLI

[![CI](https://github.com/MickeyPvX/espn-ffl/workflows/CI/badge.svg)](https://github.com/MickeyPvX/espn-ffl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://MickeyPvX.github.io/espn-ffl/badges/coverage.json)](https://github.com/MickeyPvX/espn-ffl/actions/workflows/coverage.yml)

A fast, reliable command-line tool for querying ESPN Fantasy Football player statistics and points. Built in Rust for performance and type safety.

## What it does

- **Query player stats** by name, position, or both
- **Get actual or projected points** for any week and season
- **Filter results** to find exactly what you need
- **Export data** as JSON for analysis or integration
- **Cache league settings** for faster subsequent queries

Perfect for fantasy football analysis, automation, or just checking how your players performed.

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

# Find a specific player
espn-ffl get player-data -n "Josh Allen" --week 1

# Get all quarterbacks and wide receivers
espn-ffl get player-data -p QB -p WR --week 2

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
3139477 Josh Allen [week 1] 24.12
4361370 Lamar Jackson [week 1] 22.88
2330 Aaron Rodgers [week 1] 19.44
```

**JSON output** for scripting/analysis:

```bash
espn-ffl get player-data -n "Josh Allen" --week 1 --json
```

```json
[
  {
    "id": 3139477,
    "name": "Josh Allen",
    "week": 1,
    "projected": false,
    "points": 24.12
  }
]
```

### Advanced usage

```bash
# Debug mode - see the actual API request
espn-ffl get player-data --week 1 --debug

# Cache league settings for faster queries
espn-ffl get league-data --league-id 123456 --season 2024
```

## Common workflows

**Check your lineup performance:**

```bash
# Get your starting QB and RBs for the week
espn-ffl get player-data -p QB -p RB --week 3
```

**Compare projections vs actual:**

```bash
# Projected points
espn-ffl get player-data -n "Patrick Mahomes" --week 3 --proj

# Actual points
espn-ffl get player-data -n "Patrick Mahomes" --week 3
```

**Export for analysis:**

```bash
# Get all week 1 data as JSON
espn-ffl get player-data --week 1 --json > week1_stats.json
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
