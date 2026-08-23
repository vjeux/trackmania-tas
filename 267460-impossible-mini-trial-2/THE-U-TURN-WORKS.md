# 267460 `Impossible Mini Trial 2` — the icy dirt flick: what the technique is, and what the U-shape is worth

Arm `iceflick`, 2026-08-23, node `106734.od.fbinfra.net`, branch `iceflick`.
Times in **seconds**, speeds in **m/s** where the engine reports them and km/h
where a page or a player would.

AT **16.888** · human WR **23.068** (Wirtual, the only record) · incumbent
**21.022**, unchanged.

**THE U-TURN WORKS.** A searched line flicks the car through 180° in **0.600 s**
on the first dirt platform and is heading east at **28.34 m/s** by race **6.990**,
where the human’s two-loop figure-eight has him at **+1.82 m/s** and does not
reach **+16.93** until **8.000**. The car is on the ground throughout
(`v_y` = −0.06). **It does not yet convert to a lap**, and §4b prices exactly
why: the launch wants the car high and fast, and the U delivers it east and low.
The incumbent stands at **21.022**.

Read `CLAIMS.md` §3 *"A relocated gate measures a PLANE CROSSING"* first — this
arm wrote it, and it is the reason two of this page's earlier numbers are
retracted below rather than reported.

---

## 1. The technique, and where the name lives

**MEASURED — the wiki does not have it.** `wiki.trackmania.io` has exactly the
pages you would look for and they are **stubs**: `Gameplay/SpecialTricks`
(page 33) is one heading and an empty `<figure>`; `Gameplay/surfaces/ice`
(page 160) has a **`Tricks` heading with nothing under it**. Control: the same
fetch returns real content from the same pages (`surfaces/ice` does return its
Materials list — `CustomIce`, `PlatformTechPlatformIce`, `RoadIce`), so the
fetch works and the trick sections are genuinely empty. Anonymous GraphQL
content reads are refused (`PageViewForbidden 6013`), so the page list is
authoritative.

**MEASURED — reddit and YouTube transcripts are unreachable from this node, and
that is a harness limit, not an absence.** reddit returns HTTP 403 *"You've been
blocked by network security"* through `fwdproxy`; YouTube's innertube player
returns `LOGIN_REQUIRED / "Sign in to confirm you're not a bot"` and the watch
HTML carries no `captionTracks`. The videos exist and have transcripts; this box
cannot fetch them. **The vision route was unavailable for the same reason and
one more: this node has no `ffmpeg` and no `yt-dlp`.**

**MEASURED — the name is real, from YouTube search metadata, which does load.**

| what | where |
|---|---|
| **"Scandinavian Flick \| Full-Speed Ice \| Trackmania 2020"** | video `1ZHGH1Fv28s` — the rally term, used as a title, for TM2020 ice |
| **"TrackMania COMPLETE Ice Tutorial. EVERY Trick in the Game!"**, BirdieTM, `GMoW80FWjhc` | its own `chapterRenderer` records: Ice Wallrides 1:32 · Bobsleighs 2:29 · Ice Slides 3:20 · Ice Wiggle 6:43 · **360/270 Degree Turn 7:22** · How Far It Can Go? 8:03 |

**"360/270 Degree Turn" is the community's name for rotating the car through
most of a turn on ice. "Scandinavian flick" is the name for how you start that
rotation.** Those two chapter/title strings are the whole documentary basis; I
could not read the bodies.

### The input recipe, tagged INFERRED

**INFERRED — a Scandinavian flick is: steer AWAY from the intended turn, then
reverse INTO it with the throttle lifted, so the car's own yaw inertia carries
it round further than the available grip can steer it.** The argument is that
the rally technique of that name is exactly that, the community uses that name
for TM2020 ice, and a 180° turn is only available on this landing because the
wheels are **43–55 % iced** (measured by an earlier arm, `imt2_icewheels_v1.tsv`)
— low grip is the *precondition* for a pendulum turn and the *reason* a
steady-state turn there is wide.

As a program: **phase A** full steer away, gas on · **phase B** full steer into
the turn, gas off (optionally brake) · **phase C** counter-steer to catch it.
Free variables: when A starts, and the three durations. The surface state it
needs is iced wheels, which on this map is a **clock** — ice saturates by 2.000
and decays to 0 by 6.500 at ≈0.2/s, and the landing is at 4.800.

**This recipe was implemented and enumerated, and §3 is what it did.**

---

## 2. What the pit actually is — settled by a picture, not by a measurement

Four separate measurements had left me guessing which platform is "first" and
which is "second". `tools/rend`, contributed by the video arm the same night,
settled it in one look (`pit_time.png`, banked; red 4.0–6.0, orange 6.0–8.0,
yellow 8.0–10.0, green 10.0–11.5, blue 11.5–14.0):

* the car falls in from the east and crosses the first dirt platform (**red**);
* **orange is the lower loop** and **yellow+green is the upper loop** — the
  human drives **both**, a figure-eight, 4.2 s of it;
* **blue is the run-up**, the 24 m descent that converts 41 → 175 km/h and
  feeds the turbo gate.

**Our own 21.022 incumbent already drives only ONE of the two loops** — the
upper one — which nothing had written down. Both then leave east down the same
ramp.

So "the loop around the first dirt platform" is real and there are two of them,
and "the second platform" is the run-up ramp that carries the car east to the
turbo gate at (846, 114, 720).

The human's pit, from his own telemetry (his ghost carries his own recording;
`imt2_TAS_21022_v1` and `imt2_SLIDE_arrival_7283_v1` do **not** — they are search
tapes on his container, and anything read out of their telemetry is his run):

| race | where | speed | what |
|---|---|---|---|
| 4.800 | (732.8, 119.8, 746.1) | 97.5 | first contact — 45.4 km/h scrubbed in one 50 ms sample |
| 6.200 | (713.1, 108.5, 726.9) | 64.9 | bottom of the lower loop |
| **6.950** | ≈(705.2, 113.2, 735.0) | — | **his eastward velocity crosses zero: the U is complete here** |
| **8.000** | (715.7, 116.9, 741.4) | — | **peak eastward speed +16.93 m/s** |
| 10.400 | (714.6, 129.1, 762.6) | 41.0 | top of the climb; the run-up starts |
| 15.550 | (853.6, 114.0, 710.2) | **57.11 m/s** | through the launch gate |

---

## 3. Enumerated flick programs: 45 504 of them, and none converts

### 45 504 explicit flick programs, and none of them converts

`tmprog` (ported to the current API this arm — see §6) enumerates exactly the
program shape the technique has: constant `(steer, gas, brake)` phases with free
durations, starting from race 4.300, over the full cross-product.

| family | what it was | scored on | result |
|---|---|---|---|
| **f1**, 14 400 | 3 phases, both steer signs in the flick phase, constant tail | pit stations | **0 real arrivals** — see the retraction below |
| **f4**, 31 104 | 3 phases × 6 rejoin advances × 2 reference tails | the **turbo-gate ruler** | **0 of 31 104 reach the launch** |

**The positive control the negative needs, in the same batch and the same code
path:** on `rung/r8600_0` — a station just past the landing — **2593 of 2593 of
the same family fire it**, and 2013 of 2593 fire `r5400_0`. **The family drives.
It gets through the landing. It then never gets back to the launch.** On the
real map, 1 of 2593 finishes, and that one is the control.

And the generator itself has a two-sided control: an empty program with
`--tail rejoin:0` re-simulates to **21.022 exactly**, and the same empty program
with `--tail rejoin:20` returns **DNF**.

> **What this licenses: within a family of ≤3 constant phases over 0.9 s,
> rejoining a reference line at one of six fixed advances, no flick on this
> landing reaches the launch.** It does **not** license "the U-turn is
> impossible" — a real pendulum turn is continuous steering, not three
> constants, and the rejoin is a crude reattachment. §4a is the searched
> version of the same question, and it SUCCEEDS — which is itself the result:
> **the U is a continuous-steering manoeuvre and an enumeration of constant
> phases cannot express it.**

### RETRACTED: "three programs beat both references by 1.610 s"

An earlier reading of the f1 sweep said three programs reached the pit's far
station at **5.389** against **6.999** for the human world record *and* for our
own incumbent. The same number twice looks exactly like a real shortcut.

**`fk trace` says they fall off the world.** y reaches **8.0** — the plane under
the map — and the car slides 800 m west of the map at 368 km/h; on the way down
it flew through the relocated gate's plane and fired it. The trace is sound
(self-check ok, 1956 rows, |d(pos)/dt − v| median 0.052 m/s), so this is a real
car really falling.

**Fourth instance on this map of a search record read as a route.** The
mechanism now has a name and two mechanical guards, written into `CLAIMS.md` §3:
score the gate together with a containment predicate, and compare path length
against the reference (the faller's own regeneration anchor measures **1189.5 m**
against the real run's **817.6 m**).

### RETRACTED: the first negative, and why its control did not catch it

The f1 sweep's first scoring was against **one** relocated gate in **one**
orientation. That instrument is a knife-edge here: moving the anchor **2.7 cm**
turns a 6.999 arrival into a DNF, **3 of 3 runs each way**, and the same station
fires at yaw π/2 and DNFs at yaw 0. A y-sweep gives the rule — the trigger
volume hangs **downward** from the anchor, the car must be below it, and at
+0.5 m it reaches far enough down to catch a **different, earlier pass** and
report 5.499 instead of 6.999. **A gate that fires 1.5 s early does not look
broken; it looks like a better time.**

The positive control in that batch fired, and caught nothing — because the
control is the tape the placement was fitted to. `CLAIMS.md` §3 already names
that failure; this is a fresh instance of it.

**The ruler that IS admissible here is at the turbo gate**, and what makes it
admissible is that it is **invariant**: `i11@846,117,720/1.5708` reads human
**15.370**, our incumbent **14.766**, the east-flick **DNF**, *identically at
dy = 1, 3 and 5 m*. A faller cannot reach it.

---

## 4. What a searched U is worth: two measured frontiers

Both use the fork's own state readout (`tmsearch --fork --gate`) — a **box with
six explicit bounds**, read out of engine memory, with no trigger-volume
mystery and no direction dependence. Both boxes carry a **seed identity
control** that PASSES: the fork's measured state for the reference against the
reference's own decoded telemetry, position **0.0006 m** and **0.0169 m**
respectively.

"Get there EARLY" is not expressible in the key language, which cannot see a
clock. It is expressed here as a **watchdog that aborts every candidate at a
fixed tick**, so "reached the box" *means* "reached it before the deadline", and
the frontier is a ladder of deadlines. **That construction is what exposed the
`after`-window defect in §6.**

### 4a. The U itself: eastward speed on the first platform

Box x 700..745, y 106..128, z 725..755 — the landing platform. Key
`along(1,0,0)`, the car's speed due **east**: the U-turn's whole content is
converting westward motion into eastward motion, and that is one clean number
with no weighting.

**The human's own value is the bar**, and it is what his loop buys him:

| deadline | the human's eastward speed by then |
|---|---|
| race 8.000 | **+16.93 m/s** |
| race 7.000 | +1.82 |
| race 6.000 | **−11.44** — still going west |

At the 6.000 deadline **the search refused to start**: the do-nothing tape
outscores the human there, so the objective is a decoy at that deadline and the
startup check said so and stopped. That refusal is correct and it is a fact
about the deadline, not about the flick.

**MEASURED — the U-turn works, and it is worth 11.4 m/s and 1.0 s over the loop.**
Two independent searches, 105 minutes each, ~30 workers each:

| deadline | the human's loop gives | the flick gives | winner's state, from its own `.state.json` |
|---|---|---|---|
| race 8.000 | **+16.93 m/s** | **+28.34** | tick 955, (727.02, 113.48, 735.51), v (28.34, **−0.14**, −0.16) |
| race 7.000 | **+1.82 m/s** | **+28.35** | tick 854, (742.28, 107.45, 725.05), v (28.35, **−0.06**, −0.15) |

`v_y ≈ 0` in both, so **the car is on the ground driving, not falling past the
box** — this is the check the retraction above exists to enforce.

**The manoeuvre, tick by tick, from `fk trace` on the race-7.000 winner**
(`if_U855_trace.csv`, banked; self-check ok, 2080 rows):

| race | steer | gas | brake | v_x |
|---|---|---|---|---|
| 5.100 – 5.300 | +0.03 → +0.14 → **+0.45** | off | off | −13.6 |
| 5.400 – 5.600 | **+1 (full lock)** | **off** | **pulsed at 5.500** | −8.6 → −6.5 |
| 5.700 – 6.000 | +1 held | on | off | −1.6 → **+8.0** |
| 6.200 – 6.400 | **−0.81 → −1 (counter-steer)** | on | off | +17.0 → +21.0 |
| 7.000 | settling | on | off | **+28.3** |

**That is a Scandinavian flick and it is exactly the recipe §1 predicted**:
steer built away from the turn, full lock with the throttle lifted and a brake
pulse, the car rotates, then counter-steer to catch it. **The rotation from
heading west to heading east takes 0.600 s** (5.400 → 6.000) with the car on the
platform the whole time. The human takes **2.150 s** to reach v_x = 0 and does
not reach +17 m/s until 8.000.

**And the picture says it plainly** (`u_vs_wr.png`, banked): the human's line is
a figure-eight — two loops — and the flick's is **one tight U straight out onto
the ramp**. That is vjeux's description, drawn.

**The plateau is real and it is geometric, not a search failure.** Both
deadlines converge to 28.34 to four significant figures. The box ends at
x = 745, so what is being measured is the eastward speed available *by the east
edge of the first platform*, and a whole extra second of deadline does not
raise it. **The U delivers 28.34 m/s east by race 6.99, and that is the whole
of what the corner has to give.**

### 4b. Downstream: speed through the launch gate

Box x 840..854, y 111..119, z 704..716 — the deck slab at the turbo gate, tight
enough that a car below the deck is not in it. Key `speed`. Baselines:

| | at the launch gate |
|---|---|
| human WR | **57.11 m/s** at 15.550 |
| our 21.022 incumbent | **56.44 m/s**, arriving before 15.45; **9.56 m short** by 14.45 and **52.10 m short** by 13.45 |

| deadline | best speed at the launch gate | winner's state |
|---|---|---|
| race 15.45 | **58.95 m/s** at race 15.080 | (853.93, **114.00**, 704.07), v (56.26, −1.18, −17.56) |
| race 14.45 | **43.69 m/s** at race 14.370 | (853.997, **114.03**, 714.73), v (42.60, **−0.10**, +9.67) |
| race 13.45, from the U | never reached, **3.94 m** away | — |
| race 12.45, from the U | never reached, 5.10 m away | — |

Both winners sit at `y = 114.0` with `v_y ≈ 0` — the deck's own height, not
falling through it.

**Confirmed on a second, independent instrument.** The plain oracle on the
invariant turbo-gate ruler, which knows nothing about the fork:

| | through the turbo gate |
|---|---|
| human WR | 15.370 |
| our 21.022 incumbent | 14.766 |
| **`L1600`, the race-14.45 winner** | **14.059** |

**0.707 s earlier through the launch than our own best lap, and 1.311 s earlier
than the human** — measured twice, by two instruments that share no code.

### And this is where it stops, with a number

`L1600` pays for that with speed: **43.69 m/s at the launch against the
incumbent's 56.44 and the human's 57.11.** Its trace says why — it runs the
deck's low southern edge at 161.8 km/h at race 9.500 and `y = 103.9`, **eight
metres below the deck**, then has to climb back up and **stops dead at
(831, 115.6, 706) — 4.2 km/h at race 11.500** — before the turbo pad re-launches
it. It reaches the launch early because it took a shortcut, and slowly because
the shortcut cost it everything.

**Seeding the launch hunt from the U made it worse, not better** (3.94 m short
at race 13.45 where the incumbent-seeded arm reached the gate at 14.37). The U
puts the car east *early and low*; the launch wants it *high and fast*.

**And nothing converts to a lap.** A 70-minute plain-oracle search over
`[1400, 2460]` seeded from `L1600` — the whole endgame, free — returned
**0 finishers in 344 160 evaluations**. It had no gradient to work with: this
map has one checkpoint, so every failure scores `DNF cp0` and the search is a
random walk. `--seg` is the mechanism for that and it refuses here (§5).

### The first version of this frontier was wrong, and reading the state is what caught it

A looser box (y 106..122) produced an apparently spectacular ladder — 44.28 m/s
at a race-12.95 deadline, 1.8 s earlier than our incumbent's launch. **Every
winner's banked state says `pos y ≈ 106.1, vel y ≈ −19.0`.** They are 8 m below
the deck and falling at 68 km/h: the southern-fall family an earlier arm already
mapped, passing through the box on its way down. The box was tightened to the
deck slab and the ladder rebuilt. **`--gate` writes the winning state to a
`.state.json`; read it before believing a key.**

---

## 5. What is left, and what would settle it

**The next arm's task is one sentence: the U exists and is 1.0 s and 11.4 m/s
better than the loop at the corner; it has to be joined to a run-up that ends
high.**

* **SETTLED — a continuously-steered U exists** (§4a), and no enumeration of
  constant phases finds it (§3). Search, do not enumerate, for anything of this
  shape.
* **MEASURED, and it is the wall — the U is east and LOW.** The launch wants
  `y ≈ 114` at 57 m/s; the U's own line is at `y = 107.5` at 28.3 m/s and the
  best thing built on it runs the deck's southern edge **8 m below the deck**
  and stops dead at (831, 115.6, 706) before the turbo pad rescues it.
* **UNKNOWN — whether the U can be turned NORTH and UP into the human's own
  24 m run-up** instead of east. This is the reading of vjeux's instruction I
  tested last and it is the one still open: three searches at deadlines 7.95 /
  8.95 / 9.95, seeded from the U tape, all sat at **4.14 m** from a box on the
  second platform's top and moved 0.01 m in fifteen minutes. **That is a flat
  gradient, not a refutation** — the seed's 4.15 m is achieved at race ~5.1 on
  the way past, so closest-approach cannot see the climb at all. What would
  settle it: a gate box the U's line does *not* graze early, or a `--fire`
  clause on the climb with the after-key measuring the descent.
* **UNKNOWN — whether any early launch converts to a lap.** 344 160 plain-oracle
  evaluations over the whole endgame from `L1600` produced **0 finishers**,
  because a one-checkpoint map gives every failure the same `DNF cp0` and the
  search is a random walk.
* **The `--seg` gradient, which is the fix for that, is unavailable here.**
  `tmsearch --seg 1:<turbo-gate map>` refuses: *"the incumbent does not finish on
  this segment map (it returns wrong simu)"* — even though the plain oracle
  returns **14.059** for that pair. Worth a look; it is the difference between a
  random walk and a search over the whole back half of this map.

---

## 6. Tooling changed on branch `iceflick`

| commit | what |
|---|---|
| `7081496` | **`tmprog` ported** to `tape::Patcher` / `forkoracle::inputs::Inputs`; the banked `imt2_tmprog_v1.rs` had not built since the audit moved the codec into `tools/ghost`. New **`--tail rejoin:N`**: hand back to the reference **N ticks earlier in its own clock**, because a trick that reaches a place sooner must rejoin by POSITION, not by clock. Two-sided control in the message. |
| `fd9cc89` | **`GateReport::event` reported `after = 0` for an EMPTY after-window**, conflating it with "no after-key given". Every documented after-key is a negated distance, so 0 was unbeatable and **firing on the run's last tick strictly dominated firing early and then doing the thing** — this search climbed −27.93 → **+0.0000** and stopped dead, its winner's firing tick being the abort tick. Now negative infinity, pinned by `score::tests::an_empty_after_window_loses_to_every_real_after_key`. **93 checks pass with `TM_REQUIRE_ENGINE=1`.** |
| `a60e441` | **`CLAIMS.md` §3: "A relocated gate measures a PLANE CROSSING, and a plane crossing is not a route"** — the faller, the 2.7 cm knife-edge, the guards, and the invariance requirement. |
| (in `a60e441`) | `tmmaps` `rungspec --curtain` did not compile at head (`Vec<usize>` defaulted from a `String` block id). Another arm fixed it identically the same night; theirs is in the merge. |

---

## 7. Controls, in one place

Every one of these ran in this arm, on this node, against this map.

| control | result |
|---|---|
| plain oracle re-simulates the two reference files | human `23.068`, incumbent `21.022`, exact |
| oracle determinism on a relocated-gate map | 3 of 3 identical, twice, on two placements that disagree with each other |
| `tmprog --tail rejoin:0`, empty program, on the incumbent | **21.022** — reproduces the template |
| `tmprog --tail rejoin:20`, same empty program | **DNF** — the negative half |
| the flick family is alive (`rung/r8600_0`) | **2593 of 2593** fire it; 2013 of 2593 fire `r5400_0` |
| fork seed identity, first-platform box | PASS, position **0.0004 m**, speed 0.0032 m/s, attitude 0.008° |
| fork seed identity, launch-gate box (tight) | PASS, position **0.0006 m**, speed 0.047 m/s, attitude 0.009° |
| fork seed identity, launch-gate box (wide) | PASS, position 0.0169 m |
| decoy test, launch-gate box | do-nothing tape 89.15 m away; incumbent `GATE key +56.4395` |
| decoy test, first-platform box at a race-6.000 deadline | **the do-nothing tape WINS — the search refused to start.** Correct, and it is a fact about that deadline |
| turbo-gate ruler invariance | identical at `dy` = 1, 3 and 5 m |
| every reported gate winner | its `.state.json` read for `y` and `v_y` before it was written down; two frontiers were discarded this way |
| `cargo test --release` with `TM_REQUIRE_ENGINE=1` | **93 pass, 0 fail** |

## 8. Artefacts

Banked to `~/persistent/private-30d/tm-unbeaten/267460/if_20260823/`.

| file | what |
|---|---|
| `RESULT_iceflick.md` | this page |
| `HYPOTHESIS.md` | the technique write-up, dated **before** any test was run |
| `U855.Ghost.Gbx` | **the U-turn tape** — +28.35 m/s east at race 6.990. DNF as a lap |
| `U955.Ghost.Gbx` | the race-8.000 sibling, +28.34 |
| `L1600.Ghost.Gbx` | the launch tape — turbo gate at **14.059**, 0.707 s inside our incumbent. DNF as a lap |
| `if_U855_trace.csv` | `fk trace` of the U, 2080 rows, the tick-by-tick flick in §4a |
| `u_vs_wr.png` | **the picture: one tight U against the human's figure-eight** |
| `final.png` | the U, the launch tape and the human, whole pit and deck |
| `pit_time.png` | the human's pit coloured by time — which loop is which |
| `if_f1_index.tsv`, `f4_*.tsv`, `alive_r8600_0.tsv` | the enumerated families and their scores |

**None of these is publishable and none is a lap.** `U855` and `L1600` are
DNF; they are states, not results.

---

## 9. Addendum — the two arms that ran after §5 was written

**Search totals, all completed, all clean:** `u855` 9 257 325 evaluations /
268 confirmed improvements, `u955` 8 329 275 / 101, `w1600` 7 720 500 / 127,
`v1400` 6 660 675 / 408 — **zero phantoms in any of them.** Every banked
improvement was re-validated by the plain oracle.

**The "turn the U north and up" objective is not searchable as posed, and the
tool said so before spending anything.** Both arms — a box on the second
platform's top, `y 127..140, z 756..775`, seeded from the U tape, deadlines
race 8.45 and 9.95 — **refused to start**: the do-nothing tape gets closer to
that box than the U tape does.

> That refusal is a measurement. **From the U's exit state, DRIVING takes you
> away from the second platform** — the flick points the car east and the second
> platform is north, so a tape that stops driving coasts nearer to it than one
> that keeps going. It is the same wall §4b prices, arriving from the other side.

What would make it searchable: seed from something that is already turning north
(not the U's east-pointing exit), or arm a `--fire` clause on the climb with the
after-key measuring the descent, so the objective is *"climb it, then go down
it"* rather than *"be near the top"*.

**The endgame search finished its null: 1 099 980 evaluations, 55 minutes,
0 finishers**, over `[1400, 2460]` from `L1600` on the real map. Flat `DNF cp0`
throughout, as §5 predicted. **This is a statement about the gradient, not about
the route.**
