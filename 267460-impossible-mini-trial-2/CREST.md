# 267460 `Impossible Mini Trial 2` — the wall is a CREST JUMP, and our own pit gives up 3.27 s

Arm `crest`, 2026-08-23, node `102237.od.fbinfra.net`, branch `crest`.
Times in **seconds**, speeds in **m/s** (the engine's own unit).

AT **16.888** · human WR **23.068** (Wirtual, the only record) · incumbent
**21.022**, unchanged.

---

## 0. The three headlines

**1. The mechanism behind *"the loop is there for the drop"* is a BALLISTIC JUMP,
and it has a number.**

> The pit ramp and the deck are one convex `PlatformDirtTiltTransition1UpLeft`
> block. A car at speed **leaves the surface at a crest at x ≈ 792** and is
> ballistic until it meets the deck top at **x ≈ 815, y = 115.48**. To land, its
> arc must be at **y ≥ 115.4 when it reaches x = 815**. The incumbent scores
> **115.43** and lands at (815.2, **115.478**) — **at its own apex, with no
> margin at all.**

That is a `--gate-key` expression, so it is searchable directly:

```
--gate-key 'py + vy*(815-px)/vx - 12.5*((815-px)/vx)*((815-px)/vx)'
```

**2. Launch speed is NOT the free variable on this map. The clock is.**
Terminal speed on the deck is **43.6 m/s**; the turbo pad is a fixed energy add
(`v_out² − v_in² = 1965 ± 90`, three independent tapes); the flight then needs
**≈58 m/s** at the deck edge or the car falls to y = 8. Every route that gets
onto the deck with a run-up arrives at the pad at ≈43.5 and leaves it at ≈62.5,
which is why the incumbent and the human — completely different pit routes —
launch within 0.5 m/s of each other. **The only thing a better pit can buy is
time.**

**3. It can buy 3.27 s, and this arm has a tape that does.** Not from the
U-turn: from **our own pit line, re-searched**.

| | crosses the turbo pad plane x = 846 | state there |
|---|---|---|
| `U855` (the banked U) | **never** — 0 crossings | — |
| `L1600` | 14.122 | (846.0, **114.021**, 712.3) vy **+0.035** — on the deck |
| **incumbent** | **14.794** | (846.0, 114.019, 705.1) vy +0.135, vx **47.30** |
| **this arm** | **11.520** | (846.0, **114.025**, 707.8) vy **−0.000**, vx **43.66** |

Wheels down, boosted by the pad (43.66 → 57.2), **3.274 s ahead of our published
lap.** It is not yet a lap: it lands on the grass at x = 994 and does not
convert (§6).

---

## 1. Controls

| control | result |
|---|---|
| plain oracle re-simulates the two references | human **23.068**, incumbent **21.022**, exact |
| `tmprog` empty program on the incumbent | **21.022** — the generator reproduces its template |
| **`ghost tape graft` the incumbent's head onto its own tail at its own pad tick, injected, re-simulated** | **21.022** — the graft pipeline is a no-op when it should be |
| every gate search | its decoy test printed and passed before the first candidate — and **one fired and stopped a run** (§6) |
| every trace quoted here | `fk trace`'s own self-check ok |
| the ballistic key against the world | the incumbent's crest state predicts **115.43**; it lands at **115.478**. **5 cm** |

## 2. A correction to the handover: `L1600` is a FALLER (but its gate reading stands)

`L1600.Ghost.Gbx` was banked as *"turbo gate at 14.059, 0.707 s inside our
incumbent"*. **The gate reading is real** — §0 shows it crossing x = 846 at
y = 114.02 with vy = +0.035, on the deck. **The tape is still not a route.**

`fk trace` (`cr_L1600_trace.csv`, banked, self-check ok, 1304 rows):

| race | where | what |
|---|---|---|
| 14.37 | (854.0, 114.0, 714.7) | past the launch box at 44.3, `vz` **+9.5** — drifting north |
| 15.3 – 17.3 | x 900 → 985, y 106 → 22 | falling the whole way |
| **17.560** | (994.8, **8.18**, 739.4) | **y = 8: the plane under the map** |
| 17.8 – 25.8 | y = 8.01 | it slides on that plane for eight seconds |

**And the locator lied first.** A bare `fk trace L1600` reported
`verr 0.0000 m/s, |q|-1 0.00e0, runner-up 0x worse`: it had latched a **zeroed**
slot, for which every residual is trivially zero. The self-check caught it
(`not a unit quaternion, p99.5 |q|-1 = 1.000`); `FK_ANCHOR="843,115,710,30"`
fixed it. **A residual of exactly zero is not a good fit, it is an empty
buffer.**

## 3. The endgame, priced — and it is FORCED

### 3a. The flight needs ≈58 m/s at the deck edge

Sixteen tapes: the incumbent with a brake pulse of 0–60 ticks from race 14.57.

| launch (`vx` at x ≈ 875) | outcome |
|---|---|
| 58.4 / 58.3 / 58.0 | **clears** — touches the grass slope at x ≈ 993, y ≈ 58 |
| 55.9 | falls to y = 8 at x = 977 |
| 45.7 / 44.9 / 41.0 | falls to y = 8 at x = 965 – 979 |

### 3b. The turbo pad is a fixed energy add, and BRAKING CANCELS IT

| tape | `v_in` (x ≈ 842) | `v_out` (x = 863) | `v_out² − v_in²` |
|---|---|---|---|
| incumbent | 43.36 | 62.75 | 2058 |
| human WR | 44.80 | 62.31 | 1876 |
| `L1600` | 21.73 | 49.34 | 1962 |

An 18-tick brake pulse ending 0.02 s before the pad took the gain from +19 m/s
to **+4**. Any sweep that slows pad entry by braking measures the wrong thing.

### 3c. Terminal speed on the deck is 43.6 m/s

The incumbent crosses the crest at 47.5 (off the descent), decays to **43.59**
by x = 819 and holds it to the pad. **44.5 is what the launch wants and 43.6 is
what the deck gives.**

### 3d. The ending has no tolerance

The incumbent with a 0–150-tick brake pulse from race 13.45: the zero-length one
does 21.022 and **every other one DNFs**, including a 3-tick (0.03 s) pulse,
which loses 26 m/s in one 0.15 s window at x ≈ 830 against the incumbent's 2.

## 4. The crest jump, and both of its mirrors

Airborne acceleration fitted from the incumbent's own apex: **24.8 m/s²**.
Bar **115.4** at x = 815. **Deck south edge: z = 704** — `mapgeom where` at
(817, 700) finds only `PlatformDirtBase` at y = 96, while at (817, 712) the
y = 112 platform is there. That single fact cost this arm two false positives
before it was bound into the objective.

| tape | crest state at x = 792 | arc height at 815 | lands? |
|---|---|---|---|
| **incumbent** | y 111.497, vy **+14.042**, speed 46.55 | **115.43** | **YES**, at (815.2, 115.478) |
| U + straight east | y 111.83, vy +8.01, speed 35.77 | 113.1 | no |
| best of 30 south-steered U variants | y 110.14, vy +10.92, speed 38.11 | 113.1 | no |
| best of 28 **slowed** U variants | — | — | **0 of 28 above y = 114 anywhere** |
| incumbent + 14 brake pulses | vy 9.4 – 13.9 | ≤ 114.6 | only the unbraked one |

**Both mirrors were tested and both failed.** More speed is not available on the
U's line, and *less* speed does not keep the car in contact.

> **So the descent is not there for the launch. It is there to buy the ≈46 m/s
> the crest jump needs — and 46 m/s is ABOVE the car's terminal speed on that
> surface. It can only be had by falling into it.** The U ends at the bottom of
> the descent: it has skipped precisely the thing that pays for the jump.

**And the U, searched hard, still cannot.** Seeded from `U855` with the flick
itself editable (fork resume race 3.94), on the arc-height key:
**2 040 000 evaluations, best 114.93, bar 115.4.** That is the honest null for
the U-as-published.

## 5. Where the time actually is: the pit

The human's own recording (`tmtraj export`; his telemetry is his own):

| race | where | what |
|---|---|---|
| 4.95 | (730.1, 118.7, 743.6) | **first contact**, 83.8 km/h (from 142.6) |
| 6.15 | (714.0, 108.7, 727.1) | bottom of the lower loop |
| **8.55** | **(724.0, 119.6, 746.0)** | **back within 3 m of his own landing point, 3.6 s later**, now pointing north-east and climbing |
| 10.35 | (715.2, 129.2, 762.7) | the hairpin at the top, 11.6 m/s — his slowest point |
| 13.67 | (776.9, 106.4, 720.1) | the crest, 48.5 |

**The lower loop is a 3.6 second U-turn that returns to the same point.** That
is what a flick is for — pointed **north-east up the spiral**, not east at the
deck.

Seeded from our own pit tape trimmed to race 11.5 (so any arrival is already a
2 s gain), scored on the arc-height key with the z corridor bound in:

| | crest | arc height | pad (x = 846) |
|---|---|---|---|
| incumbent | 13.525 | 115.43 | 14.794 at 47.30 |
| **searched pit** | **10.00** | **117.23** | **11.520 at 43.66** |

Traced end to end: it clears the crest, apexes at y = 117.05 at x = 817, lands
on the deck at x ≈ 832, reaches the pad, is boosted, and leaves the deck at
x = 890 at race **12.331** where the incumbent leaves at **15.556**.

## 6. The conversion, and exactly where it stops

The incumbent's lap from the pad to the flag is 21.022 − 14.794 = **6.23 s**,
and §3 says that ending is forced. So a tape at the pad at race T finishes at
**T + 6.23**: T = 11.52 → **17.75**, three and a quarter seconds inside our
published lap and 0.86 over the author time.

**Joining it is a `ghost tape graft`, and the graft does not finish yet.**
Three objectives, in the order they were needed, each one fixing what the
previous one could not see:

| objective at the departure plane x = 888..892 | seed → best | what it fixed |
|---|---|---|
| `-vdist(56.862,-11.618,0.209) - 2*abs(pz-705.05) - abs(py-111.296)` | −16.49 → **−0.27** | position and velocity: `vz` **−8.04 → −0.05**, and z now holds 705.02–705.18 across the whole 60 m drop |
| `... + 3*nose(...) + 3*roof(...)` | +5.38 → **+5.82** of 6 | almost nothing — **a cosine is flat near zero**: 11° of slip costs 0.06 of the key |
| `... - 0.35*abs(bodyright) - 0.35*abs(bodyup)` | −5.01 → **see §8** | attitude, linearly |

**What still fails, measured.** With position and velocity matched to
0.05 m/s, the grafted lap flies dead straight down the drop and **touches the
grass at x = 994** — but 1.85 m low by x = 971 and slipping ~11°, so it digs in
at **46 m/s where the incumbent skims at 73**, and falls off the slope. 46 grafts
across a 5 × 7 grid of handover ticks: **all DNF**.

**And one objective refused to start, correctly.** A gate on the finish line
itself (x 984..996, y 45..57, z 636..650), seeded from the graft:

```
decoy test: the do-nothing tape scores no gate, 38.22 m away; the incumbent
scores no gate, 54.99 m away -- THE DO-NOTHING TAPE WINS. Nothing was searched.
```

From the launch, **coasting drifts nearer the flag than the incumbent's own
inputs do**, because those inputs fly the car east to x = 1087 first. Closest
approach to the finish is a decoy on this route, and the tool said so before
spending an hour.

## 7. Tooling

| commit | what |
|---|---|
| `300ac40` | **`tmtraj route` read only the FIRST file it was given.** Its usage has said `CSV\|GHOST...` since it was written and only `--margin` honoured it; `--summary`, `--cross`, `--where` and `--near` all read `positional[0]` and silently dropped the rest, so a 30-tape family sweep printed one row and read as the family's answer. It loops now, and exit 1 means "every file was empty" rather than "the first one was". |

Nothing else needed adding, and that is the point worth recording: **the
ballistic-arc criterion, the z corridor and the attitude match are all
`--gate-key` expressions**, and the pit-to-ending join is `ghost tape graft` +
`ghost tape inject`, both already there. `--seg` was not fixed and, on this
map, was not the blocker: the gradient came from the fork state objective, and
the state-match makes the ending a graft rather than a search.

## 8. The state match, taken to the end — and the number the whole thing reduces to

The attitude objective was refined twice more and the second refinement is the
one worth keeping as a lesson.

**A cosine is the wrong shape for a small angle.** `nose()` and `roof()` return
cosines, so 11° of slip cost **0.06** of a key whose scale was 6 — the term was
in the objective and could not see the defect. Replaced with `abs(bodyright)`
and `abs(bodyup − B)`, which are linear in the angle at fixed speed, the search
took the slip from **11.3 m/s to 0.002 m/s** in half an hour.

**And `B` is not zero, which is the second lesson.** I first targeted
`bodyup = 0` — "the nose along the velocity" — and the incumbent's own value at
that plane is **+3.879**: its nose sits 3.9° BELOW its velocity vector all the
way down the drop. Targeting zero pinned the car 4° nose-up, which is what put
it 1.85 m low by x = 971 and made it dig into the grass at 46 m/s where the
incumbent skims at 73. **A state match against "the obvious value" is a match
against your own assumption; measure the reference's actual number.**

### The end state, on two handover planes

| handover plane | best key | measured state | vs the incumbent |
|---|---|---|---|
| **departure, x = 890** (airborne, 6 DOF) | −0.053 of a max +2 | (888.906, 111.343, **705.050**) v (56.822, −11.648, **0.224**) body(right, up) (**0.002**, 0.000) | position, velocity and slip all matched |
| **pad entry, x = 844** (on the ground) | −0.401 of a max +2 | (843.998, **114.011**, **705.258**) v (**41.029**, 0.102, −2.454) body(right, up) (**−0.001**, **−0.009**) | z exact, attitude exact, **speed 2.35 m/s short** |

**105 grafts across both planes and every plausible handover tick: all DNF.**

> **So the whole thing reduces to one number: 2.35 m/s at the turbo pad.**
> The pad turns that into 3.3 m/s at the launch (`v_out² − v_in² = 1965`), and
> §3a says 3.3 m/s at the launch is the difference between touching the grass
> at x = 993 and falling to y = 8 at x = 977.

**Where the 2.35 m/s went, measured.** The searched pit apexes at **y = 117.05,
1.6 m higher than the deck needs**, so it lands at **x = 832** instead of the
incumbent's 815 and has **12 m of deck** to the pad instead of 30. Terminal on
the deck is 43.6 and it arrives with 41.0. **The arc-height objective rewarded
overshoot, and overshoot is paid for in runway.** The corrected objective —
`min(vx, 100*(arc − 115.45))`, which buys height only up to the bar and then
maximises speed — is the last thing this arm ran; see §9.

## 9. What the next arm should do first

1. **Finish the corrected crest objective.** `min(vx, 100*(arc − 115.45))` in
   the crest box `x 786..798, y 104..114, z 710..722`, seeded from
   `cr_CREST_fast_vx4033.Ghost.Gbx`. The incumbent's crest `vx` is **43.27**;
   this arm took it **36.98 → 40.33** in 3 523 440 evaluations and it was still
   climbing when the lease ran out. Every m/s there is an m/s at the pad, and
   2.35 of them is the whole remaining gap. **The corrected key is worth
   +3.35 m/s over the plain arc-height key already** — that is the measured
   value of not rewarding overshoot.
2. **Then re-chain**, which is three commands and no search: crest tape →
   pad-entry state match (`0 - vdist(43.372,0.035,-1.982) - abs(bodyright) -
   abs(bodyup) + 2*roof(0.005,1,0.0001) - 2*abs(pz-705.258)` in
   `x 840..844, y 113..116, z 702..710`) → `ghost tape graft --head <it> --tail
   imt2_TAS_21022_v1 --at <its x=842 tick> --from 1625` → `ghost tape inject` →
   plain oracle.
3. **Do not spend time on `--seg`.** It was not the blocker here. The gradient
   is the fork state objective, and the ending is a graft, not a search.
4. **Do not seed from the U.** 2 040 000 evaluations with the flick itself
   editable stop at 114.93 against a bar of 115.4. The U's own line cannot make
   the crest jump; our pit line can, and does, 3.5 s early.

## 10. Artefacts

Banked to `~/persistent/private-30d/tm-unbeaten/267460/cr_20260823/`, with
`cr_MANIFEST_v1.md5`.

| file | what |
|---|---|
| `cr_CREST_1000.Ghost.Gbx` | the searched pit: the crest at race **10.00**, arc height 117.23 |
| `cr_CREST_fast_vx4033.Ghost.Gbx` + `.state.json` | the corrected crest objective: `vx` **40.33** at the crest (bar 43.27) |
| `cr_PADENTRY_matched.Ghost.Gbx` + `.state.json` | the pad-entry state match: z and attitude exact, **41.03 m/s against 43.37** |
| `cr_PAD_1154_at_4365.Ghost.Gbx` + `.state.json` | on the deck at the pad, **race 11.520, vy −0.000, 43.66 m/s** |
| `cr_DEPARTURE_matched.Ghost.Gbx` + `.state.json` | the x = 890 state match, slip 0.002 m/s |
| `cr_GRAFT_pad1309_inc1634.Ghost.Gbx`, `cr_GRAFT_departure.Ghost.Gbx` | grafted laps; both DNF, both traced |
| `cr_inc_trace.csv`, `cr_L1600_trace.csv`, `cr_U_straighteast_trace.csv`, `cr_human_trace.csv`, `cr_human_recorded.csv`, `cr_GRAFT_*_trace.csv` | every trace quoted above |
| `pit1.png`, `fly1.png`, `crest_top.png`, `deck_s2.png`, `ramp1.png`, `route_pit.png` | the pit, the flight, the crest and the deck's south edge |

**None of these is a lap. The incumbent is still 21.022.**
