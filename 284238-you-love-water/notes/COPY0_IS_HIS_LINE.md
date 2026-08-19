# 284238 — the positive control for R4, and it changes the target

`state_ADDENDUM_v7_copy0_is_his_line.md`. Sidecar to
`state_ADDENDUM_v6_rung_ladder_on_his_line.md`; supersedes nothing. Times in
seconds. Plain-oracle validation on the untouched map for every control
(record **440.238**, `best_97325` **97.325**, Yhomas on his own map **46.112**
— all exact this session).

**A `cps` number from a rung map is not a time. There are no milliseconds in
this file that mean anything about the map's finish.**

---

## The question this answers

`state_ADDENDUM_v6` built a ladder on Yhomas's line inside **copy 1** and found
R2 fires, **R3/R4/R5 do not**, R6/R7 fire. R4 — his wall contact — is the
load-bearing rung, and a rung no tape has ever fired cannot be distinguished
from a rung that *cannot* be fired. So: get a positive control for R4 by other
means before believing the climb is blocked.

The other means is copy 0. The four copies are exact images under the screw, so
**his canonical line IS a copy-0 world line**, and our standing start — the one
launch on this map already inside the working band — drives copy 0.

## The control: R4 fires, and so does most of his launch

Same seven canonical points, sited in **copy 0** instead of copy 1:

| rung | copy 1 (v6) | **copy 0** |
|---|---|---|
| R1 kicker | — | — |
| R2 kick exit | fires | **fires** |
| R3 mid-flight | — | **fires** |
| **R4 wall contact** | — | **fires** |
| R5 up the curve | — | — |
| R6 curve exit | fires | — |
| R7 his CP crossing | fires | — |

**R4 can be fired, and our own record fires it.** The dead tape fires nothing on
any of the fourteen maps. So the copy-1 R3/R4/R5 silence is a physical statement
about copies 1–3, not an instrument failure — which is what the control was for.

## And the trajectory read says more than the ladder does

`btraj2` of the record on the C0R4 map, measured per tick, distance from each of
his canonical points during the standing-start launch:

| his point | our closest approach | when |
|---|---|---|
| R1 kicker | **2.78 m** | 4.210 |
| R2 kick exit | **2.20 m** | 4.540 |
| R3 mid-flight | **4.06 m** | 4.880 |
| **R4 wall contact** | **5.93 m** | 5.220 |
| R5 up the curve | 6.78 m | 5.580 |
| R6 curve exit | 8.86 m | 6.090 |
| R7 his CP crossing | 12.57 m | 6.820 |

**Our standing start flies Yhomas's launch to within 2–7 metres, point for
point, in order.** It is not a similar launch; it is the same launch, drifting
apart only as it climbs the wall curve. Against 9.5 m of lateral error at the
wall in cycle 1, and 82–135 m of checkpoint miss for every steered variant, this
is the closest anything on this map has ever been to the target line.

**Caveat, honestly:** a relocated checkpoint proves its cell's **earliest**
visit, and on this map the record respawns 31 times — and moving CP2 moves the
respawn point, so the record's trajectory on a rung map diverges from the real
one after its first respawn (measured: median 602 m over the whole run). Every
number above is taken **before 11.000**, inside the standing-start launch and
before any respawn, where the rung map and the real map are the same run. The
firing rungs (R2/R3/R4 at 4.5–5.2) are all inside that window.

## What this does to the problem

The target launch is no longer only on another map, driven by another player,
with a different launcher. **It is on our map, in our car, in the record we
already have, at 4.2–5.2 seconds.** The remaining question is not "what does the
target state look like" — it is:

> **Copy 0 enters its launch from a standing acceleration across the deck;
> copies 1–3 enter from the tube and the arc. Reproduce copy 0's LANE-ENTRY
> STATE at the start of copies 1–3.**

And the earlier measurements now read as one coherent story:

* copy 0 crosses the kicker at vz **−18.8** and contacts the wall at z **915.4**
  (`state_ADDENDUM_v3` §4) — inside the working band, 1.5 m from Yhomas's 913.9;
* copy 1 crosses at vz **−1.9** and contacts at z **923.4** — 9.5 m out;
* the arc cannot fix it: the trilemma (`v4`) and the two-channel search (`v5`,
  6500 evaluations, z_peak tops out at 922) both say the CP2-collecting basin
  does not contain the target;
* and the ladder now says the target *is* reachable on this map, just not from
  the tube.

## The next lever, and it is a different kind of question

Everything upstream of the lane has been searched as *driving*. The copy-0 entry
is not driving — it is a **standing start**, i.e. a zero-velocity state on the
deck. On this map a respawn returns the car to its last checkpoint crossing
state (`RESULT-v2` §4), and the record contains 31 of them. The question worth
one hour is therefore:

> **Can a cycle be made to enter its lane from a copy-0-shaped state at all —
> by respawn, by a slower and lower arrival, or by any state the previous cycle
> can be made to hand over — and what does it cost in time?**

That is measurable with the instruments now in place: `fk btraj2` reads the
handover state per tick, the copy-0 ladder gives a pass/fail for "is this launch
on his line", and the plain oracle adjudicates the cost. It is also the first
lever on this map that does not assume the four copies must be driven the same
way — and `state_ADDENDUM_v4` §4 established that they cannot be entered
differently, which is exactly why the *state* at the entry is the only remaining
degree of freedom.

## Enumeration

* 7 canonical points × 2 copies = 14 rung maps, each validated against 3 tapes
  (record, `best_97325`, a dead tape) = 42 validations, all on position-only
  rungs with the checkpoint model untouched.
* R4 in copy 1 additionally swept over all six 60° yaw headings × 2 tapes: 12
  validations, all `cps=1`.
* One per-tick trajectory read (44 257 ticks) for the copy-0 distances.
