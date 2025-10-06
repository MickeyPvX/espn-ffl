# ESPN Fantasy Football CLI

[![CI](https://github.com/MickeyPvX/espn-ffl/workflows/CI/badge.svg)](https://github.com/MickeyPvX/espn-ffl/actions/workflows/ci.yml)

A fast, reliable command-line tool for querying ESPN Fantasy Football player statistics and advanced projection analysis. Built in Rust for performance and type safety.

## What it does

- **Query player stats** by name, position, team, injury/roster status
- **Get actual or projected points** for any week and season
- **Projection analysis** - ESPN projection accuracy with bias correction
- **Smart filtering** - Server-side filtering for performance
- **Export data** as JSON for analysis or integration
- **Database caching** - Local storage for faster queries

Perfect for fantasy football analysis, lineup optimization, and projection research.

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

### Quick Start

```bash
# Get all players for week 3
espn-ffl player-data --week 3

# Find specific players
espn-ffl player-data -n "Josh Allen" -n "Travis Kelce" --week 1

# Get quarterbacks and wide receivers
espn-ffl player-data -p QB -p WR --week 2

# Get projection analysis with bias correction
espn-ffl projection-analysis --week 5
```

### Team Filtering

```bash
# Filter by team name (partial matching)
espn-ffl player-data --team kenny --week 1

# Filter by exact team ID
espn-ffl player-data --team-id 123 --week 1

# Works with projection analysis too
espn-ffl projection-analysis --team kenny --week 5
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
