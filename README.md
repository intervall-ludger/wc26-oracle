# World Cup 2026 — Monte Carlo Simulation

Simulates the 2026 FIFA World Cup (48 teams, 12 groups) with an **Elo + form + Poisson** model and
runs thousands of full tournaments to estimate every team's chance of winning the trophy.

As real results come in, you enter them after each matchday: played matches are then **fixed** and
only the rest is re-simulated. Every run is **snapshotted into a timeline**, so the site shows how the
odds shift across the tournament — and a **Predictions vs Reality** section scores how well the model
actually called the matches that have been played (including the biggest upsets it missed).

## Quick start

```bash
cargo run --release -- --sims 200000
```

Prints the top 10 to the console and writes an HTML report to `web/index.html`
(open it locally: `open web/index.html`).

| Flag             | Default              | Meaning                                         |
|------------------|----------------------|-------------------------------------------------|
| `--sims`         | `200000`             | Number of Monte-Carlo tournaments               |
| `--teams`        | `data/teams.json`    | Teams, groups, Elo and form                     |
| `--results`      | `data/results.json`  | Real results entered so far                     |
| `--opta`         | `data/opta.json`     | Opta benchmark odds (comparison column)         |
| `--history`      | `data/history.json`  | Timeline of saved stages                        |
| `--out`          | `web/index.html`     | HTML report path                                |
| `--no-snapshot`  | off                  | Don't write a timeline snapshot this run        |
| `--supremacy`    | `0.0028`             | Goal difference per Elo point (model sharpness) |
| `--total-goals`  | `2.6`                | Expected goals per match (both teams summed)    |
| `--home-adv`     | `70`                 | Elo home bonus for hosts USA/CAN/MEX            |
| `--form-weight`  | `1.0`                | Multiplier on each team's form value            |
| `--dc-rho`       | `-0.10`              | Dixon-Coles low-score correlation (0 = off)     |
| `--dyn-k`        | `8.0`                | In-tournament dynamic-Elo K-factor (0 = off)    |

Tune without rebuilding: `cargo run --release -- --supremacy 0.0024 --form-weight 1.5`

## Entering results

Add entries to `data/results.json` under `results`, then re-run.

**Group match** — `match` id = `"<GROUP>:<homePos>v<awayPos>"`.
Position (1–4) is the order in `data/teams.json`. Match order per group:

- Matchday 1: `1v2`, `3v4`
- Matchday 2: `1v3`, `4v2`
- Matchday 3: `4v1`, `2v3`

```json
{
  "results": [
    { "match": "A:1v2", "home": 2, "away": 1 },
    { "match": "A:3v4", "home": 0, "away": 0 }
  ]
}
```

**Knockout match** — `match` id = `"M<number>"` (M73–M104). For a draw decided on penalties,
add the shootout winner:

```json
{ "match": "M73", "home": 1, "away": 1, "winner": "home" }
```

Each distinct result state is saved as one snapshot in `data/history.json` (deduplicated), which
drives the title-race chart and the Δ column. Re-running the same state just refreshes it.

## What the report shows

The page is organised into tabs (pure CSS, no JS): **Bracket · Groups · Odds · Path · Fixtures · Accuracy**.

- **Bracket** — the most likely team in each knockout slot with its probability of getting there, plus the
  most likely scoreline of every tie on the connectors. Fixed once real results arrive. Includes the Title
  Race line chart (championship probability of the current top 6 across every saved stage; SVG, no JS).
- **Groups** — 12 group cards with live standings, each team's chance to reach the knockout (Qualify%,
  heat-shaded), the top-2 zone highlighted, plus the full **Matchday 1/2/3 schedule** per group with the
  real result or the most likely scoreline. Before kickoff the points column shows expected points (xPts).
- **Odds** — the full 48-team title table (heat-shaded): Elo, form, title %, Δ since last stage, Opta
  benchmark + gap, and final/semi/QF/knockout probabilities.
- **Path** — for each team, the most likely opponent in each knockout round; cell shading shows the
  team's chance of reaching that round, so the row fades as the path gets harder.
- **Fixtures** — the most probable scoreline for every group match (mode of the Poisson model) with
  home/draw/away odds; played matches show the real result.
- **Accuracy** — predictions vs reality: per-match odds vs the actual outcome, Brier score, skill %, upsets.
- **Current Odds** — per team: Elo, form, title %, Δ since the previous stage, the Opta benchmark and
  our gap to it, plus final / semi / QF / knockout probabilities.
- **Predictions vs Reality** — for every played match: the model's pre-match home/draw/away odds, the
  actual score, and the probability it gave to what actually happened. A Brier score and a skill % say
  how well the model is doing overall; rows are sorted by surprise so the worst calls surface first.

## Model

- **Strength** per team = Elo + form bonus (+ home bonus for hosts), plus an optional **in-tournament
  dynamic-Elo** delta (`--dyn-k`) that nudges a team up/down as the tournament unfolds, so real results
  carry momentum into the remaining simulation.
- Expected goals for both sides from the strength gap (`--supremacy`) and a goals baseline (`--total-goals`).
- Goals per team drawn from a **Poisson** distribution with a **Dixon-Coles** correction (`--dc-rho`)
  for more realistic low-scoring results and draws.
- Group tables ranked by points → goal difference → goals → random.
- The 8 best third-placed teams advance and are slotted via a valid bracket matching.
- Knockout draws: penalty shootout, Elo-weighted.

Parameters are calibrated against the **Opta supercomputer** (title leader ~16–20 % instead of an
undamped ~30 %).

### Data

- **Elo** (`data/teams.json`): eloratings.net scale, ~June 2026. Freely editable.
- **Form** (`data/teams.json`, field `form`): Elo bonus/penalty from the last ~5–6 internationals
  (`form_last` / `form_note` document the streak). Best-effort estimate — some pre-tournament
  friendlies are speculative; edit freely. `0` = neutral.
- **Opta** (`data/opta.json`): Opta supercomputer title odds, used only as a benchmark.

## GitHub Pages & daily auto-update

`.github/workflows/simulate.yml` runs on every push, on **manual dispatch**, and **daily at 00:00 UTC**.
Each run: pulls fresh results → re-simulates → publishes to GitHub Pages → commits the updated
`data/results.json` and `data/history.json` back (so results and the timeline persist).

Setup: repo → **Settings → Pages → Source: GitHub Actions**.

### Automatic results (football-data.org)

`scripts/refresh.py` pulls finished World Cup matches from [football-data.org](https://www.football-data.org)
(the free tier includes the FIFA World Cup) and writes them into `data/results.json`, mapped onto the
fixtures via the binary's `--dump-schedule`.

1. Register for a free key at [football-data.org/client/register](https://www.football-data.org/client/register).
2. Repo → **Settings → Secrets and variables → Actions → New secret** → `FOOTBALL_DATA_KEY`.

Without the secret the refresh step is skipped and the committed `results.json` is used, so the
project still runs fully. Scores on the free tier are slightly delayed, which is fine for a daily run.
Locally, put the key in a `.env` (`FOOTBALL_DATA_KEY=...`) and run `python3 scripts/refresh.py`.
