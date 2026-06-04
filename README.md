# wc26-oracle — World Cup 2026 simulator

**Live site → https://intervall-ludger.github.io/wc26-oracle**

A statistical oracle for the 2026 FIFA World Cup (48 teams, 12 groups). It plays the whole
tournament out hundreds of thousands of times and turns that into one clear picture: how likely each
team is to top its group, reach each knockout round, and lift the trophy. It **updates itself every
day** with the real results as they happen.

## What it does

- Rates every team by **Elo + recent form** and simulates each match with a **Poisson** goal model
  (Dixon-Coles adjusted), running the full 104-match tournament as a Monte-Carlo experiment.
- Once a match is actually played, that result is **locked in** and only the remaining games are
  re-simulated — so you can watch the odds shift after every matchday.
- Keeps a **timeline** of every stage, so the site shows how the title race moved over time, and
  scores **how well the model called the games** that have already happened.

## On the site (tabs)

- **Groups** — live group tables, each group's matchday fixtures, and every team's chance to qualify.
- **Bracket** — the most likely knockout path, with real scores (and 90′ / a.e.t. / pens) as they come in.
- **Odds** — title and per-round probabilities for all 48 teams, next to the Opta benchmark.
- **Path** — for each team, the opponent it is most likely to meet in each round.
- **Fixtures** — the most probable scoreline for every group match.
- **Accuracy** — predictions vs reality: a Brier score, a skill %, and the biggest upsets the model missed.

## Staying current

A GitHub Action runs daily and pulls finished matches from the free
[football-data.org](https://www.football-data.org) tier, re-runs the simulation, and republishes the
page — no manual work during the tournament.

## Run it locally

```bash
cargo run --release -- --sims 200000
open web/index.html
```

Built in Rust (fast enough to run 250k tournaments in a few seconds). The model is calibrated against
the Opta supercomputer.

## Documentation

- **[DEVELOPER.md](DEVELOPER.md)** — architecture, the model, data formats, entering results by hand,
  CLI flags, the auto-update pipeline, and the GitHub setup.

## Data

Groups from the FIFA 2026 final draw · Elo from the eloratings.net scale · live results from
football-data.org · Opta figures used as a comparison benchmark only.
