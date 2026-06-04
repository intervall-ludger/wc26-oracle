# wc26-oracle

A simulator for the 2026 World Cup. It plays the whole tournament out hundreds of thousands of times
and turns that into one clear picture: how likely each of the 48 teams is to top its group, reach each
knockout round, and win the trophy. Every day it pulls in the real results so far and updates itself.

**Live site:** https://intervall-ludger.github.io/wc26-oracle

## How it works

Every team gets a strength rating from its Elo and recent form. Each match is then simulated with a
Poisson goal model, and the full 104 match tournament runs as a Monte Carlo experiment. Once a match
has actually been played, that result is locked in and only the games still to come get re-simulated.
So after every matchday the odds shift to match what really happened.

It also keeps a snapshot of each stage, so you can see how the title race moved over time, and it
scores how well the model called the games that have already been played.

## What you see

The page has six tabs:

- **Groups**: live group tables, each group's matchday fixtures, and every team's chance to qualify.
- **Bracket**: the most likely knockout path, with real scores (and 90', extra time or penalties) as they land.
- **Odds**: title and per-round probabilities for all 48 teams, next to the Opta benchmark.
- **Path**: the opponent each team is most likely to meet in every round.
- **Fixtures**: the most probable scoreline for every group match.
- **Accuracy**: predictions against reality, with a score for how well the model did and the biggest upsets it missed.

## Staying current

A GitHub Action runs once a day. It pulls finished matches from the free football-data.org tier,
re-runs the simulation, and republishes the page. No manual work during the tournament.

## Run it yourself

```bash
cargo run --release -- --sims 200000
open web/index.html
```

It is written in Rust, so 250,000 tournaments take a few seconds. The model is calibrated against the
Opta supercomputer.

## More

See [DEVELOPER.md](DEVELOPER.md) for the architecture, the model in detail, the data formats, how to
enter results by hand, all the CLI flags, and the auto-update setup.

Groups come from the FIFA 2026 final draw, Elo from the eloratings.net scale, live results from
football-data.org. Opta figures are shown for comparison only.
