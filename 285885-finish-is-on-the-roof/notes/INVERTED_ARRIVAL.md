# 285885 — direction 3 answered, and the first gradient anyone has found on this map

*Third agent, second working session. Read with
`att_TRIGGER_IS_BODY_POINT_v2.md` (the corrected trigger model). **This file
also corrects v2 §3** — see §6. Every number from the plain oracle on the
untouched map, or from position-only `tmmaps moveitem`/`ladder` surgery whose
return-to-origin control reproduces 61229 / 88209 / 97769 to the millisecond.*

---

## 1. What a flip is worth: 0.526 m — 7.5× the deficit

154 grounded human samples inside x ∈ [405,432], z ∈ [1695,1715], against the
fitted contact plane:

| attitude | n | origin above the plane | **tested point above the plane** |
|---|---|---|---|
| upright (`u_y` > 0.9) | 14 | 0.274 ± 0.011 | **1.098** |
| inverted (`u_y` < −0.9) | 140 | 1.363 ± 0.050 | **0.572** |

An inverted car's **origin sits 1.089 m higher** — it is resting on its roof —
but its **tested point sits 0.526 m lower**. That is a flip's true value, 7.5×
the 70 mm the fast route is short, and it is also why the old origin-based model
could never be repaired by a constant: the two effects have opposite signs and
nearly cancel. Partial tilt is what matters and it is not linear — on its wheels
the car keeps the 0.274 ride height, so the requirement is
`0.84·cos θ ≤ 0.754` ⇒ **θ ≥ 26.2°**.

## 2. What it costs: 11.2 s against a 2.042 s budget

The budget is not zero — it is `43.079 − 41.037 = 2.042 s`, the margin our
route's arrival already holds over the author time. Re-priced against that:

| conversion | measured | vs. budget |
|---|---|---|
| rank 1: flips at 39.470 at (295,122,1772), inverted in the footprint at 50.670 | **11.2 s** for 130 m | **5.5×** over |
| rank 2: flips on a wall at (405,148,1667) at 60.8, inverted in the footprint at 79.9 | 19.1 s | 9× over |
| our route, upright, over the same last 130 m | **2.6 s** | — |

The cost is not the flip. It is that **an inverted car crosses this ramp at
20–45 km/h** where an upright one does 190. So the flip must happen inside the
last ~2 s and leave the car moving.

## 3. Why it cannot: an uphill arrival can never be tilted

Four measurements, each closing one route to tilt:

1. **On the ground the car takes the surface normal.** Full lock for 0.8 s at
   190 km/h moves `u_y` from 0.974–0.983 to 0.984–0.989 — it *flattens* the car.
   Roll at ~2 g of cornering is under 1°.
2. **Suspension travel is ≤ 5 mm.** No grounded sample in any human run is ever
   more than 2/255 below the resting damper value; the full damper range is only
   59 mm of ride height anyway.
3. **The car cannot leave the surface.** The ramp's profile along the route's own
   131.7 m path has a radius of curvature of **~1500 m** ⇒ flight needs
   **440 km/h**; the route crosses at 190.
4. **Even a launched car cannot fly up this ramp.** The roof rises 0.138 m per
   metre travelled. A projectile at 51.4 m/s keeps its climb rate and then
   falls: **0.05 m under the roof after 0.1 s, 1.23 m after 0.5 s, 3.14 m after
   0.8 s.** Clearing the last 42 m would need ~3.9 m/s of extra vertical
   velocity and a 1500 m-radius plane supplies none.

And there is nothing to clip: a 100 Hz surface-residual scan along rank 2's
grounded upright pass shows the roof flat to **|d(residual)/ds| ≤ 0.026 per
metre** over x ∈ [405,428] — **no step, no lip, no panel join** (that also
answers direction 2 in the footprint's neighbourhood; the scan repeats on rank
1's and rank 3's lines, which cross at different z). The nearest tiltable ground
is a **28.3° bank at (405.1, 148.4, 1667.4)** — grounded, `u_y` 0.880, which
*would* fire the gate — but it is **39.7 m away and 4.4 m above** the footprint.

> **Direction 3's answer: an inverted arrival is not expensive on the uphill
> approach — it is unavailable at any price.** Every fire in the entire dataset,
> human or synthetic, is inverted; and no uphill approach can be. To cross this
> finish tilted, the car must arrive **descending**, from the high ground at
> x ≳ 430 / z ≲ 1670 that our route only reaches at 43.0 s.

This is consistent with the fleet's ballistic-heading law, and sharpens it here:
the law says attitude is free in the air and heading is not, so the arrival point
must be bought before takeoff. **On this approach there is no takeoff to buy it
before.**

## 4. The lever that IS on the table

Because the trigger's ceiling and the roof are two differently-tilted planes,
**position buys clearance with the car flat.** A 441-station 2-D gate offset
sweep at 0.25 m resolution (`att_grid2.log`):

| gate offset | threshold gate y | ⇒ car moved |
|---|---|---|
| (0, 0) | 144.10 | — |
| (−0.50, +0.50) | 144.00 | (+0.50, −0.50) |
| (−1.50, +1.00) | **143.90** | (+1.50, −1.00) |

**0.111 m of clearance per metre of car movement toward (+x, −z)** ⇒ **0.44 m of
displacement wins.** This independently reproduces the previous agents' trade
curve ("0.42 m along the (+x,−z) diagonal") from a different instrument — the
first thing on this map two independent measurements agree on.

## 5. The DIAGONAL LADDER, and the first rung gain on this map

> Thirteen map copies with the Goal displaced along (−x,+z) by 0, 0.06, 0.12,
> 0.18, 0.24, 0.28, 0.32, 0.34, 0.36, 0.38, 0.40, 0.44, 0.50 m — **all at the
> real gate height 144.0, so every rung is the real trigger and rung 0 IS the
> untouched map.** Rung 0's control returns 61229 / 88209 / 97769 exactly. A
> rung-0 fire is not a proxy for a finish; it is a finish.

This is a strictly better instrument than any height ladder used here, because
the quantity it ladders is the one with a measured gradient.

Baseline: `bis_418.6138_best` at **rung 11 (0.44 m)**, `seedF`/`seedG` at 0.50,
`lat_418.2_best` at 1.10.

**Result: four independent hill-climb streams — plain rectangular overrides,
fine low-magnitude overrides, and lane-change pulse pairs, over windows from
slot 3200 to 4258 — all reached rung 10 (0.40 m) in their first round and every
one of them stalled there.** Winners banked as
`att_winners/att_diag_rung10_{g1..g4,h3}.Ghost.Gbx`; all still DNF on the
untouched map, `cps=0`, with the WR returning 61229 in the same batch.

| | displacement needed | clearance deficit |
|---|---|---|
| the route as inherited | 0.44 m | 49 mm |
| after the climb | **0.40 m** | **44 mm** |

**Then a wall.** Four cross-seeded continuations (each starting from another
stream's winner, each sweeping a different window) all returned `IDENT` in round
1. **171 704 ladder evaluations in this phase**, on top of ~78 000 earlier and
the previous agents' ~1.6 M.

So the landscape is now properly characterised rather than merely negative: it
has **exactly one step of gradient, worth 5 mm of the 70, and it is flat on both
sides of it.** That is a much stronger statement than "no gradient", and it was
only visible because the ladder finally measured a quantity that moves.

## 6. Correction to `att_TRIGGER_IS_BODY_POINT_v2.md` §3

v2 §3 claimed the ceiling tilts so that clearance improves toward **−x**, with
an interior minimum. **Both wrong.** They came from fitting a ceiling slope to a
gate-*x* sweep in which the tape re-enters the displaced volume at a different
point every time, so the fit measured the tape's own path rather than the
ceiling. §4's 2-D offset sweep is the honest measurement and gives the opposite
sign. Everything else in v2 stands: the body-point trigger model, the 0.84 m
offset, the contact plane, and every negative.

That is the second phantom this map has produced from reading a gate sweep as if
the car's path through the volume were fixed. **A gate displacement moves where
the car ENTERS the volume as well as where the volume is. Only a displacement
sweep that reports the THRESHOLD — the lowest gate height that still fires — is
safe to interpret.**

## 7. Oracle throughput on this map: 4.5×, with controls

Applying the fleet throughput addendum here:

* **Batching was my own defect** — my evaluator ran one candidate per server
  call. 401 candidates on 60 workers: **14.36 s → 5.17 s (batch 4) → 3.44 s
  (batch 12)**; batch 40 is worse (6.34 s), so the optimum is around 12.
* **The declared-race-time prune works and is worth having for the constraint,
  not the speed.** This map's tapes declare 61229, not a multi-hour recording,
  so re-declaring at the 43.079 budget only buys ~8% (3.44 → 3.19 s) — but it
  makes *"slower than the author time"* return DNF for free, which is exactly
  the time constraint a shaping ladder needs, with no scorer changes.
  Three controls, all passed:
  1. `seedY` re-declared at 55.000 still returns **50229**, to the millisecond;
  2. `seedY` re-declared at 43.079 returns **DNF** (its 50.229 is over budget);
  3. the fast route re-declared at 43.079 gives an **identical** 13-rung ladder
     profile to the original.
  Tool: `attdecl IN OUT NEW_MS` (rewrites every u32 in the body equal to the old
  declared value; 6 occurrences here).

## 8. Files

`att_grid2.log` (441-station 2-D offset sweep) · `att_diag_ladder.txt` (the
thirteen rung positions and rung 0's control) · `att_winners/` (the five
rung-10 tapes) · `att_ident_r2.csv` (rank 2's live 100 Hz trajectory, used for
the surface scan) · `att_tools.tgz` (adds `attdecl.rs`, `inv.rs`, `cmp2.rs`,
`step.rs`, `tiltmap.rs`, `ballist.rs`, `grid2.rs`, `thr2.rs`, `dclimb.sh`,
`killmine.sh`).

**`killmine.sh` is in there for a reason:** `pkill -f` on a pattern that also
appears in the calling shell's own command line killed this session's shell
**three times** tonight, including once when the pattern was written with the
usual `dc[l]imb` self-exclusion trick — because the *parent* `bash -c` still
carried the literal text. Match on `ps -eo comm` (the executable name), never on
args.
