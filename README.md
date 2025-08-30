# ESPN Fantasy Football CLI

A command-line tool written in Rust for fetching **player stats** from the private ESPN Fantasy Football API.

---

## Features

- 🔎 **Query players** by name, position(s), or both.  
- 📅 **Filter by season and week** (single week at a time).  
- 🏈 **Outputs player ID, name, and weekly points**.  
- 📦 Optional `--json` flag for machine-readable output.  
- 🔑 Supports **private leagues** using cookies (`SWID` and `espn_s2`).  
- 📊 **Sorts results by points (descending)** for quick analysis.  
- ⚡ **Supports projected points** with `--proj`.
- 🗄️ **Caches league settings** (`mSettings`) in `~/.cache/espn-ffl` for reuse across queries.  
- 🐞 **Debug mode**: use `--debug` to print the URL and headers of the request.

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

### JSON Output

```bash
espn-ffl get --league-id 123456 -n "Josh Allen" --week 1 --json
```

### Projected Points

```bash
espn-ffl get --league-id 123456 --week 1 --proj
```

### Cache League Settings

```bash
# Fetch and cache league scoring/roster settings in ~/.cache/espn-ffl
espn-ffl get league-data --league-id 123456 --season 2024
```

### Debug Mode

```bash
espn-ffl get --league-id 123456 --week 1 --debug
```

This will print the full URL and request headers before executing the query.

---

## Example Output

**Default (human-readable, sorted by points):**

```bash
12345 Patrick Mahomes [week 3] 27.40
67890 Travis Kelce [week 3] 18.70
13579 Tyreek Hill [week 3] 16.25
```

**With `--json`:**

```json
[
  {
    "id": 12345,
    "name": "Patrick Mahomes",
    "week": 3,
    "projected": false,
    "points": 27.4
  },
  {
    "id": 67890,
    "name": "Travis Kelce",
    "week": 3,
    "projected": false,
    "points": 18.7
  }
]
```
