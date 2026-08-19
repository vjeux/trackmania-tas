# 285885 — the height ladder moves the 70 mm wall (interim, run B)

*Sidecar v1 from the objective agent (`bis4211`). Companion to
`bis4211_OBJECTIVE_v1.md`, which is the measurement that justified building
this. Everything of mine is `bis4211_`-prefixed; I have modified none of the
two previous agents' files. A continuation arm was running when this was
written — if a `_v2` exists, it supersedes this file.*

**Headline: the 70 mm clearance is NOT locked. It was locked against a 10 mm
rung. With 1–2 mm rungs the same class of search moves it, and moved it 6 mm —
144.070 → 144.064 — in 35 712 evaluations, on a number two agents established
as immovable across ~57 000.** The author time is still not beaten: 64 mm
remain, and the map's own verdict is unchanged until a tape fires the untouched
gate.

## 1. What was run

| | |
|---|---|
| objective | height ladder (`bis4211 lsearch`), score = the lowest of 20 gates the tape still fires, tie-broken by fire time |
| seed | `bis4211_seed_144069.Ghost.Gbx` (clearance 144.069, itself found by the §4 perturbation sweep in `bis4211_OBJECTIVE_v1.md`) |
| mutation window | slots 3600–4270 = race 34.5–41.2 s, the whole roof climb |
| population | 230 exploiters at T=350, 154 explorers at T=2500 (one rung = 1000 score units) |
| arrival constraint | reject any candidate entering the footprint after 42.000 s |
| run | 35 712 evaluations in 6 034 s (100 min), 5.9 eval/s, 168 concurrent servers |

## 2. The result: the wall is a gradient

| clearance reached | gate y | evaluations to reach it | fire time |
|---|---|---|---|
| the two previous agents' wall | 144.070 | — (0 finishers in ~57 000) | 41.074 |
| perturbation sweep (§4 of the objective sidecar) | 144.069 | 147 tapes | 41.084 |
| ladder rung 1 | **144.068** | 384 | 41.084 |
| ladder rung 2 | **144.067** | 2 304 | 41.059 |
| ladder rung 3 | **144.066** | 5 376 | 41.059 |
| ladder rung 4 | **144.064** | 12 672 | 41.069 |
| — | 144.063 | not reached in 35 712 | — |

**6 mm, and the depth was free in time**: the deepest tape fires at 41.069,
*earlier* than the incumbent's 41.074. Cost per rung roughly doubles each time
(384 → 1 920 → 3 072 → 7 296 evaluations for successive millimetres), which is
the number whoever continues should plan against, not the 6 mm itself.

**Why the previous ladders returned zero, restated as a fact about the
instrument rather than the map:** they asked for a 10 mm step in one rung
(144.070 → 144.060) and for a 50 mm lateral step. Both are ~5–25 rungs of the
gradient that actually exists. A negative from a rung the population cannot
reach in one mutation says nothing about the rungs in between. This is
ACQUISITION §0.4's "suspect the enumeration before the hypothesis", one level
down: the enumeration here was the rung spacing.

The explorer island earned its keep: the tier-9 state that unlocked 144.067 came
from an explorer migrating into the exploiter population
(`MIGRATION: explorer 82 (tier 9, value 9470.5) beats the exploiters' best`).

## 3. Controls — all of them, and what each one had to say

| # | control | result |
|---|---|---|
| 1 | **return-to-origin on every gate** | printed and enforced on every scan and every search (`control OK: rebuilt-at-origin reproduces the untouched map`); the run aborts exit 9 otherwise. Position-only `moveitem` surgery on an item that is **already the Goal gate**, so no model swap and no promotion — the failure mode that returned 50589-for-61229 cannot arise |
| 2 | **known answer** | the incumbent `bis_418.6138_best` scores clearance 144.0700 / dwell 37 ms, both matching the banked hand measurements |
| 3 | **negative control** | 79 of 147 perturbations never reach the route: **NO SCORE**, not a small plausible number. In the search, 10 364 of 35 712 evaluations scored nothing at all |
| 4 | **the crawl trap, fired deliberately** | with the cap at 42.000 s the human WR, rank 2 and `seedY` are all rejected. With the cap raised to 60.000 s, `seedY` scores **5499** against the route's **2499** — i.e. without the constraint the search abandons a 41 s route for a 50 s crawl in its first generation. The constraint is not decorative; it is the difference between the two attacks |
| 5 | **identity** | the seed re-encoded through the tape factory scores exactly what the original scores (4463.0 = 4463.0); printed at the start of every search |
| 6 | **can the ladder say DEEPER?** | with the rungs shifted up, the incumbent descends all six and reports the deepest tier — and `seedY`, which really does finish the untouched map at 50.229, reports the **winning** tier. The top of the ladder is verified against a tape that genuinely finishes |
| 7 | **dead-window control** | the same search over slots 0–140 (before the race starts): **896/896 evaluations return the seed's exact score, zero "new best" events**. The objective invents no gradient where there is none, and the pipeline is deterministic |
| 8 | **independent instrument** | the other agent's `tmmaps ladder` binary, run on the deepest tape: fires 144.070/144.068/144.066 at 41.059, refuses 144.065. Same answer, different code path |
| 9 | **cross-node** | rebuilt from the tarballs on a second machine: field 3/3 exact, `seedY` 50.229, and the tapes report the same clearances |

## 4. On the shaping notice, explicitly

`FLEET_NOTICE_reward_shaping_is_inert_v1.md` proves shaping is inert when a
*finishing* incumbent makes every DNF strictly worse. **This search is the
exception it names, and it is structured accordingly rather than bolted on:**

* the incumbent DNFs the untouched map, so every comparison in the population is
  DNF-vs-DNF;
* the ladder is therefore **the** objective for this population, not a bonus
  term — no score in this search is built from `FINISH_BASE` at all, so there is
  no `1e8 − t` for it to be eaten by;
* the ordering is lexicographic by construction: tier first, fire time clamped
  to half a rung so it can never trade against a rung. The deepest tier **is**
  the untouched map, so a finisher still dominates every non-finisher — which is
  correct here, since a finisher is the result;
* the temperatures were chosen against the score's own units: one rung = 1000,
  exploiters at T=350 (one rung down = 5.7 % acceptance), explorers at T=2500
  (67 %). The notice's `exp(−2.4e6)` failure cannot occur because the dynamic
  range of the score is 1000 per rung, not 6e7.

## 5. On the gate-ladder notice (`FLEET_NOTICE_gate_ladder_three_repairs_v1.md`)

Read mid-run; it changes nothing here, and the reasons are worth recording:

* **Plane orientation.** That notice is about *block* gates relocated by cell
  bytes, where a `dir` byte picks an x- or z-plane. These rungs are the map's
  own **item** gate (`GateFinishCenter8mv2`), moved by overwriting its three
  position floats only — model, cell and yaw untouched. More to the point, **every
  rung is the same volume translated along one axis**: whatever its shape and
  orientation, only the height differs between rungs, which is exactly what makes
  the comparison valid. The vertical sensitivity itself is established
  bidirectionally in the banked work (lower the gate and every tape DNFs; raise
  it and every tape fires, saturating), so a y-insensitive plane is excluded by
  measurement.
* **Wide rung = decoy generator.** The footprint rungs sit at the real gate's
  x/z, so a fire there is the real trigger at a raised ceiling — there is nothing
  to be a decoy of. The **approach stations** (used only to grade candidates that
  never reach the footprint) are relocated 8 m gates and *are* exposed to that
  hazard; they are capped at tiers 0–1, can never produce a win, and no reported
  depth depends on them.
* **Already-Goal gate.** Item 0 is already the finish, so the ladder is
  position-only on the untouched map, as that notice recommends.
* **`tmtas splits` reads the header.** Not used. No claim here comes from
  decoding a synthesised tape; every number is a fire time from the plain
  oracle.

## 6. Where this leaves the map

Unchanged as a verdict, changed as a prognosis:

* best validated time on the untouched map is still **50.229** (`seedY`, the
  route agent's), against an author time of **43.079**;
* the fast route still DNFs the untouched map — `bis4211_ladder_144064` returns
  `DNF cps=1`, as it must at 64 mm of remaining clearance;
* but "the crossing geometry is locked" should be retired. It is not locked; it
  is expensive, at a cost per millimetre that roughly doubles as you go down.

**The honest extrapolation is bad news and should be stated as such:** at the
observed doubling, the next 6 mm costs ~10⁵–10⁶ evaluations and 64 mm is out of
reach by this route. The value of this arm is that it converts a wall into a
measured cost curve, and it removes the reason to keep grinding local mutations
at 10 mm granularity. If the map falls, it will be to a lever that buys tens of
millimetres at once — the other arm's attitude/tilt attack is exactly that shape
— and this ladder is the instrument that will grade it, at 1 mm, with the
controls above.

## Files

| file | what |
|---|---|
| `bis4211_bin_main.rs` | the instrument: `yscan`, `score`, `lsearch`, `perturb`, `info` |
| `bis4211_seed_144069.Ghost.Gbx` | clearance 144.069 (accel cut at slot 4220) |
| `bis4211_ladder_144066.Ghost.Gbx` | clearance 144.066, fires 41.059 |
| `bis4211_ladder_144064.Ghost.Gbx` | **clearance 144.064, fires 41.069** — the deepest crossing measured on this map |
| `bis4211_runB_progress.log` | the run's own log, including every control line |
| `bis4211_runB_log.jsonl.gz` | per-candidate tier/t_in/t_fire/accept, 35 712 rows |
| `bis4211_yscan_*.csv` | the dwell-vs-clearance measurement behind the objective |
