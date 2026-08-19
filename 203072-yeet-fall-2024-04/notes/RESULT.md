# 203072 — `YEET Fall 2024 - 04` — the author time falls by 0.692 s, and the map was never broken

**Headline for a driver, before any detail:**

> This map was written off as unusable. It is not: the oracle reproduces it to
> **1.7 mm**, and the 31 % of the leaderboard that would not re-simulate turns
> out to be **a bounded window of game builds** (2026-01-18 … 2026-04-28) with
> **80 / 80 perfect on both sides of it**. Once you can search it, the map gives
> up 1.4 seconds. And what the 272-person field is missing is not a route — our
> line sits 7.7 m from the world record's own line. It is that **everybody
> climbs.** The human record arcs to 99 m and has to come back down; we top out
> at 82 m and spend the reactor on going *forwards*. The last three differences
> that produced our final tape are **two throttle blips and one wheel you do not
> unwind** — and none of the three works without the other two, which is exactly
> why 272 people never stumbled onto it.

| | time | vs AT | what it is |
|---|---|---|---|
| human online WR `ayti__` | **12.083** | +0.749 | best of 272 records |
| **author time (AT)** | **11.334** | — | never beaten by a human |
| our best unconstrained TAS | **10.642** | **−0.692** | §3 |
| action-key flight (6 values, 22 events) | **10.717** | **−0.617** | §7 |
| **keyboard flight (3 values, 14 events)** | **10.743** | **−0.591** | §7 — the drivable one |

uid `EnEzR77_U96WsKsPyet1Ay4lcCc` · Nadeo mapId `9962e5e5-cf56-45cc-beb0-8a7660f8d39e`
· TMX 203072 · map sha256 `b821d5e71d05047f9c306ad55eec8de5c016bfa9f81072585572d1589f2b853d`
(Nadeo's own CDN, anonymous — byte-identical to the banked copy).
**Nothing here has been or will be submitted to a Nadeo leaderboard.**

Times are seconds with a decimal throughout.

---

## 1. The map was on the abandoned list. Getting it off was most of the work.

`ACQUISITION.md` §8 names 203072 as *the* canonical unusable map, and the
previous agent on it ran **no search at all** for that reason. §8b then asked
for exactly this re-examination:

> "203072's 24/34 was described as build-correlated 'but backwards'; it deserves
> re-examination in this light before it stays on the abandoned list."

Full write-up: `RESULT-section8-resolved-v1.md`. In brief, four things were
measured before a single search was started:

1. **There is no second version of the map.** Nadeo's own `/maps/<guid>/file` is
   **sha256-identical** to the copy under test. §8 called resolving this "an
   authenticated fetch this project has deliberately not attempted"; the same
   file's own 270051 update already says it needs no authentication, and it
   does not. §8 candidate (a) is dead.
2. **Not the §8a truncation trap** — every ghost's telemetry runs past its own
   declared finish, and a fresh private download with `.part` discipline
   reproduced the previous agent's table ghost-for-ghost.
3. **The oracle is faithful on this map.** `fk verify` re-simulates a ghost's
   own tape and compares against that ghost's own recorded telemetry:
   **1.75 mm / 1.72 mm / 1.72 mm position RMS** on three ghosts from three
   different builds, over full 12–14 s runs. The 134672 precedent that
   exonerated *that* map measured 8 mm.
4. **The whole field, n = 270 of 272** — not a 34-ghost sample:

| recording build | n | reproduces |
|---|---|---|
| 2024-09-16 → 2025-07-04 (five builds) | **76** | **100.0 %** |
| **2026-01-18 → 2026-04-28 (three builds)** | **190** | **48.4 %** |
| 2026-07-22 | **4** | **100.0 %** |

**80 / 80 outside the window. 92 / 190 inside it.** It was never "old versus
new" — the previous reading was made on a sample in which the post-window
builds had n = 1. It is a **bounded interval**, with perfection on both sides.

Why only half the window fails: the map is chaotic (§2), so any physics
difference is amplified without limit — which predicts failures that look like a
coin flip independent of driver skill, and that is what they are (31–63 % across
finish-time deciles, no trend; 100 % outside).

**The cost of the window, stated plainly: the human world record was set on
2026-02-02, inside it, and DNFs in our oracle.** Every search here is seeded
from outside the window (p002, 12.242, recorded 2024-09-17, which re-simulates
exactly and rebuilds through our own encoder to 12.242).

## 2. What the map is

| phase | race | what |
|---|---|---|
| A | 0 → 4.4 s | ground, accelerate to ~200 km/h |
| B | 4.4 → 6.4 s | two full-lock direction changes, then the launch |
| C | **6.4 s → finish** | **~4.2 s of powered flight**, 228 → 430 km/h, finish plane at **z ≈ 702.7** |

Boost direction, measured off the body frame: **71–76 % of the non-gravity
acceleration lies on the car's own +forward axis** (`an thrust`). This is *not*
274191's belly-mounted reactor pushing out through the floor at 87 % on −up.
Here the thrust is **out of the nose**, so *where the car points is where it
goes*, and the flight is a thrust-vectoring problem with the nose as the vector.

Two structural facts that shaped everything:

* **No decided tail.** `tmprobe hold` freezes every input from tick T on and
  sweeps T: **every T from 400 to 1200 DNFs**, only T = 1220 (three ticks from
  the line) survives. The whole flight is live control.
* **The car model has no purchase here.** `tmtas carmodel` over the field
  explains **6.6 % of yaw-rate variance** — the water/loop end of the scale
  (2.7 %), not the road end (71 %). So no steering prior, corridor or predicate
  was used, exactly as the brief instructs.

### 2a. An 840 ms dead zone in the early flight

`tmprobe range` forces a constant over a window and sweeps the constant.
On the 11.461 tape, **every one of 15 constants returns 11.461 exactly** for
every window inside ticks 793–876, and single-tick perturbation confirms the
edges to the tick:

```
tick 792  ->  11516        (live)
ticks 793 … 876            DEAD   -- +80 steer on any single one returns 11461
tick 877  ->  DNF          (live)
```

**840 ms of the flight in which steering does nothing at all.** Outside it the
same perturbation mostly DNFs — the map is hypersensitive everywhere else.

It is not a place on the map: p002 is ~50 m further along at the same moment and
has the same window. Across four fast runs the window starts at each run's own
takeoff and ends at race ~7.0–7.2 s; a slow run (p150, 26.272) has no such
window in that range at all. So it is tied to the flight, not to the clock and
not to a location — the same shape as 274191's "no air control between leaving
the road and half a second after the launcher". **The mechanism is not
established and I am not claiming one.**

Consequence for a driver: **the attitude you carry into 6.4 s is the attitude
you have at 7.2 s, and nothing you do in between matters.**

## 3. Where the time is: everybody climbs

Our line is **not** exotic. Mean distance to the nearest point of the human
world record's own path, over the whole run:

| tape | mean | max |
|---|---|---|
| **ours** | **7.7 m** | 36.4 m |
| p002 (rank 2) | 6.1 m | 22.4 m |
| p003 | 14.4 m | 39.3 m |
| p005 | 15.3 m | 48.0 m |
| p011 | 19.2 m | 62.9 m |

Same route. The difference is the **shape of the arc**:

| run | apex height | apex time |
|---|---|---|
| **ours** | **81.8 m** | **8.85 s** |
| p001 human WR | 99.3 m | 9.70 s |
| p002 | 97.2 m | 9.40 s |
| p005 | 120.1 m | 9.47 s |
| p003 | 130.3 m | 9.25 s |

**We fly 17.5 m lower than the world record and apex 0.85 s earlier.** Every
human in the sample climbs higher than we do. Station by station the world
record is *ahead* of us through the middle of the flight and then spends
1.5 seconds coming down — it reaches z = 740 at 9.95 s against our 10.69, and we
still cross the line 1.2 s sooner, because it has to convert all that height
back into progress while we never bought it.

That is the whole finding, and it is the same one 274191 reached independently
on the other YEET map from the other direction ("the human lets the reactor
swing up to +53° where we hold it at +17–25°"). **On a nose-thrust flight map
the field's systematic error is aiming the boost too high.**

## 4. The technique: three changes, none of which works alone

The final tape differs from its immediate predecessor (11.426) in **exactly
three places**, all in the last 1.5 s before the launch, and the improvement is
**566 ms**:

| | race | what |
|---|---|---|
| **R1** | 4.82 – 4.85 s | **40 ms off the throttle**, during the full-right lock |
| **R2** | 5.49 s | **10 ms off the throttle**, during the full-left lock |
| **R3** | 6.25 – 6.36 s | **hold ≈ 10 % left lock into the launch** instead of unwinding to centre |

A full 2³ factorial against the oracle:

| edits | result |
|---|---|
| none (base) | 11.426 |
| R1 | **DNF** |
| R2 | **DNF** |
| R3 | 11.561 (*worse*) |
| R1+R2 | **DNF** |
| R1+R3 | **DNF** |
| R2+R3 | **DNF** |
| **R1+R2+R3** | **10.860** |

**Every proper subset fails.** This is an irreducible three-way interaction, and
it is the honest answer to "what did 272 people miss": *nothing incremental*.
Any single piece of it makes your run worse or ends it, so no amount of
iterative human refinement finds it — the gradient points away from the answer
in all three directions.

**Verdict: KNOWN-BUT-UNREACHABLE, not undiscovered.** The route is the field's
own. The trick is a combination lock.

**An honest negative, stated because it matters:** grafting R1+R2+R3 onto a
human tape (p002, p005) **DNFs**. These are the last three differences within
*our* lineage, not a recipe to bolt onto a human run. What transfers to a human
is §3 (fly flatter) and the drivable tape in §7 — not these three edits in
isolation.

## 5. Correctness

* **Identity control**: the seed re-simulates to 12.242 exactly, and rebuilds
  through the search's own encoder to 12.242.
* **Every headline tape re-validated cold, in a second process with a different
  binary** (`/tmp/tmtas-rs` vs `/tmp/tmtas-hard`), with **three known-answer
  controls in the same batch** returning 12.242 / 13.762 / 14.191 exactly.
* **A ratchet ran throughout**: every new champion re-validated cold with a
  control before being banked. **Zero phantoms in the entire session**; nothing
  was written to `tm-loop/phantoms/`.
* **Every search process had its own staging root** (the phantom-manufacturing
  hazard: 7 of 13 shared-root runs produce phantoms, 0 of 8 with distinct roots).
* **Raw server verdict on the winning tape**: `"NbRespawns": 0`,
  `"NbCheckpoints": 1`, `"MapUid": "EnEzR77_U96WsKsPyet1Ay4lcCc"`,
  `"Time": 10860` → `10642`. No respawn, no skipped geometry, right map.

## 6. The predictions from PLAN.md, scored honestly

`PLAN-v2-reopen.md` (md5 `ea239bcbc7fa3df2b24019d15bfdbb3d`) was written before
any search.

| | prediction | outcome |
|---|---|---|
| **P1** | a validated time ≤ 11.334 | **CORRECT** — 10.642 |
| **P2** | ≥ 70 % of the gain accrues after the last ground contact | **WRONG, and instructively so.** The decisive three edits are all *before* the launch (4.82 / 5.49 / 6.25 s). The gain is *delivered* in the air but *bought* on the ground — the same shape 274191 found. |
| **P3** | launch speed within ±5 %, mean projected thrust up ≥ 10 % | **PARTLY.** The mechanism is right (thrust aiming, §3) but I predicted it as a *thrust-projection* effect; what it actually is, is an *apex-height* effect, and the winning edits change the launch attitude rather than the in-flight aiming. |
| **P4** | field-level corr(projected thrust, finish) ≤ −0.5 | **NOT TESTED.** Superseded: apex height separates our tape from all five sampled humans without needing a field correlation, and the 228811 rule (report spread, declare untestable rather than publish a weak r) applies. Recorded as not done, not as supported. |
| **P5** | refuted if no improvement / a phantom / gains all in phase A | none occurred |
| **P6** | keyboard member under the AT with ≤ 25 change events | **CORRECT** — 10.743 with **14** flight events on 3 values |

## 7. The low-input family

Alphabet read off human tapes, never invented: several strong records in this
field are *already* action-key runs — **p005 finishes 12.366 using exactly
`{−127, −51, 0, 50, 127}`** with 39 steer events, and p008/p011 use 4 values.
That is the ladder used below.

**Whole-tape projection fails**, as the project's rule says it must: quantising
the 10.642 tape to keyboard over its whole length DNFs (26 events, 3 values,
DNF). **Projecting only the part that tolerates it is free** — 274191's lesson,
confirmed here, and the difference is one range:

| member | steer alphabet | flight events | time | vs AT |
|---|---|---|---|---|
| unconstrained | 160 values | 134 | **10.642** | −0.692 |
| action-key, flight from 6.44 s | `{−127,−51,0,50,127}` (6 incl. boundary) | 22 | **10.717** | −0.617 |
| **keyboard, flight from 6.44 s** | **`{−127, 0, 127}`** | **14** | **10.743** | **−0.591** |

All three cold-re-validated with controls. **The entire 4.2-second flight can be
flown on three keyboard values and fourteen presses, 0.591 s inside an author
time no human has ever matched.**

### The fourteen presses

`−127` = hold left, `0` = release, `127` = hold right. Ground phase up to 6.44 s
is ordinary analog driving plus the two throttle blips of §4.

```
  6.44 s   release the wheel        (into the dead zone -- nothing matters until 7.20)
  7.20 s   FULL LEFT
  7.35 s   release
  7.88 s   FULL LEFT
  8.35 s   release
  8.56 s   FULL LEFT   (brief)
  8.59 s   FULL RIGHT
  9.23 s   release     (brief)
  9.25 s   FULL RIGHT  (brief)
  9.28 s   release     (brief)
  9.30 s   FULL RIGHT
  9.98 s   release
 10.05 s   FULL RIGHT
 10.26 s   release
 10.64 s   finish
```

Throttle: full throughout, except **off 4.82 → 4.85 s** and **off at 5.49 s**
(§4). Brake taps at 3.01 s and 4.01 s are inherited from the seed's ground
phase.

### How tight is it — with the control UNBEATEN.md requires

Recoverable tolerance (mistime one event, re-time only the later ones — what a
driver who is late actually does), ±1…3 ticks on every flight event:

| tape | variants | still finish | still within 50 ms of own base |
|---|---|---|---|
| **ours (keyboard, 14 events)** | 84 | 88 % | **85 %** |
| **human p002's own tape (control)** | 186 | 96 % | **45 %** |

**Our drivable tape is nearly twice as tolerant as the human seed's own
driving.** Twelve of the fourteen presses have ±30 ms of slack. The exceptions:
the release at **6.44 s** is genuinely tight (±1 tick DNFs — it is the entry to
the dead zone), and the two full-lefts at 7.20 / 7.35 s are fragile on the early
side only. The 9.23–9.30 s cluster is three flickers inside 70 ms and is the
part that will take practice.

So the honest answer to "is this humanly realistic": the flight is *more*
forgiving than the human seed's flight, and the hard parts are the single
release at takeoff and one 70 ms cluster. **"Not humanly executable" is not the
verdict here, and the AT itself is proof a person drove this map faster than the
field ever has.**

## 8. Negatives, with what was swept

Stated per the fleet method rule (a negative from a hand-enumerated list is
worth nothing unless you say what was enumerated):

* **Best-of-field splicing does not work on this map.** 60 splices — 10
  reproducing donors × 6 boundaries (donor flight onto our ground at ticks
  560/600/640/700; our flight onto donor ground at 560/640) — **all 60 DNF.**
  This is *not* the container-portability problem (all grafts were within one
  container, rewriting the input arrays in place); it is that the lap is a
  single integral, which the hold-probe independently shows. Cross-run
  composition is unavailable here.
* **Gas/brake windows buy nothing in the air.** 44 candidates — 11 windows of
  50 ticks across the flight × all 4 gas/brake combinations — no improvement.
* **No embedded author ghost.** §9a-verified, not taken at face value: the LZO
  body was decompressed (2 689 616 bytes) and scanned — 0 × `CPlugEntRecordData`,
  0 × ghost inputs, 0 × ghost body, 0 × ghost splits, 0 occurrences of the
  string `CGameCtnGhost`. So the brief's primary plan — read the author's own
  lap out of the map — was unavailable here, and everything above was
  reconstructed without it.

## 9. Transferable findings

### For ACQUISITION.md

1. **§8 needs a third branch.** Not just "old ghosts fail ⇒ exclude them" but
   **"an interval of builds fails ⇒ exclude the interval"** — which is invisible
   unless *both* edges are sampled. A 34-ghost sample gave a qualitatively wrong
   shape here ("backwards") purely because the post-window builds had n = 1.
2. **When a §8 check fails, read the build off every ghost before interpreting.**
   It is free and it is behind the LZO, so `strings` on the file finds nothing:
   `tmmaps scan GHOST` (added this session) prints it.
3. **A §8 shortfall is not evidence about the oracle.** `fk verify` answers "is
   our physics right on this map" directly, in one command. Here it said yes to
   1.7 mm while 31 % of a sample was failing.
4. **203072 comes off the abandoned list**, and §8's own text should stop citing
   it as the canonical unusable map.
5. **§9f's "the saving build is the discriminator" for a missing author ghost is
   contradicted here**: 134672 had no ghost and was saved on 2022-07-06, but
   203072 has no ghost and was saved on **2024-09-17**, contemporary with maps
   that do carry one.

### For searching

6. **Verify the tape's clock offset before setting `--hi`, and verify it against
   the tape, not the telemetry.** This tape's `start_offset_ms` is **−1560**, so
   the finish is at tick 1380, not 1224. I set `--hi 1230` from the telemetry
   and **froze the last 1.5 seconds of every candidate for the first 45 minutes
   of search.** Unlocking it moved the best from 11.461 to 11.426 within
   minutes and opened the region that produced the breakthrough. The brief warned
   about exactly this and I still did it; the check that would have caught it is
   one line: `tmprobe events` prints the offset.
7. **Retune the search from its own improvement log.** Tallying `op=…@tick` over
   the first 20 minutes showed every improvement in ticks 0–199 or 600–1299 and
   none in 200–599; restricting arms to the productive regions broke a plateau
   that four operator configurations had not.
8. **Sweep a constant through the air phase before writing anything** (274191's
   rule, re-confirmed). One `tmprobe range` sweep found an 840 ms dead zone that
   changes what a driver should be told and where the search should spend.
9. **The dead zone is not where the search should look, but its EDGE is.** The
   breakthrough operator fired at tick 792 — one tick before the dead zone
   starts. What you set going into a dead window is worth more than anything
   inside it.

### Tools added (banked in `tools/`)

`tmmaps scan` (decompress a map or ghost body; count ghost class ids; print the
recording build), `tmprobe graft` (replace a tick range from a donor — for
splicing and for making `fk traj`-acceptable analysis variants), `tmprobe
quantrange` (project steer onto an alphabet over a range only — this is what
produced the whole low-input family).

## 10. Files

`acq/nadeo.Map.Gbx` (sha256 above) · `acq/ghosts/` (270 records) ·
`acq/val_full.txt`, `acq/builds.tsv`, `acq/joined.tsv` (the §1 tables) ·
`acq/fid_*.txt` (`fk verify` fidelity) · `tapes/m203072_TAS_10642_*.Ghost.Gbx`
(the unconstrained best) · `tapes/` also carries the improvement ladder ·
`family/` (the three low-input members + controls) · `PLAN-v2-reopen.md` ·
`RESULT-section8-resolved-v1.md` · `tools/`.
