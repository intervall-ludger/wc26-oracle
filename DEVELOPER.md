# Developer documentation

Technical reference for wc26-oracle. For the what-and-why, see the [README](README.md).

## Architecture

A single Rust binary (`wm2026`) reads JSON data, runs the Monte-Carlo simulation, and writes a static
`web/index.html`. A small Python script refreshes results from an API. A GitHub Action ties it together.

```
src/
  model.rs      Team / Result structs, JSON loading, group indexing
  bracket.rs    group fixtures + the full 2026 knockout bracket (M73–M104) and third-place slots
  sim.rs        Elo+form strength, Poisson/Dixon-Coles match model, group + knockout simulation, Tally
  analysis.rs   resolve played matches, predictions-vs-reality (Brier), --dump-schedule data
  history.rs    per-stage snapshots (the title-race timeline), dedup by results signature
  report.rs     renders the whole HTML report (tabs, tables, SVG bracket + chart, heat cells)
  main.rs       CLI flags, wiring, --dump-schedule mode
data/
  teams.json    48 teams: code, group, draw position, elo, form (+ form notes)
  results.json  real results entered/auto-pulled so far
  opta.json     Opta benchmark title odds (comparison only)
  history.json  saved snapshots (committed back by CI)
scripts/refresh.py   pulls finished matches from football-data.org into results.json
web/style.css        styles (report HTML is generated; index.html is gitignored)
```

## The model (`src/sim.rs`)

- **Strength** of a team = `elo + form_weight·form (+ home_adv for hosts) + dynamic delta`.
  - `form` is an Elo-point bonus/penalty from the last ~5–6 internationals (in `teams.json`).
  - **Dynamic Elo** (`--dyn-k`): during each simulated tournament the strength drifts after every match
    (Elo step scaled by goal margin, clamped). Real results feed this too, so a team on a hot streak
    carries momentum into the rest of the simulation.
- **Goals**: expected goals for each side come from the strength gap (`--supremacy`) around a goal
  baseline (`--total-goals`); goals are drawn from independent Poissons with a **Dixon-Coles**
  correction (`--dc-rho`) for realistic 0-0 / 1-0 / 1-1 frequencies. Knockout draws → elo-weighted
  shootout.
- **Group ranking**: points → goal difference → goals → random. The 8 best third-placed teams advance
  and are slotted into the bracket via a valid perfect matching.
- **Calibration**: parameters are tuned so the title spread roughly matches the Opta supercomputer
  (clear favourite ~16–20 % rather than an undamped ~30 %).

### CLI flags

| Flag             | Default              | Meaning                                          |
|------------------|----------------------|--------------------------------------------------|
| `--sims`         | `200000`             | Number of Monte-Carlo tournaments                |
| `--teams`        | `data/teams.json`    | Teams, groups, Elo and form                      |
| `--results`      | `data/results.json`  | Real results entered so far                      |
| `--opta`         | `data/opta.json`     | Opta benchmark odds (comparison column)          |
| `--history`      | `data/history.json`  | Timeline of saved stages                         |
| `--out`          | `web/index.html`     | HTML report path                                 |
| `--no-snapshot`  | off                  | Don't write a timeline snapshot this run         |
| `--supremacy`    | `0.0028`             | Goal difference per Elo point (model sharpness)  |
| `--total-goals`  | `2.6`                | Expected goals per match (both teams summed)     |
| `--home-adv`     | `70`                 | Elo home bonus for hosts USA/CAN/MEX             |
| `--form-weight`  | `1.0`                | Multiplier on each team's form value             |
| `--dc-rho`       | `-0.10`              | Dixon-Coles low-score correlation (0 = off)      |
| `--dyn-k`        | `8.0`                | In-tournament dynamic-Elo K-factor (0 = off)     |
| `--dump-schedule`| (none)               | Print determined fixtures as JSON, then exit     |

Tune without rebuilding: `cargo run --release -- --supremacy 0.0024 --form-weight 1.5`

## Data formats

### `data/teams.json`
Per team: `name`, `code` (3-letter), `group` (A–L), `pos` (1–4 draw position), `elo`, `form`
(Elo points), plus optional `form_last` / `form_note` for transparency. Edit freely.

### `data/results.json`
```json
{ "results": [
  { "match": "A:1v2", "home": 2, "away": 1 },
  { "match": "M73", "home": 1, "away": 1, "winner": "home", "decided": "pens" }
] }
```
- **Group match id** = `"<GROUP>:<homePos>v<awayPos>"`. Positions are the order in `teams.json`.
  Matchday order per group: MD1 `1v2`,`3v4` · MD2 `1v3`,`4v2` · MD3 `4v1`,`2v3`.
- **Knockout match id** = `"M<number>"` (M73–M104).
- `winner` (`"home"`/`"away"`) is only needed for a knockout tie level after the score.
- `decided` (`"90"`/`"aet"`/`"pens"`) is optional; it drives the bracket label (a.e.t. / pens).

Each distinct result state is snapshotted into `history.json` (deduped by a signature), which drives the
title-race chart and the Δ column. Re-running the same state just refreshes it; `--no-snapshot` skips it.

## Auto-update pipeline

`scripts/refresh.py` (stdlib only):
1. runs `wm2026 --dump-schedule` to get the currently-determined fixtures with team codes,
2. fetches finished World Cup matches from football-data.org (`/v4/competitions/WC/matches`),
3. maps each finished match to a fixture by team pair (orientation-aware), capturing the scoreline,
   the shootout winner, and how it was decided,
4. merges into `data/results.json`.

Team mapping is by football-data's `tla`, falling back to a name alias table, and all 48 teams resolve.
Without `FOOTBALL_DATA_KEY` the refresh is skipped and committed data is used. Local dev: put the key
in a gitignored `.env` (`FOOTBALL_DATA_KEY=...`).

## GitHub Actions (`.github/workflows/simulate.yml`)

Runs on push, manual dispatch, and daily at 00:00 UTC: build → refresh → simulate → commit
`results.json`/`history.json` back (`[skip ci]`, so it doesn't re-trigger) → deploy to Pages.

One-time repo setup:
1. **Settings → Secrets and variables → Actions → New secret** → `FOOTBALL_DATA_KEY`
   ([free key](https://www.football-data.org/client/register)).
2. **Settings → Actions → General → Workflow permissions → Read and write** (so the bot can commit back).
3. **Settings → Pages → Source: GitHub Actions**.

## Local development

```bash
cargo build --release
cargo test                                   # if/when Rust tests are added
./target/release/wm2026 --sims 200000        # writes web/index.html
open web/index.html
```

Rendering a tab to a PNG for review (headless Chrome):
```bash
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless --screenshot=/tmp/s.png \
  --window-size=1280,1600 "file://$PWD/web/index.html"
```
