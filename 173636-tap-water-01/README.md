# Tap water 01 — the author time falls by 1.253 s, and the map is a duty-cycle contest

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **22.072** | **−1.253** | **−1.566** |
| TAS at a 40 ms input grain | 23.125 | −0.200 | −0.513 |
| 1-minimal, 747 events | 23.183 | −0.142 | −0.455 |
| **a uniform 2-on/1-off tap, no search at all** | **23.335** | −0.303 vs the WR | −0.303 |
| Author time (never beaten by a human) | 23.325 | — | −0.313 |
| Human WR — Reddnox, who is also the author | 23.638 | +0.313 | — |

TMX map [173636](https://trackmania.exchange/maps/173636) · **602 recorded
runs** — one of the most hunted maps in this collection.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## The map is a throttle duty-cycle contest

One straight 1:2 ramp, no checkpoints, essentially one-dimensional. Steering
through the glide is **provably inert** — zeroing it over the *entire* glide
returns the identical millisecond.

So the only thing that matters is how much of the time the accelerator is down.
And that single number orders the whole leaderboard:

| rank | throttle duty over the glide |
|---|---|
| 1 | **66.9%** |
| 20 | 58.5% |
| 120 | 48.1% |

**Nobody exceeds 67%**, because holding the accelerator on for even 0.2 s ends
the run. That is the map: find the highest duty cycle that the surface tolerates.

## Two things nobody in 602 runs has

**A uniform 2-ticks-on, 1-off tap — 33 Hz, 66.7% duty — is worth 0.303 s over
the world record, with no search at all.** It is a metronome, and it beats every
human on the board.

**And the first 4.7 seconds are worth more than the whole glide.** Optimising
the drop-in is worth **0.835 s**; perfecting the entire 19-second glide is worth
**0.526 s**. Everybody optimises the part that looks like the map. The fast
tape drives the world record's own line into the drop — gas-and-brake scrub and
all — it just modulates the throttle through it, which no sampled human does.

## Tap *rate* orders nothing

Worth stating because it is the obvious thing to measure and it is a red
herring:

| | duty | presses per lap |
|---|---|---|
| rank 3 | 66.6% | **24** |
| rank 9 | 60.0% | **381** |

Sixteen times the input for less duty. **Check what actually orders the field
before naming a technique.**

## How a human does it: precision-bound, on a technique nobody is missing

The map's *name* tells you to tap, and all 30 sampled records drive the same
line. The author's validation lap is 313 ms better than his own online best
because he caught a better duty and phase — **not a different route.**

So the honest low-input axis here is **grain, not event count** — the tape *is*
a pulse train, and counting its events is meaningless. What matters is how
coarse the timing may be:

| input grain | best time | inside the AT? |
|---|---|---|
| 20 ms | 23.272 | no |
| 30 ms | 23.173 | no |
| **40 ms** | **23.125** | **yes** |
| 50 ms | 23.578 | no |
| 100 ms | **none of 40,180 uniform rhythms finish at all** | — |

**The author time still falls at a 40 ms grain**, which is inside what the field
already does with its hands.

## Validation

**45 files in one batch, all exact** — 30 downloaded human ghosts and all 15
tapes together, from rank 1 (23.638) to rank 470 (38.195), beside the 22.072.
Reproducible by anyone with the directory:

```
tmtas validate --map map.Map.Gbx --jobs 10 ghosts/*.Ghost.Gbx tapes/*.Ghost.Gbx
```

All 30 ghosts passed a completeness check before use (a truncated download
validates as `DNF cps=1` and reads as a genuine failure).

## Three findings for anyone doing this elsewhere

**A container's tail slack is neither zero nor unlimited — measure it.** Every
ghost here ends within 10 ms of its own finish, so a 174-pattern rhythm sweep
came back 0/174 and looked like physics. It was the harness: the simulation runs
on past the end of the tape, and rank 1's container tolerates **+2.1 s**. The
same map later produced a *genuine* all-DNF sweep, which is exactly why the first
kind has to be ruled out by measurement.

**Normalise tapes to the race-start timeline before splicing.**
`start_offset_ms` varies by **158 ticks** across one map's own ghosts.

**A constraint not read back off the artefact is not a constraint.** The first
grain-repair pass delivered "grain 3" tapes containing 2-tick runs. One printed
line caught it; those numbers were discarded.

## Files

| file | what |
|---|---|
| `replays/TAS_22072.Ghost.Gbx` | the fastest run |
| `replays/UNIFORM_2on1off_23335.Ghost.Gbx` | **the metronome — beats the world record with no search** |
| `replays/GRAIN40MS_23125.Ghost.Gbx` | inside the author time at a 40 ms input grain |
| `replays/DDMIN_747ev_23183.Ghost.Gbx` | 1-minimal at 747 events |
| `notes/RESULT.md` | the full write-up, negatives with their enumerations |
