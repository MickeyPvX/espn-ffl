# ESPN Fantasy Football Stats CLI

A Rust command-line tool for fetching **fantasy football player stats** from the ESPN Fantasy Football API (private or public leagues).  
This project is designed to query ESPN’s hidden API with flexible filters, making it easy to pull player information into scripts, databases, or dashboards.

---

## Features

- Query ESPN Fantasy Football stats programmatically.
- Filter by:
  - **Player name**
  - **Position(s)** (QB, RB, WR, TE, K, D/ST, FLEX)
  - **Active status**
- Works with **private leagues** using ESPN cookies (`SWID` and `espn_s2`).
- Supports specifying **season** and **week**.
- CLI built with [`structopt`](https://crates.io/crates/structopt).
- JSON response parsing with [`serde_json`](https://docs.rs/serde_json).
- Ready for expansion (e.g., saving results to Postgres).

---

## Installation

### Prerequisites
- Rust (stable)  
- Cargo  
- ESPN Fantasy Football account  

### Build

```bash
git clone <repo-url>
cd espn-ffl
cargo build --release
```

---

## Usage

The CLI uses `structopt` for parsing. Example:

```bash
./target/release/espn-ffl get   --league-id 123456   --season 2025   --week 3   -p QB -p WR -p RB
```

### Arguments

- `--league-id, -l`  
  ESPN league ID (falls back to `$ESPN_FFL_LEAGUE_ID` env var).

- `--player-name, -n`  
  Filter players by last name.

- `--positions, -p`  
  One or more player positions (`QB`, `RB`, `WR`, `TE`, `K`, `D`/`DEF`, `FLEX`).

- `--season, -s`  
  Season year (default: `2025`).

- `--week, -w`  
  Scoring period/week.

---

## Private Leagues: Cookies

To access **private leagues**, ESPN requires authentication cookies. You need two values:

- `SWID`
- `espn_s2`

### How to Find Them

1. **Log in** to ESPN Fantasy Football in your browser.
2. Open **Developer Tools** (usually `F12` or `Ctrl+Shift+I`).
3. Go to the **Application** tab (in Chrome/Edge) or **Storage** tab (in Firefox).
4. In the left sidebar, expand **Cookies** and select `https://www.espn.com`.
5. Find the entries for:
   - `SWID`
   - `espn_s2`
6. Copy the full values exactly as shown.

⚠️ **Notes:**
- `SWID` usually looks like a **UUID in curly braces**, e.g. `{12345678-90AB-CDEF-1234-567890ABCDEF}`.
- `espn_s2` is a long alphanumeric string.

### Set Them as Environment Variables

```bash
export ESPN_SWID="{12345678-90AB-CDEF-1234-567890ABCDEF}"
export ESPN_S2="AEB3VYx3aLz0N...rest_of_string..."
```

The CLI will automatically use these values when making requests.

---

## Example Queries

### Get all QBs for Week 1 of 2025

```bash
espn-ffl get -l 123456 -s 2025 -w 1 -p QB
```

### Get all WRs named “Smith”

```bash
espn-ffl get -l 123456 -s 2025 -w 2 -p WR -n Smith
```

### Get all active players (default)

```bash
espn-ffl get -l 123456 -s 2025 -w 3
```

---

## Output

Currently the CLI prints player names (with `serde_json::Value` parsing):

```
"Patrick Mahomes"
"Justin Jefferson"
"Christian McCaffrey"
```

This can be expanded later into **structured output** (CSV, JSON, or database insertions).

---

## Next Steps

- [ ] Add **Postgres integration** for persistent storage.
- [ ] Expand `view` support (`mDraftDetail`, `modular`, etc.).
- [ ] Strongly typed structs for deserializing player data.
- [ ] Add subcommands for different ESPN API endpoints.
- [ ] Support exporting results to CSV/JSON.

---

## License

MIT License © 2025
