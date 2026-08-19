# 227654 "The Blev Special" — PLAN (agent `blev2`, 2026-08-19)

AT **57.853**. Previous agent's best validated **59.912**. Gap **2.059 s**.
Everything below was established or re-verified by me tonight; the numbers I
inherited that I re-checked are marked ✔.

## Re-verified inherited facts

| claim | my check |
|---|---|
| `tas_59912` = 59.912 on the untouched map | ✔ 59912 |
| `clean_64871` = 64.871 | ✔ 64871 |
| human WR 147.031 / #2 676.640 reproduce exactly (controls) | ✔ both |
| `p3_46646` = 46.646 on `map_seg2`, `p1_49107` = 49.107 | ✔ both |
| map sha256 `a5768448…5b8d82` | ✔ |

## What I found that changes the problem

### 1. The flight is BALLISTIC, and mid-air gates do not fire

From the merged (all-entity) telemetry: the car leaves the bowl at ~142.1 s at
651 km/h and is airborne from 142.5 s to the finish at 147.031 — a 717 m
ballistic arc from (823,186,674) to (1456,210,336), apex y = 320.7. No input
changes it. So the tail's chaos is entirely in the bowl, and the finish is a
2-D targeting condition on the release state.

I swept the map's finish gate (block #854, cell-placed) over **every cell the
arc passes through** (53 cells) and over the full 3×7×3 neighbourhood of a
mid-flight cell (63 maps): **0 fire**. Ground cells do fire. So there is no
mid-air rung and no readout of where a failing arc goes.

### 2. There IS a working float-precision gate ladder — before CP2

CP1 (block #511) and CP2 (block #541) are **free blocks**: their positions live
in chunk `0x0304305F` as float triples, not in the block record. I added
`tmmaps freeblocks` / `moveblock` / `moveblockcell` / `blocks` / `bchunks` to my
fork and confirmed:

* `map_seg2` + `moveblock --block 541 --at x,208,576` slides the finish along
  the road with **float precision** and a **monotone** readout —
  x=929 → 53903 (= `map_seg2` exactly, the origin control ✔), 937 → 53620,
  945 → 53308, 953 → 52907, 956 → 52703.
* Rotated to **yaw 0** the same gate reads the *other* axis:
  (932,208,578.0) → 48788, (932,208,578.5) → 48963 — i.e. it times the car's
  **z** as it slides inside the wedge.
* Waypoint COUNT is what a map may not change: renaming any spare block into a
  waypoint model (`GateFinish` / `RoadTechFinish` / `RoadTechCheckpoint`)
  breaks every run (4 waypoints, ghost declares 3). Renaming to a
  non-waypoint model is harmless (`147031` unchanged ✔). Moving CP2 keeps the
  count at 3, which is why the ladder works.
* A hole in the road **cannot** be plugged with a renamed spare free block —
  tested three ways (pillar and wall models, over a deleted road block): all
  DNF. So no rung is placeable *after* CP2.

### 3. The respawn graft works, and proves the tail is a function of the CP2 state

New tool `blevcat` joins two tapes: A[0,k) ++ B[j,end). Grafting the WR's last
respawn + its 10.531 s winning tail onto the WR's own prefix finishes exactly
on the arithmetic — 5590 → 65951, 5610 → 66151, … (+200 ms per +20 packets),
identity control 147031 ✔. Grafting the same tail onto **p3** (searched line)
or **p1** (wedge-cut line): 0 of 31 and 0 of 31 finish, and a (k, tail-shift)
sweep of 124 more also gives 0. **A TM2020 respawn restores the run's own CP2
crossing state, not a canonical one** — so it does not make the human's tail
portable. Worth knowing; it closes that door cleanly.

### 4. THE LEVER: the wedge is a state collapse, and it is 9 s of waste

Telemetry 47.000 → 51.750 s: the car is pinned at **x = 959.83 ± 0.01,
y = 210.96 ± 0.02**, speed 1.7–3.9 km/h, steer −127, gas on, sliding only in
**z**, 577.86 → 578.88 (one metre in 4.75 s). It is a one-dimensional residual.

And the approach to it is the real waste: at 37.75 s the car is at x = 1040
doing **198 km/h**; it then brakes, crawls at 20–50 km/h for nine seconds and
finally noses into the corner at 46.9 s. Eighty metres in nine seconds.

## The plan

**Do not re-derive the tail. Keep the whole post-wedge program byte-identical
and arrive at the wedge earlier.**

Splice `W[0,k) ++ clean_64871[m,end)` where W is a candidate that gets wedged
early and m is a tick inside clean's own wedge. Then

```
finish = 64871 + 10·(k − m)
```

with everything after the splice — the dwell, the escape, CP2, the run-up, the
bowl, the release and the 717 m arc — exactly the inputs that are already known
to work. To beat 57.853 the candidate must be in the wedged state about
**7.0 s** earlier than the human, i.e. wedged by ≈ 40.0 s instead of 46.9 s,
assuming the dwell has to be matched tick for tick. The (k, m) sweep measures
how much of that assumption is real.

### Steps

1. **Wedge-gate segment map** `map_wedge`: `map_seg2` + block #541 at
   (932, 208, 578.0), yaw 0. Fires at 48788 on clean_64871. It fires only when
   the car is at x < 964 with z ≥ 578 — the corner — so it is hard to satisfy
   without actually wedging. Origin control: the same surgery at CP2's own
   position must give 53903.
2. **Search** the window [3925, 5325) (t = 37.75 → 51.75 s) of clean_64871
   against `map_wedge`, minimising the gate time. Everything outside the
   window is frozen.
3. **Splice sweep** the survivors: (k, m) over the wedge, validated on the
   **untouched** map with `clean_64871` and the human WR in every batch as
   known-answer controls.
4. Whatever wins, re-validate standalone on the untouched map, then do the
   §0.7 work (human route story + low-input family).

### Guards

* Own staging root `/tmp/blev`, own build tree `/tmp/tmtas-blev`. No shared
  `/dev/shm` root; `tmtas validate` already uses a pid-unique one.
* Every ladder claim carries its origin control in the same batch.
* Decoy check for step 2: a candidate can in principle clip the corner at speed
  and fire the gate without wedging. That is why step 3 (the real map, real
  finish) is the adjudicator and the gate is only a pre-filter.
* Files are versioned sidecars, prefixed `blev2_`; nothing overwritten.
