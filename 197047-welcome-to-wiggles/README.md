# Welcome to wiggles — author time beaten by 4.945 s, and beaten on two keys

| | time | vs AT | vs human WR | alphabet |
|---|---|---|---|---|
| **TAS** | **95.839** | **−4.945** | **−5.955** | pad |
| **keyboard, two keys** | **96.412** | **−4.372** | −5.382 | **`{−127,+127}`** |
| keyboard, metronome | 96.759 | −4.025 | −5.035 | `{−127,0,+127}` |
| earlier validated tape | 96.852 | −3.932 | −4.942 | pad |
| Author time (never beaten by a human) | 100.784 | — | −1.010 | — |
| Human WR | 101.794 | +1.010 | — | 229 values, pad |

TMX map [197047](https://trackmania.exchange/maps/197047) · tags **Endurance,
Race, Educational** · 21 recorded runs · **100 seconds**, the longest map in
this collection.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## What the map is

Not an endurance course. It is 100 seconds of **the wiggle**: gas and brake held
*together*, steering flipped full-left/full-right, creeping a car that cannot
otherwise move along 620 m of flat straight at 242 m altitude, at 22 km/h. Two
checkpoints — one 6 m from the spawn, one at the far end — and nothing in
between to cue off. "Educational" means it teaches exactly one thing.

So the objective is not a trick at a feature. It is **the mean speed of a
periodic limit cycle**, which is why the margin is large: a technique found once
applies 400 times over.

## The finding: the field wiggles about 15% too fast

The world record's median half-cycle is **21 ticks**. The fastest limit cycle is
**25 ticks — hold each side 250 ms.** That is the whole map.

And the field is not even consistent: only **53% of the world record's own flips
land within one tick of its own median**. So the advice is two words — *slower,
metronomic* — and it is worth about 4 seconds.

**Amplitude is physically irrelevant.** 70, 90, 110 and 127 of full lock over
the same rhythm land within 1 ms of each other. That is why the deliverable here
is a **two-key tape**: if strength does not matter, a keyboard is not a handicap.
This was checked against the humans rather than assumed — both keyboard runs on
the leaderboard already flip straight from −127 to +127 without passing through
zero on ~90% of flips.

## The other 1.5 seconds: the run ends with a respawn

The far gate is not the end. **Respawn is an editable input** — bit 31 of the
packet's 34-bit state literal, not part of the steer/accel/brake triple, which
is why `ghost::Factory` could not see it and why every candidate came back with
an identical time for an hour.

```
finish = (first respawn tick − 154)·10 + 1504 ms    (exact)
```

Pressing respawn on the **first tick after the far gate** is worth ~75 ms
against the world record, who waits.

## The driving guide

The map has no features to cue off — that is the point of it — so the cues are
the countdown, the gate, and a count in your head.

1. **Start → checkpoint gate (0 → ~0.8 s).** Full gas, no brake, straight down
   the platform. The gate is 6 m away.
2. **The run-up (0.8 → ~1.9 s).** Full gas. The car reaches ~100 km/h and then
   the surface takes it away — you feel it stop pulling about a car-length past
   the gate structure.
3. **Enter the wiggle (~1.95 s).** **Add the brake and keep the gas.** Both held
   for the next ninety-eight seconds. Never release either — one tick of release
   kills the run.
4. **The wiggle (2 s → ~94 s).** Alternate full left and full right. **Hold each
   side a quarter of a second — 250 ms — not the fifth of a second the world
   record uses.** Two flips per second: a 120 bpm metronome, a flip on every beat
   and every off-beat. Steer strength does not matter, so use whichever input you
   can time better.
5. **Hold the line.** The corridor is a few metres wide and the car creeps
   sideways. Correct with the *length* of a half-cycle, not with a partial steer:
   lengthen the side you want to come back from by 10–20 ms, then straight back
   to the rhythm.
6. **The far gate (~94 s).** The instant you touch it — not after you read the
   split — **press respawn.** You are teleported to the start line and the clock
   from there is fixed at ~1.5 s. Every millisecond you wait is a millisecond on
   your time.

## How forgiving is it

Measured over all 431 flips, 2,587 oracle runs:

- **53% of flips take ±30 ms of mistiming for free**, and a mistiming that does
  not kill the run costs between −5 and 0 ms.
- **Sensitivity decays with distance remaining.** The first 35 seconds is where
  errors are expensive; **after the first minute, no single-flip shift can lose
  the run.**

Verdict: **known but mistimed.** Nobody needs to discover anything here. The
rhythm is the easy part; holding it for 92 seconds without a drift that walks you
off the edge is the hard part — which is exactly what this leaderboard is already
struggling with, and why it has 21 entries rather than 900.

## Two things that generalise

**A homogeneous map has no decisive sector.** Using 20 relocated finish gates,
every one of 19 sectors correlates 0.44–0.80 with the final time. There is no
feature to attack — and the field's assembled per-sector minima already beat the
author time by ~2 s, which is what told us the margin was real before any search
found it.

**A gate ladder neutralises the real checkpoint.** The best marched tape reached
the far end 509 ms ahead of the winner and then returned `DNF cps=1` on the real
map at every respawn tick: it had been optimised into a line that misses the real
checkpoint's trigger volume, and repairing it cost more than it had gained.
Marched candidates are hypotheses until the plain oracle validates them on the
untouched map.

## Validation

Every tape re-validated through the plain oracle against the untouched map, with
the human world record carried as a known-answer control in the same batch
(returns 101.794 exactly).

## Files

| file | what |
|---|---|
| `replays/TAS_95839_analog.Ghost.Gbx` | the fastest run |
| `replays/KEYBOARD_96412_twokey.Ghost.Gbx` | **two keys, `{−127,+127}`** — the one to practise |
| `replays/KEYBOARD_96759_metronome.Ghost.Gbx` | three-value metronome variant |
| `replays/TAS_96852_v1.Ghost.Gbx` | the first tape under the author time |
| `inputs/*.tick.txt` | tick scripts for each |
| `notes/RESULT.md` | the full write-up: per-sector table, tolerance study, tooling |
| `notes/PLAN.md` | the pre-search analysis |
