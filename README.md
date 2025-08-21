# ESPN Fantasy Football CLI

A command-line tool written in Rust for fetching **player stats** from the private ESPN Fantasy Football API.  
This project is designed for **data collection, analysis, and future persistence** into databases like PostgreSQL.  

---

## Features

- 🔎 **Query players** by name, position(s), or both.  
- 📅 **Filter by season and week(s)** (single or multiple).  
- 🏈 **Outputs player ID, name, and weekly points**.  
- 📦 Optional `--json` flag for machine-readable output.  
- 🔑 Supports **private leagues** using cookies (`SWID` and `espn_s2`).

---

## Installation

```bash
git clone https://github.com/yourusername/espn-ffl.git
cd espn-ffl
cargo build --release
```

The compiled binary will be located in `target/release/espn-ffl`.

---

## Authentication: Cookies

Private leagues require authentication using cookies (`SWID` and `espn_s2`).  

### Step 1: Open your browser’s Developer Tools

- Go to your ESPN Fantasy Football league page.  
- Log in if necessary.  

### Step 2: Inspect Network Requests

- Open **Developer Tools** (F12 or right-click → Inspect).  
- Navigate to the **Network** tab.  
- Reload the page and look for requests to:

  ```bash
  https://fantasy.espn.com/apis/v3/games/ffl/...
  ```

### Step 3: Find the Cookies

- In the request **Headers**, locate the `cookie` field.  
- Copy values for:
  - `SWID={...}`  
  - `espn_s2={...}`  

### Step 4: Store Them as Environment Variables

```bash
export ESPN_SWID="{your-swid-here}"
export ESPN_S2="{your-espn_s2-here}"
```

This way the CLI automatically attaches your credentials.

---

## Usage

### Basic Command

```bash
espn-ffl get --league-id 123456 --week 3 --season 2024
```

### Query by Player Name

```bash
espn-ffl get --league-id 123456 -n "Patrick Mahomes" --week 5
```

### Query by Position(s)

```bash
espn-ffl get --league-id 123456 -p QB -p WR --week 2
```

### Multi-Week Query

```bash
espn-ffl get --league-id 123456 --week 2 --week 3 --week 4
```

### JSON Output

```bash
espn-ffl get --league-id 123456 -n "Josh Allen" --week 1 --json
```

---

## Example Output

**Default (human-readable):**

```bash
12345 Patrick Mahomes [{ week: 3, points: 27.4 }]
67890 Travis Kelce [{ week: 3, points: 18.7 }]
```

**With `--json`:**

```json
[
  {
    "id": 12345,
    "name": "Patrick Mahomes",
    "weeks": [
      { "week": 3, "points": 27.4 }
    ]
  },
  {
    "id": 67890,
    "name": "Travis Kelce",
    "weeks": [
      { "week": 3, "points": 18.7 }
    ]
  }
]
```

---

## Next Steps

- Persist results into **PostgreSQL** for later querying.  
- Extend filters (injury status, active players, etc.).  
- Add support for **team-level** stats and matchups.  
