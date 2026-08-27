# Haystack 6 — compiled research notes

Everything we established about **Haystack 6** (by m1el), as of 2026-08-27.

Stock map MD5 `440c2bc538d9fcff2bde3f595c0d2f21`, verified unchanged before and
after every experiment. The map was never edited.

Claims are tagged **MEASURED** (driven through the engine, with its control),
**DERIVED** (arithmetic on measured values, method stated), or **CONJECTURED**
(a reading that has not yet had its control). Times are seconds.

---

## 1. Where it stands

| | |
|---|---|
| Best fully driven route | **278 moves, 278 fresh groups** — every respawn placement exact, reproduced on a second fork |
| All 322 groups individually collectable | **yes**, each with a direct driven proof |
| A complete once-only route | **not found** |
| A driven continuation (two gates, one drive, no respawn) | **never achieved**, in ~900 driven tapes |

The 278 is a **free-endpoint lattice route**: it starts at an arbitrary lattice
landmark, ends at another, and omits the mandatory start cascade and the finish
funnel. It is a fragment library and an ordering prior — **not a prefix of a
real Start→Finish run**, and its length is not a meaningful "44 short of 322".

Video of the 278 run: [issue #2](https://github.com/vjeux/trackmania-tas/issues/2).

---

## 2. The engine's actual rule

Not a static one-colour edge. Transitions are **ordered, set-labelled events**
with held-event suppression and history-dependent destinations:

```text
fresh       = [group in ordered_events(move) if group not already held]
move legal  iff fresh is non-empty
held       |= fresh
destination = respawn state of the LAST FRESH event
```

Three consequences that broke earlier models:

- **One move can collect 2–3 groups.** The rule is only "every move must collect
  at least one group not previously held" — never "one group per move".
- **The destination depends on `held`.** A 30-move tape collecting a primary at
  move 12 and then crossing the same pocket at move 30 emits only the bonus
  event and respawns *inside the bonus pocket*, not at the ordinary landmark.
- **The authored waypoint graph is not the driven vehicle graph.** The lower
  `CPBox_U` trap-box escapes exist in the map data and cannot be fired from the
  state ordinary entries place the car in.

Structural facts: the 8 bonus-only teleports are **terminal** (legal only as a
route's last move); the 22 lid-blocked rooms are **dead origins but live
destinations** (reachable only as a last move).

---

## 3. The author's own route, recovered from the map file

The map's validation replay was stripped — the metadata literally says
*"validation replay removed"*, with a remover tag. But
**`Race_AuthorRaceWaypointTimes` survived**: 323 strictly increasing event
times, **1.709 → 911.615** (322 groups + finish). MEASURED.

Nine of the 322 gaps are under 0.25 s — two gates collected in one fall — so:

| quantity | value | class |
|---|---|---|
| **Physical crossings in the author's route** | **314** | DERIVED |
| — entered by a respawn move | 297 | DERIVED |
| — entered by continuous driving, no respawn | **16** | DERIVED |
| Multi-event crossings | 8 (7 × 2 events, 1 × 3) | DERIVED |
| Finish bundled into the last checkpoint's fall | yes, 0.231 s | DERIVED |

**The two populations are separated by an empty band.** No gap falls in
**[1.817, 1.995]**, a hole about 17× the local gap spacing — the split is a hole
in the data, not a threshold anyone chose.

**The terminal drive.** Events 311→322: **11 consecutive drive-cadence gaps,
mean 1.606, sd 0.104, span 17.665** — 12 checkpoints, then the finish only
0.231 later while still falling. Blocks 1–5 are isolated single drive crossings
mid-run (41.653, 299.573, 482.342, 565.644, 570.173).

**Two independent measurements agree**: the author's timing implies exactly 8
double-collect pockets, and the project had separately measured exactly 8
pockets carrying a second gate. Six intra-fall gaps invert through the derived
gravity to **0.94–1.04 m** against a *driven* 1.00 m pocket separation — ≤0.06 m
agreement between instruments that share no source.

**This is the load-bearing finding: every model this project built assumes each
move ends in a respawn press. That is false for 16 of the author's 314
crossings, and false for his entire terminal descent.**

---

## 4. Constants we measured

| quantity | value | notes |
|---|---|---|
| Release-acquire | **0.490** | car crosses its gate this long after throttle; n=30, 10 landmarks, 3 throttle onsets |
| Fall-acquire | **1.490** | four independent landmarks, spread **0.000** |
| Structural respawn floor | 1.82 | 103-tick dead time + 79-tick fall; a *respawn* price only |
| Priced repertoire floor | 2.04 | cheapest measured respawn primitive |
| Peak net horizontal displacement | **6.72–6.90 m at ~0.75** | and **non-monotonic** — the car arcs out and falls back |
| Horizontal reach at the 8 m drop instant | **2.42–2.6 m** | with 15.85 m of path travelled |
| vmax by family | 18–**36.41** m/s | anti-correlated with reach |

**The load-bearing correction is the axis, not the speed.** Every earlier
argument reasoned `distance = v × t`. Horizontal displacement is bounded near
6.7 m across a `v` ranging 8–36 m/s, so pricing a continuation off
`travel(distance, v_in)` overstates reach by roughly 6×.

Gravity is **gated on throttle** — a neutral tape is not a free-fall
experiment. `tape::from_spec` dithers every tick's steer by ±1, so no "straight"
tape is straight.

---

## 5. Map structure

**The exit funnel is not a chute.** 53 one-way `ExitFunnel` rooms in a
32 m × 32 m footprint over the Finish; `funnel_depth` is **radial**, not
vertical. Depths 1–6 (20 of the 44 forward gates) lie on one plane at y = 144.

| hop type | n | Δy | horizontal |
|---|---|---|---|
| `CPBoxEndFW` | 44 | **+4.90** | 6.00 m |
| `CPBoxEndU` | 8 | **+12.40** | 2.00 m |
| `CPBoxEndD` | 8 | −3.60 | 3.00 m |
| `CPBox` → Finish | 1 | −0.30 | 2.24 m |

Every ordinary hop *lifts* the car; the one descending type drops less than an
ordinary hop lifts.

**All 53 funnel rooms have out-degree exactly 1** — once inside, the chain is
forced. Every one of the 8 lattice entry gates therefore yields **10 gates +
finish = 11 crossings**. The author's terminal block is **12 + finish**, so
**block 6 is not the exit funnel.** (LID 67 is itself a funnel entry gate,
correcting an earlier note that no route into group 67 existed.)

**The lattice geometry that blocks chaining.** Gate triggers sit **9.00 m above
the pad** the car lands on and the trigger is vertically tight (±3 m), so a car
on the floor is ~6 m below every gate in its room. There is a real 8-gate fall
column at x = 752, z = 722 (`CPBox_U` → `CPBoxUp`, all groups distinct), but
each gate has a pad 9.00 m below it and the next gate 24–32 m below: a fall that
stays in the trigger column is arrested by the pad 15–23 m short, and a fall
that clears the pad drifts out of the column. Mutually exclusive.

There are **32 pairs with gate B exactly 8.00 m directly below gate A at zero
horizontal offset**, and 6,290 of 6,397 gates have a live partner within 5 m.
The geometry the author's cadence needs exists; we could not fly it.

---

## 6. Comparison with the external graph analysis

[m1el's gist](https://gist.github.com/m1el/fe744797fc68771de43362902bdca9c6) is a
strong description of the **authored** structure: 6,396 edges, 1,274 rooms, 323
LIDs. Its funnel and corridor structure we judged sound. Mapping confirmed
exactly: its 8 `double take` LIDs = our 8 bonus groups, its 8 `trapbox-escape`
LIDs = our 8 formerly-open groups, its 4 starter/hub LIDs = our 4 centre groups.

Where it and the driven oracle disagree: importing its edges gave **6,170
accepted, 226 rejected**, including all 18 burnable `CPBox_U` escapes, which the
vehicle cannot perform. Full detail in
[`M1EL-COMPARISON.md`](M1EL-COMPARISON.md).

---

## 7. Retractions

Kept deliberately, because several stood for hours and shaped the search.

| claim | status |
|---|---|
| "L=279 is UNSAT" | **retracted** — proved under an AllDifferent single-colour surrogate that misrepresents the map; scoped to the surrogate |
| "291-move route" | **retracted** — a 301-move walk with ten repeated groups is not drivable |
| "F001/F009 reach 314/322, 8 missing" | **retracted** — a lattice-tail score relabelled as `322 − 8`; the walk collected 270 distinct wanted colours and no route bytes ever existed |
| "two gates collected with no respawn press" (`drive-model`, `core-spiral`) | **retracted** — both fire in the first sample with the car at 0.53 m/s and 0.04 m of displacement; spawn artifacts |
| "the car scores one gate then stops — a physics ceiling" | **retracted** — banked containers chain 221, 220 and 68 gates on the same instrument |
| "the spawn ejects the car 13.70 m before the first tick" | **retracted** — a blind observation window, not physics (see §8) |
| "second lid real in 8 of 22" | **refuted** by driving — solid in all 18, unreachable from the lower sealed lid across 540 probes |
| "`Order` field shows 65× enrichment" | **artefact** — z = +0.70 without the arrival constraint, z = −0.15 with it |

---

## 8. Instrument defects found (the expensive ones)

**`fk trace --at tick:140` truncates 1.33 off the front of every run.** Every
arm used it. It does not change `cp_final` (read from the last log line), but it
**halved every speed measured at a falling landmark** — lm 66 8.83 → 18.14, lm
107 9.17 → 18.14, lm 3237 11.27 → 22.40 m/s — and those speeds underpinned the
"cannot reach" arguments.

**Spawning inside a gate scores it.** A six-second neutral tape, car stationary
at vmax 0.01 m/s with zero displacement, still reads counter 1 at **8 of 15**
theory landmarks. Map-wide base rate is 1.1%, but it concentrates ~36× in
exactly the closest-partner geometry every chaining experiment selects for —
which is why several arms met it and none named it. **Every chaining batch needs
a `600:0:0:0` zero-motion control at the same landmark.**

**An increment time-locked to the release, not the drive.** At lm 107 the 1→2
increment fired at 2.830 in 12 of 12 runs *including three where the brake held
the car 1.07 m short of the trigger*. A gate-to-gate time is only real if a
held-short run **fails** to fire it.

**The "spawn ejection" was a blind window.** The harness's first trace sample
lands at race time 1.11–1.22, so the tape's first ~115 ticks are applied but
never observed. Prefix the tape with 200 neutral ticks and the first observed
sample is the gate trigger centre exactly — Δ (0.000, +0.005, 0.000), speed
0.008. The apparent 13.70 m displacement was ordinary driving, unobserved.

**Steer is odd in x, even in z.** At t = 1.250, dx(−S) = −dx(+S) to 0.005 m
while dz(−S) = dz(+S) to 0.003 m across all 11 steer values. An even response
has no signed authority: both steer directions push z the same way. Worth ±6.6 m
of aim in x, **zero in z**.

---

## 9. Negative results, with their controls

Each of these had a positive control in the same batch; a negative without one
was not counted.

- **Two-gate continuation**: ~900 tapes, ~30 landmarks, every input family
  including the reverse primitives. `cp_final` never exceeded 1 outside the
  spawn-scoring artefacts. Chaining control (`ONCE265` → **221 gates**)
  reproduced on every box.
- **Exact repair around the 278**: cuts after moves 80, 120, 160, 200, 220, 240;
  replacement windows to original length + 2; all 15 centre cascades to depth
  10, two to depth 14. **No 279 in those neighbourhoods.**
- **Trap-box escapes**: 2,340 probes, never fires.
- **`freeride` proximity claim**: self-falsified — the car sat 4.8 s at 1.41 m
  from a `CPBox_U` with the counter unmoved while 8 ordinary gates fired in the
  same run.
- **lm 66 → 837** (flat, 4.00 m apart, clean controls): dead. The car spawns
  *on* the gate row and falls 8 m with gravity gated on throttle, so the input
  decides whether the fall starts but not where it goes; across inputs whose
  peak speeds differ 2× and which end in opposite directions the gate scores at
  **1.840 in 17 of 18 runs**. Closest approach to the partner is 3.923 m against
  4.000 m — nearest at spawn, receding after.
- **The 8.00 m-below pairs** (1561→1552, 1644→1629/1640): gate A stands on a mat
  spanning dz ∈ [−6.7, +3.1] and **every B-row gate within 10 m lies under that
  pad**. Best B-row crossing over ~70 runs: 4.624 m out, on the side with no
  gate. Minimum miss to a real gate: 2.55 m.

---

## 10. What we would do next

1. **The pad is the obstacle, not the reach.** The one geometry never tested is
   leaving the pad *sideways* — a lateral fall at dz ≈ 0. The pad's x extent is
   unmeasured; that measurement is cheap and it is the first thing to do.
2. **Aim at the 1.0 m intra-fall bundles**, not the 8 m rows. The author's six
   double-collects are ~1.0 m apart — a far shorter target, and one we have hit
   accidentally.
3. **Fix the selection criterion.** Proximity ranking has failed repeatedly.
   Candidates should be chosen from the measured reach envelope — a pair whose
   separation matches a trajectory the car can actually fly, height included.
4. **Fold the confounds into the tools**: zero-motion control and full-trace
   reads as defaults, so truncation and spawn-scoring cannot silently poison
   another batch.
5. If a continuation is ever driven, price it as
   `travel(distance, v_in) + acquisition` — **not** a flat 1.606 per gate, which
   fits the terminal block's mean and misprices everything else.

---

## 11. Method rules this map taught us

- **Before promoting a repeated negative to a property of the world, run a
  known-good artefact through the identical instrument.** Five arms blamed five
  different geometries for a one-gate ceiling that did not exist. Every failing
  tape shared one unexamined choice — forward throttle — and the project's own
  notes already said every working primitive uses reverse. A negative reproduced
  across many configurations measures the shared choice, not the world.
- **An anomaly that survives your explanations is the experiment.** "Brake is
  the only input that moves a scored car" was reported three times as a
  curiosity. Brake from standstill *is* reverse; it was the whole diagnosis.
- **A control copied into the same directory as the subject shares its defect.**
  A known-good map staged beside a failing one failed too, and that was read as
  "the machine is broken" rather than "the location is wrong".
- **When a repeated failure has a suspicious common factor, grep your own banked
  docs for that factor before designing another experiment.**
