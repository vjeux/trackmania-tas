# Training - 10 Long — author time beaten on a keyboard

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS, unconstrained** | **13.071** | **−0.009** | **−0.010** |
| **TAS, keyboard only** | **13.075** | −0.005 | −0.006 |
| TAS, 5-level pad | 13.074 | −0.006 | −0.007 |
| **TAS, keyboard, no input shorter than the WR's own shortest** | **13.074** | −0.006 | −0.007 |
| Author time (never beaten by a human) | 13.080 | — | −0.001 |
| Human WR — in-.- | 13.081 | +0.001 | — |

TMX map [191465](https://trackmania.exchange/maps/191465) · uid
`kpOLuGFTMICPkW7gp383PEQ_0A2` · author **in-.-** · **856 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## Why this one matters

The author time here is the author's own editor validation lap, and their best
public attempt sits 1 ms behind it. 856 people have ground this map.

The keyboard-only run — steering restricted to `{-127, 0, +127}`, nothing else —
comes in at **13.075, five milliseconds under an author time that has never
fallen**, using the exact input alphabet a keyboard player already has.

A 5-level pad tape matches 13.074. That is worth stating plainly: **the time was
never hiding in analog resolution.** It is hiding in *what* you steer, not *how
finely*. The unconstrained floor is 13.071, three milliseconds below that, and
those three milliseconds are the only thing analog buys on this map.

## What it does differently

Same route, same lanes, no air phase anywhere on the map. Two contributions:

- **≈1.3 ms** — clipping booster 3 about **one metre tighter** than any of the
  14 measured human runs.
- **≈4.9 ms** — being **quieter on the wheel** through the last 448 m. The human
  field puts in *eight full-lock corrections in the final 1.3 seconds*. This run
  does not.

So the coaching point is unusually simple: **stop sawing at the wheel on the
run-in, and take booster 3 a metre tighter.**

Classification: **known but unheld** — this is not a secret line, it is a
discipline problem. Which is exactly why a keyboard tape can do it.

## Validation

Three cold passes in fresh processes with the human world record carried as a
known-answer control (13081 every pass). Transcript and sha256 in
`notes/VALIDATION.md`.

Distinct search roots throughout, so the cross-contamination bug that affected
other work in this project never applied here.

## A note on the search

A plain search reached 13.080 in nine seconds and then sat there for 240,000
evaluations. The reason is that integer milliseconds are a hopelessly coarse
score on this map — at the finish speed of 858 km/h, **1 ms is 24 cm of
travel**, so almost every candidate reports the same millisecond and the search
has nothing to climb.

Adding a **sub-tick timing plane** — reading the interpolated crossing of a
fixed plane inside the tick, so finisher scores are microseconds rather than
milliseconds — took the same seed from 13.081 to 13.077 in 77 seconds.

## Files

| file | what |
|---|---|
| `replays/WIP_keyboard.Ghost.Gbx` | **13075, keyboard only** — the one worth studying |
| `replays/TAS_13074_analog.Ghost.Gbx` | 13074, unconstrained |
| `replays/WIP_pad5.Ghost.Gbx` | 13074, steering in {-127,-64,0,64,127} |
| `inputs/TAS_13074_analog.inputs.tsv` | per-tick inputs for the analog run |
| `inputs/human_WR_13081.inputs.tsv` | the human world record's inputs, for comparison |
