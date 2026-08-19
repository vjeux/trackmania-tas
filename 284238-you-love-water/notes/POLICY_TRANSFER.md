# 284238 — closed-loop POLICY transfer from the sibling map: what it bought,
# what it cost, and the one block that is worth 1.30 s

`poltx_RESULT_v2_closed_loop_policy_transfer.md`. Write-once sidecar, `poltx_`
prefix (third arm). **Supersedes `poltx_RESULT_v1_launcher_price_and_policy_transfer.md`
(md5 `4ff5b8c31caa12e49522c46c9f196606`)**, which was an interim bank written
mid-run: v1 is correct on everything it states, and v2 adds the negatives, the
enumerations, the artefacts and the next lever. Times in seconds. Nothing was
submitted to any leaderboard.

---

## 0. Headline

1. **Yhomas_TM's 46.112 re-simulates EXACTLY on 279008** — my brief's
   deliverable (a), confirmed independently on my own build.
2. A closed-loop controller fitted on his run **transfers to our copy 0 to
   sub-metre accuracy** (median lateral error **0.02 m**, median 3-D distance
   **0.92 m** over the whole segment) and crosses CP1 at **64.4 m/s at 6.520**
   against our record's **52.8 m/s at 6.797** — the first time anything on this
   map has crossed a checkpoint at the sibling human's speed.
3. Holding the LINE fixed and varying the MAP prices the launcher substitution:
   **1.30 s, all of it paid in the first 1.4 s of the race, on the water start
   block.** After that our car matches his acceleration exactly.
4. **46.112 + 1.30 = 47.4 against an author time of 50.459.** The author time is
   **3.0 s slower** than the sibling human's line carried onto our map. That is
   the strongest evidence this map has produced that 50.459 is driven and that
   the route is essentially Yhomas's.
5. The transfer then **fails after CP1**, and the failure is localised: a
   sub-metre, 1–2.5 m/s residual at take-off is amplified by the ballistic
   segment into 7–13 m by the chute entry, where there is no authority to
   correct it. **The same law also fails on HIS OWN MAP at about the same
   place** (ceiling phase 1878 of 4599 there, 1013 here), so the cycle-1 failure
   is a limit of THIS LAW, not evidence that the transfer is impossible.

## 1. Controls — including the ones designed to fail

Plain oracle, untouched maps, everything read **from this store, nothing from
`/tmp`** (transcript: `poltx_artifacts/poltx_revalidation_v1.txt`):

```
279008  cold_279008_yhomas_46112  ->  46112   exact     <- deliverable (a)
279008  cold_279008_ashura_92018  ->  92018   exact
284238  rank00001_440238          -> 440238   exact
284238  best_97325                ->  97325   exact
284238  poltx_ctl_roundtrip_440238-> 440238   exact     (tape rewritten, 0 edits)
284238  poltx_ctl_roundtrip_97325 ->  97325   exact     (and BYTE-IDENTICAL)
284238  poltx_best_norespawn_canvas-> DNF cps=0         (respawns removed: expected)
```

Per-tick reader against each ghost's own telemetry: Yhomas **median 0.0122 m,
94.90 % within 5 cm**; our record **median 0.0075 m, 91.10 % within 5 cm** — the
second reproduces the other arm's published number on an independent build.

**Instrument controls, two-sided:**

| control | expected | measured |
|---|---|---|
| law with `ff=1`, all feedback zero, on HIS map with HIS reference | reproduces his run | CP1 phase at **5.210** (his 5.240), **66.0 m/s** (his 65.4) |
| law with `ff=0` (steer 0 everywhere) | must die | dies at phase 325/519, car stationary |
| respawn stripper with zero edits | must reproduce the map's own times | 440.238 and 97.325 exact; the 97.325 file is byte-identical |
| respawn detector vs. respawn packets | must agree | 31 position jumps ↔ 31 state-literal bit-31 packets, same ticks |

## 2. The instrument: closing the loop through an open-loop oracle

`fk pol` (new `fk` subcommand, Rust; banked in `poltx_tools_v1.tgz`, md5
`67f3bc0a8ed8219b9e301ea8ca32a5db`).

The oracle replays a whole tape, so the loop is closed by **iteration, and the
iteration is exact**:

> `u_{i+1}(t) = K(x_i(t))` for all `t ≥ locked`. At the first tick `d` where the
> new action differs from the old, the prefix is already a fixed point
> (identical prefix ⇒ identical states ⇒ identical actions), so `d` never
> decreases and strictly increases each iteration. The loop **terminates** on a
> tape satisfying `u(t) = K(x(t))` at every tick — the exact closed-loop
> trajectory of the law, produced by the real engine.

No relocated gate, no promoted gate, no shaped reward, no surrogate: the only
oracle is the engine. Every state the law sees is read out of the engine's own
vehicle struct via the fixed clock-first locate, never off a ghost file. A
5.5 s segment rollout costs **0.1–1.0 s**; a whole lap 1–20 s. Typical
convergence 6–50 iterations.

**The law** (7 gains, all searchable):

```
ground: steer = ff·steer_ref(phase) + klat·e_lat + khead·e_head
                                    + kvlat·e_vlat + kvert·e_vert
air:    steer = ff·steer_ref(phase) + kair·e_head
gas/brake = the reference's own, overridden by speed-error thresholds
```

`phase` is the nearest point on the reference **path**, not the matched time —
that is the whole difference between a policy and a tape, and it is why the
feedforward re-aims when the car is early or late. It also means the error
signal is a `z(x)`-style match, not `z(t)`, which is what the other arm warned
was required.

Two design choices came straight off fleet notices and both were load-bearing:

* **Feedback is gated OFF while the reference is airborne**, and the phase index
  there advances one per tick instead of re-matching by position
  (`FLEET_NOTICE_ballistic_heading_law_v1`: in flight the steer axis rotates the
  chassis and cannot move the velocity, so a position gain fits noise and a
  position match stalls or jumps on a metre of ballistic error).
* **`fk pol strip`**, below, because `ghost::Factory` is blind to respawns.

## 3. NEW CAPABILITY: `fk pol strip` — a respawn-free canvas

A respawn is bit 31 of the 33/34-bit state literal and `ghost::Factory` indexes
only steer/accel/brake, so **a controller that writes those three channels
cannot remove a retry, and every rollout is capped at the template's first
respawn.** This cost me two dead experiments before I found it: the record
retries at race 11.040 and *every* variant reported the identical `t 10.800`.
That is the "byte-identical times across plainly different candidates"
signature 197047 documented, in a new costume.

`fk pol strip --template T --out F` clears them (`--dry` counts only).
Round-trip control with zero edits reproduces 440.238 and 97.325 exactly.
Counts: our record **31**, `best_97325` **4**.

**Fleet-relevant:** any search on this map seeded from either tape is searching
under a frozen retry schedule it cannot see.

## 4. The measurement: same policy, same line, two maps

Copy 0 of 284238 and of 279008 differ by **exactly one block record** —
`PlatformWaterStart` vs `PlatformTechStart` at (776, 1872, 943) — and by no
items: all six extra `GateSpecial32mTurbo2` pads are in copies 1–3 (y = 1824 /
1768 / 1712), verified with `tmmaps freeblocks` and `allitems` plus `comm` on
both maps. Copy 0 is therefore a like-for-like A/B.

`dt` = our race time minus his **at the same phase**:

| phase | ours t / v | theirs t / v | dt | dv |
|---|---|---|---|---|
| 0 | 0.25 / 1.9 | 0.22 / 3.5 | +0.03 | −1.5 |
| 100 | 1.79 / 12.5 | 1.22 / 19.3 | **+0.57** | −6.7 |
| 120 | 2.25 / **7.8** | 1.42 / 28.0 | **+0.83** | −20.2 |
| 140 | 2.78 / 30.7 | 1.62 / 40.2 | **+1.16** | −9.5 |
| 180 | 3.26 / 57.3 | 2.02 / 61.8 | **+1.24** | −4.5 |
| 240 | 3.89 / 82.1 | 2.62 / 85.4 | +1.27 | −3.3 |
| 300 | 4.51 / 85.5 | 3.22 / 87.8 | +1.29 | −2.4 |
| 400 | 5.52 / 69.9 | 4.21 / 70.7 | +1.31 | −0.8 |
| 460 | 6.07 / 66.8 | 4.80 / 66.0 | +1.27 | **+0.9** |
| 502 (CP1) | 6.49 / 55.9 | 5.19 / 66.0 | +1.27 | −9.8 |

(that last row is the ff-only run; with feedback CP1 is crossed at **64.4 m/s**,
§5.)

**Read the `dt` column.** The deficit is created in the first 1.4 s and never
grows again: from phase 180 to 460 — the lane, the kicker, the entire ballistic
flight — it is flat at +1.24…+1.31 while `dv` closes from −4.5 to **+0.9**.

The mechanism is the phase-120 row: at x ≈ 808 our speed **falls** 12.5 → 7.8
while his goes 19.3 → 28.0. Standing acceleration off the block: ours
0 → 12.5 m/s in 1.54 s (8.1 m/s²), his 0 → 19.25 in 1.00 s (19.3 m/s²), a factor
of **2.4**. A water start does not grip from a standstill.

> **The launcher substitution is worth 1.30 s, paid once, at the line.**

## 5. What the feedback bought, and where it stops

Gains are `ff,klat,khead,kvlat,kvert,vlift,vbrake,kair`. On our map, from the
race start, template `poltx_best_norespawn_canvas`:

| gains | CP1 t / v | median \|e_lat\| |
|---|---|---|
| our record (no controller) | 6.797 / **52.8** | — |
| `1,0,0,0,0` (feedforward only) | 6.490 / 55.9 | 0.43 m |
| `1,-0.02,-0.25,0,0` | 6.520 / **64.4** | ~0.02 m |
| `1,-0.005,-0.5,0,0` | 6.510 / **64.4** | 0.03 m |
| `1,-0.02,-0.5,-0.005,0` | 6.570 / 62.9 | **0.02 m** (d median 0.92 m) |
| Yhomas, on his map | 5.240 / 65.4 | — |

**The sign matters and it is not the obvious one**: positive `klat` (steer
*toward* the reference in my frame convention) makes the error grow; the working
sign is negative. Worth stating because the first grid I ran was the wrong half
of the axis and read like a dead lever.

Then it fails. Per tick, from the best copy-0 run:

| phase | what | distance from his line |
|---|---|---|
| 502 (CP1) | on the ground | **0.88 m** |
| 517 | last ground contact | **0.32 m** |
| 600 | in flight | 6.9 m (all vertical) |
| 700 | in flight | 12.8 m |
| 713 → 734 | chute entry | lateral error −6 → −35 m in 0.5 s; car falls past the catch |

We leave the ground **0.32 m and ~2 m/s** from his state, and 2 s of ballistic
flight turns that into 13 m — which is one chute width. **In the air there is no
authority**, so the residual has to be nulled before take-off, and the law has
no terminal objective, only a tracking one.

## 6. Negatives, with their enumerations

**(a) The standing start cannot be driven better — 32 variants, exhaustive over
that family.** Forced input over the water start (`--pre`), then the law:
window ends 220 / 260 / 300 / 340 ticks × steer 0, ±0.25, ±0.5, +1.0 with full
gas (24), plus gas+brake and lift at each of the 4 window ends (8). **Nothing
beats straight and full gas.** Two variants shave 0.03 s to phase 180 and then
die before CP1. So the 1.30 s is a property of the surface, not of the driving,
over this family — and no other family was tried.

**(b) The law cannot hold a lap, on either map.** Ceiling in phases of 4599:

```
his map 279008 : 1878   (through CP2 at 1627, into cycle 2)   36-vector grid
our map 284238 : 1013   (dies in cycle 1's fall)              36-vector grid
```

Grid: `klat ∈ {−0.03,−0.05,−0.1}` × `khead ∈ {−0.5,−1,−2}` ×
`kvlat ∈ {−0.06,−0.1,−0.15,−0.25}`, plus an earlier
`klat ∈ {−0.05,−0.1,−0.2,−0.35}` × `khead ∈ {−1,−2,−4}` ×
`kvlat ∈ {−0.01,−0.03,−0.06}`; and before those, ~200 vectors over
`klat ∈ [−0.04, +0.05]`, `khead ∈ [−1.5, +3]`, `kvlat`, `kvert`. Exhaustive over
those ranges at that spacing and **over nothing else** — in particular no
per-phase gain scheduling and no throttle feedback channel were tried, and both
are named below as the next thing.

**(c) The lateral-velocity damping term is what extends the reach** — `kvlat`
−0.06 took his-map reach from 1418 to 1878 — but on our map the same term costs
CP1 speed (64.4 → 57). The law wants different gains in different phases, which
is exactly what it does not have.

## 7. What I would do next, in order — and why it is not "bounded negative"

The lever's kill criterion was *"if the controller cannot hold the line on our
map, measure why"*. It held the line to **0.02 m** where it has authority, and
lost it where it has none. The mechanism is named, and it points at two distinct
next levers, both cheap:

1. **Stop tracking; SHOOT.** The launch is a two-point boundary-value problem:
   hit a target STATE (position + velocity, 6 numbers) at the last grounded tick
   before the gap. The ~50 ground ticks before take-off are the control, each
   fork is ~1 ms, and the rollout machinery already exists — what is missing is
   a terminal objective, not a better tracker. A tracking law spends its
   authority reducing error it already has; a shooting method spends it on the
   only error that survives the flight.
2. **The water start is the largest single measured loss on this map (1.30 s)
   and §6(a) only closes the simplest family.** A standing start has **no
   upstream coupling at all** — 150 ticks, one fork each. What has not been
   tried: any *time-varying* input (the sweep above is piecewise-constant),
   respawn-assisted starts, and whether the six boost pads in copies 1–3 have a
   copy-0 analogue that the route can reach.
3. **A throttle channel as a first-class control through the arc**, per the other
   arm's trilemma result — my law has gas as feedforward-plus-threshold, which is
   the "mode, not channel" shape they warned against.

And the strategic point, which I think is the most useful thing in this file:
**the author time is 3.0 s slower than the sibling human's line carried onto our
map, with the launcher penalty already paid.** Whatever 50.459 is, it is not at
the edge of what this geometry allows.

## 8. Artefacts — everything needed to reproduce every number here

```
poltx_tools_v1.tgz                   md5 67f3bc0a8ed8219b9e301ea8ca32a5db
    poltx_src/pol.rs, fk_main.rs     the controller (Rust, fk subcommand)
    poltx_src/README.md              how to build and drive it
    poltx_src/ref_{full,c1,start}.tsv  the reference tables (Yhomas, world coords)
poltx_artifacts/
    poltx_SHA256SUMS_v1.txt          map + control ghosts + tapes + tools
    poltx_revalidation_v1.txt        store-only re-validation transcript
    poltx_ctl_roundtrip_440238.Ghost.Gbx    stripper control -> 440.238 exact
    poltx_ctl_roundtrip_97325.Ghost.Gbx     stripper control -> 97.325, byte-identical
    poltx_best_norespawn_canvas.Ghost.Gbx   respawn-free template (DNF by design)
    poltx_seed_copy0_yhomas_line.Ghost.Gbx  the copy-0 policy tape (DNF: it is a
                                            SEED, not a result -- it drives copy 0
                                            on his line and then falls)
    poltx_seed_copy0_pertick.csv     that run's per-tick state + errors + inputs
```

Maps and human ghosts are the ones already in this directory
(`map.Map.Gbx`, `ghosts/`, `cold_siblings/`); their sha256 are in the sums file.
Reproduce with two commands:

```
tar xzf poltx_tools_v1.tgz          # then build fk per poltx_src/README.md
fk pol run --template <canvas> --map map.Map.Gbx --tick 60 \
          --ref ref_full.tsv --dk 0 --ticks 700 --phmarks 502 \
          --gains 1,-0.02,-0.25,0,0,1e9,1e9,0
```

## 9. Method notes for the fleet

**A policy is a control variable that a tape cannot be.** Two open-loop tapes on
two maps differ in the line *and* in the map, and every difference downstream is
confounded. Fixing the LINE with a feedback law and varying only the map is what
turned "the launchers are different" into "1.30 s, here, by this mechanism".
That construction is reusable on any pair of maps sharing geometry, and this
project has at least five such pairs.

**A closed loop can be run through an open-loop oracle exactly, not
approximately.** The prefix-lock argument in §2 costs nothing to implement,
terminates by construction, and the tape it produces is an ordinary tape the
plain oracle can validate. There is no phantom class here: the only oracle is
the engine.

**And the failure that cost me the most was an input I could not see.** Not a
bad hypothesis — a frozen respawn schedule that made 60 different candidates
report the same time. `ghost::Factory` is blind to it, so every search this
project runs inherits its seed's retries. `fk pol strip` is banked.
