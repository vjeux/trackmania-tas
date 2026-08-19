# ACQUISITION addendum — `tmtraj decode` can return the WRONG CAR, silently

Found 2026-08-18/19 on **153527**, where it invalidated an entire published
measurement without anything looking wrong. Write-once; supersede with `_v2`.

This belongs with `ACQUISITION_addendum_controls_v1.md`: it is the eighth
instrument on this project whose broken state was indistinguishable from its
working state.

---

## The defect

`CPlugEntRecordData` stores one record per **entity**. `tmtraj decode` (and the
Python `entrec.py` it was ported from) picks the player by a single rule:

```rust
// Some ghosts carry TWO CSceneVehicleVis entities: a heavily decimated
// one (6-7 samples, ~3 s apart) plus the real full-rate track. Always
// take the one with the most samples.
if cid == Some(CLASS_CSCENEVEHICLEVIS) && veh.map_or(true, |v| ent.times.len() > v.times.len())
```

That rule is correct for a solo time-attack ghost. It fails on two conditions
that co-occur on marathon / RPG maps:

1. **The player's car is destroyed and recreated on respawn.** On 153527 the
   player is **46 separate `CSceneVehicleVis` entities**, tiling the 5 661.680 s
   race back to back at 50 ms with zero overlap and zero gaps. No single one of
   them is large.
2. **Another car is in the recording.** 153527's ghost carries a second player's
   car as one continuous 85 811-sample entity spanning the whole race.

"Most samples" therefore selects **the other player**. There is no error, no
warning, and the CSV that comes out is a plausible-looking 85 811-sample
trajectory over the right map with the right race duration.

## What it cost

The whole quantitative core of `153527/RESULT.md` §3 — 26 023 m of residue path,
77.1 km/h average, the speed distribution, and specifically the
**"path/displacement is 17× at CP3 and 20× at CP12"** that got the map reopened
— was measured on the other player's car. The corrected figures are 24 546 m,
72.8 km/h, and ratios of 1.6–13.9×. The 17× and 20× do not exist.

(The parts of that write-up that came from the packet stream — 111 respawn
presses, the segment table, the 1 214.585 s floor — were right, and agree with
the corrected track to 10 ms on nine of nine window starts.)

## How to detect it — three checks, all cheap

**1. Coverage.** Multiply the returned sample count by the modal period and
compare with the race duration.

```
153527: 85 811 x 60 ms = 5 148.7 s   against a 5 661.7 s race   <- 9.6 % missing
        23 110 gaps > 60 ms, largest 15.6 s
```

A track with real gaps is either the wrong entity or an incomplete one. A
correct solo ghost has none.

**2. The waypoint referee — this is the decisive one.** `tmmaps list MAP` gives
every waypoint's grid cell. Convert with `world = (32cx + lx, 8*(cy − y_ground)
+ ly, 32cz + lz)` and check that the track is inside the right cell at each
declared split. Recover `y_ground` from the SPAWN and GOAL blocks against the
track's first and last samples — on 153527 the car sits at y = 82.0 on a spawn
block anchored at 80 and y = 234.2 on a goal block anchored at 232, so
`y_ground = 8`, confirmed twice, and items carrying both a cell and an absolute
position agree.

```
                    inside the right checkpoint cell at the 12 splits
  entity #9 (decoder's pick)                 2 / 12
  the 46 per-life entities                  12 / 12
```

**3. Start and finish.** The player's first sample is at the spawn block, at
walking pace. 153527's decoder pick starts **50 m below the spawn, in free fall
at 81.4 km/h**, and its last sample is 500 m from the goal block.

## The fix

Do not select by sample count. Select the entity set that:

* is not stationary (bounding box under a metre in x and z is a start-block
  placeholder — 153527 has two of those),
* tiles the race with no overlap and no gap, and
* is inside the right waypoint cell at every declared split.

`route track` / `route which` in `153527/route_tools/` implement all three and
print the 12/12 vs 2/12 table. The per-life entities also give something the
single-entity path never had: **exact life boundaries**, and a 50 ms track
instead of a gap-ridden 66 ms one.

## The general lesson, restated

The stock decoder is an instrument that can only say yes: handed a ghost with
50 vehicle entities it returns one of them and never reports that it chose.
**Every trajectory measurement on this project should carry the waypoint
referee** — it costs one `tmmaps list` and a cell comparison, and it is the only
check here that would have caught this.

Two further notes for anyone re-checking old work:

* Multiple `CSceneVehicleVis` entities are common (126859's field has ghosts
  with 2, 3 and 4). They are usually harmless because the largest really is the
  player. The danger is specifically **long maps with respawns**, where the
  player's own record is fragmented.
* A ghost's declared splits (`0x0309202B`) come from the ghost header and are
  correct regardless of which entity you decode — so split-based work
  (retry deletion, segment tables) survives this defect while anything using
  positions or speeds does not.
