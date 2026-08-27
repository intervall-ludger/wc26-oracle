# How the model did

The 2026 World Cup ended on July 19 with Spain beating Argentina 1-0 after extra time. This is the
post-tournament scorecard for the simulator: what it got right, where it was off, and what the
numbers say about the model itself.

All figures below come from the committed data (`data/results.json`, `data/history.json`) and from
the pre-tournament ratings in `data/teams.json`, which were never touched during the tournament. The
match odds the model is scored on use base Elo plus form only, with no in-tournament update, so every
one of the 103 forecasts is a genuine ex-ante prediction. Reproduce them with:

```bash
cargo run --release -- --dump-played
```

103 of the 104 matches are scored. The third-place play-off (England 6-4 France) is not part of the
model's bracket.

## The short version

The model named the champion before a ball was kicked, and its top four were exactly the four
semifinalists.

| Pre-tournament rank | Team | Model | Opta | Actual finish |
| --- | --- | --- | --- | --- |
| 1 | Spain | 21.2% | 16.1% | champion |
| 2 | Argentina | 13.6% | 10.4% | runner-up |
| 3 | France | 13.0% | 13.0% | 4th |
| 4 | England | 6.2% | 11.2% | 3rd |
| 5 | Brazil | 4.8% | 6.6% | round of 16 |
| 6 | Colombia | 3.6% | 2.1% | round of 16 |
| 7 | Germany | 3.5% | 5.1% | round of 32 |
| 8 | Portugal | 3.4% | 7.0% | round of 16 |
| 9 | Netherlands | 3.3% | 3.6% | round of 32 |
| 10 | Mexico | 3.1% | 1.0% | round of 16 |

The final was model #1 against model #2. Across the 35 saved snapshots Spain was the title favorite in
32 of them; France led briefly in three (after the round of 16 and again after the first quarterfinal).

## Match forecasts

Brier is the multiclass score on a 0 to 2 scale, lower is better. A uniform 1/3-1/3-1/3 guess scores
exactly 0.667, so skill is the improvement over that. Log loss compares against ln 3 = 1.099.

| Slice | n | Brier | Skill | Log loss | Correct |
| --- | --- | --- | --- | --- | --- |
| All matches | 103 | 0.520 | 22.0% | 0.882 | 68 (66.0%) |
| Group stage | 72 | 0.547 | 18.0% | 0.916 | 44 (61.1%) |
| Knockout | 31 | 0.458 | 31.3% | 0.805 | 24 (77.4%) |
| Group MD1 | 24 | 0.662 | 0.7% | 1.068 | 11 (45.8%) |
| Group MD2 | 24 | 0.484 | 27.4% | 0.820 | 18 (75.0%) |
| Group MD3 | 24 | 0.494 | 25.8% | 0.859 | 15 (62.5%) |

Matchday 1 was the tournament's chaos, and the model scored no better on it than a coin toss with
three sides. Ghana beat Panama, Spain drew 0-0 with Cape Verde, England drew 0-0 with Ghana, Qatar
took a point off Switzerland. From matchday 2 onward the picture is normal for a 1X2 forecast.

Counting only the 52 group matches that were actually decided, the model had the right side in 44 of
them (84.6%). Draws are where the score is lost, which is the usual story.

Knockout ties are better judged as two-way forecasts: split the draw mass proportionally and score
against the team that advanced.

| | Value | Reference |
| --- | --- | --- |
| Brier (2-way) | 0.312 | 0.500 for a coin flip |
| Log loss | 0.484 | 0.693 for a coin flip |
| Correct | 24 / 31 (77.4%) | |

Mean probability assigned to the team that actually advanced, by round: round of 32 65.2%, round of 16
59.6%, quarterfinals 68.9%, semifinals 57.3%, final 55.6%. From the quarterfinals on the model went
7 for 7.

The seven ties it called wrong:

| Match | Result | Model |
| --- | --- | --- |
| R32 Germany vs Paraguay | 1-1, Paraguay on pens | Germany 64% |
| R32 Netherlands vs Morocco | 1-1, Morocco on pens | Netherlands 60% |
| R16 Brazil vs Norway | 1-2 Norway | Brazil 57% |
| R32 Australia vs Egypt | 1-1, Egypt on pens | Australia 53% |
| R16 Canada vs Morocco | 0-3 Morocco | Canada 52% |
| R32 Belgium vs Senegal | 3-2 Belgium a.e.t. | Belgium 49% |
| R16 Switzerland vs Colombia | 0-0, Switzerland on pens | Switzerland 45% |

Only three of those were real misses. The other four were coin flips that the model happened to lean
the other way on, including two it "lost" while the team it slightly disfavored went through.

Biggest surprises overall, by the probability the model gave to what actually happened:

| Match | Result | P(actual) |
| --- | --- | --- |
| Ghana vs Panama (MD1) | 1-0 | 17.3% |
| England vs Ghana (MD2) | 0-0 | 18.5% |
| Spain vs Cape Verde (MD1) | 0-0 | 18.8% |
| Qatar vs Switzerland (MD1) | 1-1 | 19.3% |
| Ecuador vs Curacao (MD2) | 0-0 | 20.1% |

## Team-level forecasts

Champion forecast over all 48 teams, scored against the one realized champion:

| | Brier (48-way) | Log score |
| --- | --- | --- |
| Model | 0.673 | 1.551 |
| Opta | 0.761 | 1.826 |
| Uniform | 0.979 | 3.871 |

The model beat the Opta benchmark on both, but this is a single realization of a 48-way outcome, so it
is one data point and not evidence that the model is generally sharper. Opta was closer on England,
the model was closer on Argentina and Mexico.

Group qualification, as a binary forecast over all 48 teams:

| | Value | Baseline |
| --- | --- | --- |
| Brier | 0.175 | 0.222 (constant 32/48) |
| Log loss | 0.514 | 0.637 (constant 2/3) |

Taking the 32 teams with the highest qualification probability as the model's pick, 25 of the 32 real
qualifiers are in there. The seven it had in the top 32 that went out in the groups: Turkiye (81.7%),
Uruguay (80.5%), Iran (76.2%), South Korea (72.5%), Czechia (71.8%), Panama (68.0%), Scotland (64.5%).
The seven qualifiers it had outside: Australia (57.1%), Bosnia and Herzegovina (56.5%), Sweden (52.3%),
Cape Verde (47.2%), DR Congo (44.3%), South Africa (33.4%), Ghana (20.5%). Five of the fourteen were
above 55% either way, so most of the churn happened in the band where the model was close to
undecided anyway.

## Where the model was weak

### It was underconfident

Grouping all 309 probability-outcome pairs by the probability assigned:

| Predicted band | n | Mean predicted | Observed | Gap |
| --- | --- | --- | --- | --- |
| 0.0 - 0.1 | 10 | 7.2% | 0.0% | -7.2pp |
| 0.1 - 0.2 | 37 | 16.4% | 10.8% | -5.6pp |
| 0.2 - 0.3 | 134 | 26.1% | 18.7% | -7.5pp |
| 0.3 - 0.4 | 45 | 35.0% | 35.6% | +0.5pp |
| 0.4 - 0.5 | 29 | 45.0% | 62.1% | +17.1pp |
| 0.5 - 0.6 | 35 | 55.1% | 71.4% | +16.4pp |
| 0.6 - 0.7 | 9 | 64.4% | 100.0% | +35.6pp |
| 0.7 - 1.0 | 10 | 73.5% | 60.0% | -13.5pp |

The shape is systematic: probabilities in the 40 to 70 percent band came in well above forecast, and
the crowded 20 to 30 percent band (mostly draw probabilities) came in below. In short, favorites won
more often than the model expected and the model was too flat.

Re-scoring the same 103 matches over a parameter grid points the same way. `--supremacy` controls how
much an Elo gap turns into a goal gap, and the shipped value of 0.0028 is roughly half of what the
tournament wanted:

| supremacy | Brier | Skill | Log loss |
| --- | --- | --- | --- |
| 0.0028 (shipped) | 0.520 | 22.0% | 0.882 |
| 0.0040 | 0.500 | 25.1% | 0.849 |
| 0.0056 | 0.487 | 27.0% | 0.830 |
| 0.0072 | 0.490 | 26.5% | 0.850 |

This is an in-sample fit on 103 matches, so treat 0.0056 as a direction rather than a calibrated
value. The number of correctly called matches (68) does not move anywhere in that range, which is
what you would expect: sharpening the probabilities does not reorder the favorites.

### Too few goals

| | Model | Actual |
| --- | --- | --- |
| Goals per match (all 103) | 2.60 | 2.89 |
| Goals per match (group stage) | 2.60 | 2.99 |
| Home goals per match | 1.41 | 1.70 |
| Draws | 27.1 expected | 24 |
| Goal-difference MAE | 1.29 | |
| Exact score from rounded xG | 8 / 103 (7.8%) | |

`--total-goals 2.6` was about 0.3 goals per match low. Raising it does not help the 1X2 score on its
own (more goals means flatter outcome odds), so the two knobs need to move together: more goals and
more supremacy.

## Data and pipeline

The bracket logic came out clean. Reconstructing all 31 knockout ties from the official results and
comparing them against the fixtures the simulator derives itself, every pairing and every home/away
orientation matched, from the round of 32 through the final. That validates the group tie-breakers and
the Annex C third-place allocation end to end, not just in the unit sense.

The daily job ran 89 times with 88 green runs and one failure, about a minute per run, and committed
results, schedule and history back 59 times after the last hand-written commit. No manual work during
the tournament, which was the point.

Two defects worth writing down:

**Shootout goals are folded into the score.** `scripts/refresh.py` copies the provider's `fullTime`
score verbatim, and for penalty shootouts that score includes the shootout. Four matches are stored
wrong:

| Match | Stored | Actual |
| --- | --- | --- |
| M74 Germany vs Paraguay | 4-5 | 1-1, 3-4 on pens |
| M75 Netherlands vs Morocco | 3-4 | 1-1, 2-3 on pens |
| M88 Australia vs Egypt | 3-5 | 1-1, 2-4 on pens |
| M96 Switzerland vs Colombia | 4-3 | 0-0, 4-3 on pens |

The bracket still advances the right team, because the inflated score points the same way as the
shootout. The visible damage is the scoreline shown on the page and the goal statistics: with the
shootout goals in, the tournament looks like 3.14 goals per match instead of 2.89, and four draws are
counted as decided matches. The effect on the Brier score is small (0.519 with the stored scores,
0.520 with the corrected ones). The fix is to use the provider's regular-time score and set `winner`
from `score.winner`.

**A latent hole in the hit counter.** `accuracy()` in `src/analysis.rs` counts a match as correctly
called only if the model's top pick was home or away. A match whose most likely outcome is a draw can
never be counted right, even when it ends level. It never fired here, because the model's draw
probability peaked at 28.9% and a draw was never the modal outcome in any of the 103 matches, but the
counter is wrong as written.

## Compute

Single-threaded, on an M-series Mac, simulating the full tournament from scratch:

| Simulations | Wall time |
| --- | --- |
| 50,000 | 1.69 s |
| 200,000 | 6.77 s |
| 500,000 | 17.03 s |

That is about 29,000 tournaments per second, or 3.1 million simulated matches per second. The binary
is 788 KB, the generated page 120 KB, the whole thing about 2,600 lines of Rust. Runtime was never a
constraint: the daily CI run spent more time installing the toolchain than simulating.

## What this says about the model

For a model whose entire input is one Elo number and one form number per team, the result is better
than it has any right to be. It named the champion, the finalist and the semifinal field before the
tournament started, and it beat a uniform guess by 22% on individual matches.

The one clear structural finding is that it was too timid. Elo gaps at World Cup level translate into
bigger goal gaps than the shipped `supremacy` assumed, and World Cup matches have more goals than the
shipped `total-goals` assumed. Both point in the same direction: the next edition should be sharper
and higher-scoring. One tournament of 103 matches is not enough to calibrate that properly, but it is
enough to know the sign.
