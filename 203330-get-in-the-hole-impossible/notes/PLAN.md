# Map 203330 — "Get in the Hole ( Impossible )" — plan, argued from measurement

uid `RL64wn0vFhuqHfKGLnMOql2SMaj` · Nadeo mapId `bdf00e2a-5e09-4522-b3e9-e125a4cd0e32`
AT **13995** · online WR **14018** (in-.-, set 2026-08-11) · 5 records · gap 23 ms
Author / AT setter: **EvenOliveTM.exe** (Hannover), uploaded 2024-10-17.

Everything below was measured on 65139.od.fbinfra.net on 2026-08-18, not assumed.

---

## 1. Acquisition and the identity control (PASSED)

Followed `ACQUISITION.md` exactly (fwdproxy, descriptive UA, 1.8 s between ghost
downloads). Two trackmania.io calls + one Nadeo file download + 5 ghost
downloads. No auth needed.

    tmtas validate --map /tmp/m203330/map.Map.Gbx --jobs 5 /tmp/m203330/ghosts/*.Ghost.Gbx
    r01_14018 -> 14018   r02_14031 -> 14031   r03_15478 -> 15478
    r04_21230 -> 21230   r05_23153 -> 23153

All five re-simulate to their exact recorded millisecond. Map loads, ghosts
decode, oracle agrees with the online board. The candidate **factory** also
round-trips: `tmsearch --verify` of r01 re-validates to 14018.

`tmtas splits`: **the map has NO intermediate checkpoints** — one waypoint
(`Goal`). So there is no checkpoint decomposition and no segment-map reward
shaping available for free; a DNF returns nothing but "did not finish".

## 2. Reading the medals: the AT is a driven lap, not a formula

    authorScore 13995 · gold 15000 · silver 17000 · bronze 21000

Gold/silver/bronze are exact round thousands. Nadeo's generated medals are never
round like that, so the author typed them in by hand and left the AT as the one
number the editor filled from an actual validation run. The AT is therefore
**a lap the author drove**, i.e. reachable in principle. The author does not
appear on the online leaderboard at all, so the AT is their editor validation
run and no public attempt of theirs is on the board.

## 3. What the map actually is (geometry, from a new `tmmaps blocks` dump)

3770 blocks, 359 items, one Start and one Finish. The load-bearing pieces:

| what | where (world) | role |
|---|---|---|
| `RoadTechStart` + `GateSpecial8mNoSteering` | (1520, 66, 240) | start, steering DISABLED |
| 7 × `RoadTechSpecialTurbo2` | x 1296…1488, y 66, z 240 | the accelerator: 0 → 810 km/h in 3 s |
| `GateSpecial24mReset` | (1130, 67, 240) | re-enables steering at t≈3.3 s |
| road ends | x≈1070 | first launch, a 3.5 s dive |
| ramp / boosters | (360…170, 8…28, −61…131) | redirect: −x,−z flight becomes +z, 940 km/h |
| **the cannon** | (170, 9→11, 131→143) | t=8.50→8.55 s, 841 → **998 km/h**, fires +z |
| **THE HOLE** | wall at z=976, x 112…240, y 10…106 | one missing cell: (176, 64…72, 976) |
| corridor | walls at x=112 and x=240, z 976…1520 | 96 m wide, ~96 m tall tube |
| water | (144…208, 10, 1008…1168) | the floor under the flight is water |
| floor / platforms | z 1200…1264, then a gap, then z 1488…1520 | landing strip |
| `PlatformTechFinish` (+ a Magnet custom block) | (176, 10, 1520) | the finish, on the corridor floor |

**The name is literal.** At z=976 a wall spans the whole corridor from y=10 to
y=106 (`DecoWallBasePillar` at cells (5,9..13,30), `DecoWallBase` (5,14,30),
`DecoWallSlopeBase` (5,15,30), then `DecoWallBase` (5,17,30) and up). Cell
(5,**16**,30) — world x∈[160,192], y∈[64,72] — is the only empty cell in it.
You must fly a 32 m × 8 m window at ~250 m/s. That is the map.

## 4. Do the five humans take the same route? Yes — and most of it is on rails

Decoded all five with `tmtraj decode-all` (all exact, 281–475 samples at 50 ms).

Through t = 8500 ms the five trajectories are **bit-identical to the printed
precision** (e.g. at t=5000 four of the five report x=838.842, y=86.585,
z=134.432). Four different players cannot drive identically; the approach is
input-independent, because steering is gated off for the first 3.3 s and the
rest of it is a scripted dive onto boosters. r03 differs only in the 5th decimal
by t=5000 and by 0.56 m at t=8500 — the tiny freedom that exists is amplified
downstream.

Where they diverge is only *after* the cannon, and the divergence decides
everything:

| run | x at the hole (z=976) | outcome |
|---|---|---|
| r01 14018 | ≈176.5, y≈64.0 | through, lands z≈1315, slides in |
| r02 14031 | ≈177 | through, lands ~1 tick earlier, bleeds more speed → +13 ms |
| r05 23153 | ≈179 | through and FAST (786 km/h at z=1504 at t=14.00 s) but **overshoots** the finish, bounces around for 9 s |
| r03 15478 | ≈166 | clips the wall (895 → 621 km/h at z≈976) |
| r04 21230 | ≈174 | clips the wall (896 → 492 km/h) |

Two of the five records are wall clips and one is an overshoot. **This is not a
ground-flat route; it is a 32×8 m window and a landing box, and the field of 5
has barely explored it.**

r05 is the most informative human run in the set: it proves a car can be at
z=1504 at t=14000 ms doing 786 km/h. r01 is at the same z at the same time doing
312 km/h (it has already crashed into the landing) and still finishes at 14018.
So the endgame — land, keep speed, trigger the finish — is worth tens of ms, not
one or two.

## 5. Where the 23 ms is: measured, not guessed

30 000 unbiased single-move samples from the r01 incumbent
(`tmsearch --dump`, `tmtas analyze-dump`):

    finish            78.4 %
    improve           9.84 %  (3382 of 34360)
    best single move  −27 ms          → 13991 ms, already under the AT
    best-of-800       −23.6 ms expected

    -- where improvements come from (tick of the move; 1 tick = 10 ms)
    ticks         n     improve  best
    0-155      3174       0.00%     0
    155-310    3616       0.00%     0
    310-465    3838       0.00%     0
    465-620    3765       0.00%     0
    620-776    3835       0.63%     2
    776-931    3634      10.40%    27
    931-1086   3374      24.48%    27
    1086-1241  3555      27.90%    23
    1241-1396  3606      23.04%    21
    1396-1552  1963      16.86%    23

Three conclusions, and they set the whole strategy:

1. **Ticks 0–620 are literally dead** — 15 000 random moves there, zero
   improvements, ever. That matches the geometry (steering gated, then a
   scripted dive). The search window is **[620, 1552]**, 60 % of the tape
   discarded, and the fork server can resume at tick ~600 and skip 4/10 of every
   simulation.
2. **This run is nowhere near converged.** 9.84 % of *random single moves*
   improve on the human WR. Map 2's converged incumbent gave 0.01 %. The human
   field here is 5 people; nobody has ground this flat. The 23 ms gap is not the
   interesting number — it is the floor, not the ceiling.
3. **The value is concentrated at ticks 931–1396** (t = 9.3–14.0 s): the
   ballistic flight through the hole, the landing, and the slide into the
   finish. Not the approach.

By operator: `shift` (retime the tail) improves 35 % of the time — extreme, and
a signature of a run whose *timing* is off rather than its shape. `dbl` 9.7 %,
`cos` 10.6 %, `scale`/`edge`/`acc` ≈ 0. Long moves win: span 40–160 ticks
improves 16–17 % vs 4–5 % for span < 10.

## 6. Are the car model and the predicates trustworthy here? Assume NOT

The fitted car model (`--carmodel`) and every predicate in the map-2 control
line were built for a **ground** run on a normal road:

- `crash:speeddrop` — this map *legitimately* loses 150 km/h in the landing and
  gains 160 km/h at the cannon in one tick. A speed-drop predicate would abort
  the good candidates.
- `off:offref:dist` — the reference trajectory is a ballistic arc; being 20 m
  off the reference laterally at the apex is normal and can be *better*.
- `stuck:floor:speed` — the car is never slow here until it has already finished.
- the compensated operator (`--ops mix3`, `comp`) uses a car model fitted to
  ground dynamics; 60 % of this run is in free flight where that model does not
  apply.

So: **search without predicates and without the car model first** (`--ops mix2`,
which is the measured-best uncompensated set), and only adopt either after an
A/B against a concurrent control on this map. Predicates that could be right for
*this* map are geometric, not dynamic — e.g. "abort if |x−176| > 16 at z=976"
(missed the hole) — and are worth building only if throughput turns out to be
the binding constraint.

## 7. The plan

1. Scout search from r01, window [620,1552], `--ops mix2`, T=25, islands with
   migration. (Running: 13987 ms within 10 s of starting, already −8 under AT.)
2. Verify the **fork server** is exact on this map at a boundary tick of ~600
   before using it for throughput; a resume that is not exact is worse than
   useless. Then re-benchmark with a concurrent control per `PROTOCOL.md`.
3. Sector-attack the endgame with the measured windows: 931–1086 (the hole),
   1086–1396 (landing + slide), 1396–1552 (the finish trigger).
4. Re-validate every claimed best through the plain oracle with absolute paths
   before adopting or reporting it. Every batch carries the identity control.

### What each lever is expected to be worth

| lever | expectation | basis |
|---|---|---|
| any search at all | ≥ 24 ms (13994 or better) | best-of-800 from the dump is −23.6 ms |
| endgame (landing + slide) rework | 50–150 ms | r05 is at z=1504 at t=14000 at 786 km/h vs r01's 312 |
| flatter/faster line through the hole | 10–40 ms | the hole is 8 m tall and r01 threads it at its bottom edge |
| the approach (ticks 0–620) | **0 ms** | 15 000 samples, zero improvements |
