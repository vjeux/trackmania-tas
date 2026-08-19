# 252289 — `surely my least cooked at` — the author time, beaten

**Map** `eetemRii0Hscd6vEudBsy4mbMK3` (TMX 252289), by **in-.-**, uploaded
2025-06-23. Stadium, tag *Mini*, one checkpoint (the finish).
**Author time 3851 ms. Best human ever 3867 ms** (KevinMagPizza, 2026-06-13),
over 706 recorded runs. The AT had never been beaten.

| tape | validated | vs AT 3851 | vs human WR 3867 | input change events | steer alphabet |
|---|---|---|---|---|---|
| human WR (KevinMagPizza) | **3867** | +16 | — | 8 | `{-127, 0, +127}` — **keyboard** |
| `tas_twoinputs_3848` | **3848** | **−3** | −19 | 12 (+4 on the WR) | keyboard |
| `tas_keyboard_3844` | **3844** | **−7** | −23 | 18 | keyboard |
| `tas_3836` | **3836** | **−15** | **−31** | 18 | 8 values |

All four re-validate through the plain oracle to the millisecond, in cold
processes, with the human WR carried in every batch as a known-answer control
(`revalidate.sh` in this directory regenerates the transcript).

The headline for a driver is not the 3836. It is this:

> **Take KevinMagPizza's world-record run and add two keyboard actions in the
> last corner — a single-tick right tap at 2.63 s and a 50 ms throttle lift at
> 2.89 s — and it finishes in 3848 ms. That beats the author time by 3 ms with
> nothing a human cannot do.**

---

## 1. What this map actually is

Four blocks. The start and the finish are **side by side**: `RoadTechStart` in
cell (16, 9, 22), `RoadTechFinish` in cell (15, 9, 22), with two
`RoadTechStraight` run-off blocks behind them in the next cell row. Cells are
32 m, so the two lanes are 32 m apart and the seam between them is at
**x = 512**.

The car spawns at (528, 720) at rest, **facing +z**. The finish is a plane at

> **z = 731.01 m**, crossed travelling in **+z**

— measured, not assumed: take the last telemetry sample of each of the 14 clean
leaderboard ghosts, extrapolate it by its own velocity to its own finish time,
and all fourteen land on z = 731.01 ± 0.03 m.

So the whole task is: from a standing start facing +z, get **16 m sideways**
into the neighbouring lane and be **11 m further up the track**, moving in +z,
as fast as possible. Straight-line that is 19 m, and the car is capable of
28 m/s — yet the record is 3.87 s. The map is not a distance problem, it is a
**rotation** problem, and that is why nobody drives it forwards.

A single forward arc cannot work, and the geometry says why. Curving away from
the start, the car's z gain and its x travel are the same number: to reach
x < 512 you need a radius of at least 16 m, and with a 16 m radius you have
already crossed z = 731 at x ≈ 524, still in the start lane, where the gate does
not exist. You have to spend the x first and the z second.

## 2. What the human record does

Eight input events, pure keyboard:

```
0.00   brake ON  (reverse), steer neutral
0.12   steer LEFT
0.72   brake OFF
0.79   steer neutral
0.82   steer RIGHT
0.84   throttle ON
2.44   steer neutral
2.68   steer LEFT   (held to the line)
```

- **0.00–0.72 s** — reverse out of the start under full left lock. This is not
  wasted time, it is the only way to rotate: the car ends the phase 4.6 m back
  down the lane and turned 37°.
- **0.72–1.30 s** — flip to throttle and full right; the car scrubs off the
  reverse speed and comes to a dead stop at 1.30 s, now pointing across the
  track.
- **1.30–2.44 s** — full throttle, full right lock, rotating to face −x
  (across the lanes) and reaching 55 km/h.
- **2.44–2.68 s** — straight, 55 → 69 km/h, crossing the lanes.
- **2.68 s – finish** — full left lock, full throttle, one long 100° sweep that
  turns the car back to +z and crosses the line at 101 km/h.

## 3. Where the 31 ms is: all of it is in the last corner

Per-tick trajectories of both runs (`fk btraj`, exact — the fork reports the
same 3836/3867 the plain validator does) side by side:

| t (s) | TAS speed | WR speed | Δ |
|---|---|---|---|
| 2.60 | 65.4 | 64.5 | +0.9 |
| 2.70 | 65.9 | 68.0 | **−2.1** |
| 2.90 | 72.3 | 73.3 | −1.0 |
| 3.00 | 73.1 | 73.7 | −0.6 |
| 3.10 | 70.9 | 67.5 | **+3.4** |
| 3.20 | 72.5 | 67.5 | +5.0 |
| 3.50 | 88.4 | 83.0 | +5.4 |
| 3.80 | 101.7 | 98.5 | +3.2 |

Up to 2.6 s the two runs are the same run — 8 cm and 0.9 km/h apart after
2.6 seconds of driving. Then:

- The TAS **gives up 2 km/h turning in** to the final corner.
- Between 3.02 s and 3.12 s **the human's car loses 6.2 km/h** — 73.7 down to
  67.5 — and never gets it back. The TAS loses 2.2 over the same stretch.
- From 3.1 s to the line the TAS is 5 km/h faster, every tick.
- At the line the TAS is **1.11 m further down the track**. At 28 m/s that is
  the 31 ms.

Textbook slow-in / fast-out, on a corner nobody thought of as a corner.

**What is the human losing it to?** The last corner is taken at full lock and
full throttle from 2.68 s, and around 3.0–3.1 s the car crosses the seam
between the two road blocks (x = 512, at 2.97 s) with the suspension already
loaded by the turn. Both runs dip to a body height of 9.95 m at 3.02 s and
rebound. The human's car takes the rebound at 73.7 km/h and the tyres let go —
slip angle stays under 3° in both runs, so this is not a drift, it is the
contact patch being unloaded and reloaded while the car is asking for maximum
lateral grip. The TAS arrives ~1 km/h slower with the suspension a fraction of
a cycle further along, rides the rebound, and keeps its grip.

## 4. The driver's guide

Everything below is measured against the world-record run as a base. Times are
race clock; the game runs at 100 Hz, so "1 tick" = 10 ms and 5 ticks = 50 ms.

### The minimum change that beats the author time — two actions, 3848 ms

Drive KevinMagPizza's run exactly, and add:

1. **At 2.63 s — one-tick tap of RIGHT.** You are in the neutral-steer phase
   between the rotation and the last corner, doing **66 km/h**, pointed across
   the track. Tap and release. (Yes, a right tap 50 ms before you turn left.
   It sets the car's attitude for the corner.)
2. **At 2.89 s — lift the throttle for 50 ms**, i.e. release and re-press over
   five ticks, then full throttle again at 2.94 s. You are at **73 km/h**, at
   full left lock, and the seam between the two lanes is just about to pass
   under the car (you cross it at 2.97 s).

That is 3848 ms, validated. Three milliseconds under the author time.

**The lift is the trick.** On its own, added to the WR with nothing else
changed, it is worth 9 ms (3867 → 3858). The right tap on its own is
catastrophic (4189) — it only makes sense once the lift follows it. They are a
pair.

### The full keyboard run — 3844 ms

Nine ticks differ from the world record, all of them between 2.63 s and 2.98 s,
all of them releases:

| time | action | note |
|---|---|---|
| 2.63 s | tap RIGHT for 1 tick | attitude set-up |
| 2.72 s | release LEFT for 1 tick | feather |
| 2.74 s | release LEFT for 1 tick | feather |
| 2.89–2.93 s | **throttle off for 5 ticks** | the money |
| 2.98 s | release LEFT for 1 tick | feather |

Nothing before 2.63 s changes at all. The reverse, the stop, the rotation and
the cross-track run are exactly the human's. The two extra feathers are worth
2 ms each on top of the two-action version.

### The analog run — 3836 ms

Twenty-one ticks differ from the world record, in four clusters, and it needs
steering values a keyboard cannot produce (`+3`, `−112`, `+80`, `+1`, `−80`):

| ticks | time | WR | TAS |
|---|---|---|---|
| 0–11 | 0.00–0.11 s | steer 0 | steer **+3** (2% right, while reversing) |
| 29 | 0.29 s | −127 | **−112** |
| 158–162 | 1.58–1.62 s | +127 | **+80** (ease the lock for 50 ms mid-rotation) |
| 273, 276, 277 | 2.73, 2.76, 2.77 s | −127 | **−112, +1, −80** |

This is the TAS-only tape. A driver cannot hold 2% of steering lock for 110 ms,
and the value of the intermediate steering positions is exactly the structural
edge a TAS has over a keyboard: 255 steering positions instead of 3.

## 5. Can a human practise this? Yes — but learn the LINE, not the TAPE

Two different questions, and they have opposite answers.

**Mistime one input and change nothing else** and the run detonates. Every
boundary in the 3836 tape, shifted by a single tick:

```
event      +1 tick    -1 tick
0.12 s      +527 ms    +318 ms
0.72 s      +548 ms      DNF
1.58 s      +543 ms    +283 ms
2.44 s      +365 ms    +480 ms
2.68 s      +341 ms    +335 ms
```

That is not a driving property, it is a property of an **open-loop tape** in a
chaotic simulator: a change at t invalidates every input after it, and the tape
has no way to notice.

**The control that settles it: the human world-record tape is exactly as
fragile.** Shift one of KevinMagPizza's own eight inputs by a single tick and
his 3867 becomes:

```
0.12 s  +276 / +1251 ms      0.84 s  +779 / +2871 ms
0.72 s  +305 /  +382 ms      2.44 s  +313 /  +451 ms
0.79 s  +298 /  +387 ms      2.68 s  +281 /  +497 ms
```

A person drove that run, 706 people chased it, and it is no more tick-tolerant
than ours. So per-tick fragility of a frozen tape is not evidence about human
executability — it is evidence about replaying open-loop input on this map.

**Mistime one input and then drive the rest of the corner** and it costs
nothing. The same 40 mistimings, each followed by re-optimising only the inputs
*after* the error:

```
40 of 40 mistimings recovered to 3836 ms (one to 3839).
```

Every single one. Shifting the very first steering input two ticks early turns
a DNF into 3836 ms once the rest of the run is re-driven. A human is
closed-loop — they are re-aiming every frame — so the correct reading is:

> **The line is robust. The tape is not.** Learn the shape: reverse-rotate,
> stop, rotate under power, cross the lanes, and then *breathe* in the last
> corner instead of holding it flat.

The one input that genuinely needs precision is the throttle lift, because its
value comes from where the car is on the seam when the load comes back. Held
against an otherwise unchanged WR tape, starting the lift one tick late costs
34 ms and one tick early costs 242; making it longer than 50 ms costs about
100–130 ms, making it shorter costs more. So the lift wants to start at 2.89 s
±1 tick and last at least 50 ms. That is a tap at a speed you can read off the
dashboard (73 km/h) with a visual cue under the wheels (the seam), and taps at
a cue are learnable — this is the same class of input as a brake tap into a
Kacky landing.

## 6. Is the author time legitimate?

Yes, on the evidence. 3851 sits **7 ms above a run that uses only the three
steering positions a keyboard can produce** and only nineteen more milliseconds
of input events than the existing world record — and 3 ms above a run that is
the current world record plus two key presses. A driven validation lap at 3851
is entirely consistent with what the physics allows on keyboard. The map's
metadata does carry `atSetByPlugin: true` and the author's own leaderboard time
is 3932, so the AT was not set by the run that stands on the board — but
nothing here suggests the number is out of reach. It is a hard, honest AT, and
the 706 people who chased it were 16 ms away from a throttle tap.

## 7. Evidence and method

- **Oracle**: `TrackmaniaServer /nodaemon /validatepath=.`, GameVersion 3.3.0.
  Ground truth established before anything else: all **15** downloaded
  leaderboard ghosts re-simulate to their exact recorded times, and the WR tape
  rebuilt through our own encoder returns 3867.
- **The fork server was deliberately not used.** Measured on this map, one full
  re-simulation costs **8.9 ms** (`wall = 2.2 s + 8.86 ms × candidates`, fitted
  at N = 1/1000/4000); the fork server's floor is ~11.5 ms. On a 3.9 s map the
  fork is *slower* than starting over. Everything here is the classic path with
  the plain validator as the score, so none of the fork-resume defect classes
  can apply to it by construction.
- **Search**: `tmex`, written for this map — the tape is 387 ticks, so a *full*
  single-tick sweep of every tick × every one of 255 steering values is 98,298
  candidates and **16 seconds** of box time at ~6,000 candidates/s on 168
  workers. At that price you enumerate the neighbourhood instead of sampling
  it. Rounds: exhaustive single-tick; block-constant at every offset and length
  with full value resolution; accel/brake blocks; linear ramps; quadratic
  profiles; segment-value, boundary-shift, split, joint pairs, joint triples,
  joint two-tick; periodic notch combs; and a randomised round with the
  compensated operator mix, all under iterated local search with kicks.
- **Both banked tapes are 1-move optimal at millisecond resolution.** A full
  closing cycle from the 3836 tape — accel/brake, segment values, boundary
  shifts, exhaustive single-tick over all 255 values, block-constant at every
  offset/length/value, ramps, quadratic profiles, splits, joint pairs, joint
  triples, joint two-tick, and a 300k randomised round: **5,918,450 candidates,
  zero improving**. The keyboard line's closing cycle: **6,276,512 candidates,
  zero improving** after 3844. Because the score *is* the plain validator, that
  statement is immune to every evaluator defect by construction. About 21.6 M
  candidates were simulated in total across the session.
- Where the improvements came from, in order: `blk` 3862→3853, `fine`
  3853→3838, `triple` 3838→3837, `ptick` 3837→3836. The structured
  segment-level rounds alone stall at 3862; the free-form block and
  accel/brake rounds are what broke it open. **Everything the search found is
  early or mid-tape or in the last corner — the "improvements land late in the
  tape" rule from the long map did not hold here.**

### One incident, reported

Two keyboard tapes (`best_3844`, `best_3846`) failed re-validation: they were
banked as 3844/3846 and re-simulated as **4006/3848**. Cause, found and fixed:
the alphabet restriction quantised each *candidate* to `{-127, 0, +127}` inside
the evaluator, but the search stored the **un-quantised** state as its
incumbent, so the tape written to disk was not the tape that had been scored.
The evaluated trajectory was always legal, so the search's own scores were
sound; only the artefact was wrong. Specimens preserved in
`tm-loop/phantoms/252289-alpha-quantise-2026-08-18/`. Fixed by quantising at
every point where a state becomes the incumbent, and re-running the keyboard
line from scratch — the rebuilt 3844 and 3846 re-validate exactly. This is a
**fourth** phantom class, independent of the three fork/shared-root defects
circulating in the fleet, and it was caught by nothing but the standing rule to
re-validate every banked tape through the plain oracle.

## 8. Files

| file | what |
|---|---|
| `tas_3836.Ghost.Gbx` | the fastest run, 3836 ms |
| `tas_keyboard_3844.Ghost.Gbx` | keyboard-only, 3844 ms |
| `tas_twoinputs_3848.Ghost.Gbx` | the WR plus two key actions, 3848 ms |
| `human_WR_3867_KevinMagPizza.Ghost.Gbx` | the control, 3867 ms |
| `map_eetemRii0Hscd6vEudBsy4mbMK3.Map.Gbx` | the map |
| `*.tick.txt` | the same tapes as TICK input scripts (`tmsite verify` = EXACT MATCH) |
| `revalidate.sh` | regenerates the validation transcript, control included |
| `validation.txt` | the transcript |
| `tol_3836.txt`, `tolre_3836.txt` | frozen and repaired tick tolerance |

The declared race time inside each `.Ghost.Gbx` has been rewritten to match its
simulated time, so a viewer does not show 3867 over a 3836 run; the input
bitstream is untouched (the server's `Inputs` RLE is identical before and
after) and every file is re-validated after the rewrite.

**Not submitted to any Nadeo leaderboard**, per the project rule.
