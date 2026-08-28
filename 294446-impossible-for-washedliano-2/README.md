# impossible for washedliano poss for sauucey part 2

**The deck the whole map crawls across has steering-dependent friction. Sliding
it diagonally is worth 4.630 s, and it takes the world record by three seconds.**

**impossible for washedliano poss for sauucey part 2** — TAS **37.995** (+2.396) | AT 35.599 | WR 49.274 by FiggeTM

https://github.com/user-attachments/assets/e6ef5549-da4d-4c18-91fe-1345ae488105

*Two cars, with this run own inputs drawn on. The clip ends at our finish; the human is still driving.*

The author time is **not** beaten. The human world record is, by **11.279**.

## The map

One checkpoint, four `PlatformTechFinish` gates at (1456, 90, z in 528/752/784/1008),
a 25-block turbo runway to 990 km/h, a far-corner launcher, and a
288 x 1152 m deck of 324 `PlatformTechSpecialNoEngine` platforms. Two records.

**The launcher is two stages and neither is on a block.** Stage 1 at race
20.100, stage 2 at 22.31 where vy goes to +97.5 and vz reverses — **both happen
outside the 48x48 map grid**, with the car at y = -15, below the runway, for
401 ticks. It is the car under and outside the map being bounced by something
unplaced. The map hands you a free matched pair to prove it: the human hits the
corner **twice**, pass 1 fails and pass 2 launches, and **|v| leaving the bounce
is 277.6 in both**. The bounce sets the magnitude; the launch sets only
direction.

## Where the time went

Four absolute steer overrides on FiggeTM own tape; gas and brake untouched.

| ticks | race | steer | what it does |
|---|---|---|---|
| 2500-2700 | 23.44-25.44 | -127 | **the diagonal slide — 4.630 s** |
| 4170-4290 | 40.14-41.34 | -127 | turn back before the platform east edge |
| 4575-4695 | 44.24-45.44 | **112** | dive onto the z=1008 gate |
| 4705-4785 | 45.54-46.34 | 127 | exit tightening |

**The no-engine deck friction is steering-dependent.** The diagonal lands
*slower* (159 vs 240 km/h) and decays 159 to 79 where the human decays 240 to
26, reaching the reset column at 40.040 instead of 44.670.

And the dive sits at **steer 112, not at the stop** — the only interior steer
value that beats full lock anywhere on this map.

## The trap in the middle of it

Transplanting the human dive verbatim to the earlier arrival DNFs at all ten
start ticks. The trace says why: the candidate reaches the reference race-48.00
*position* at 44.800 but at **215.6 km/h against 161.8** — same place, 54 km/h
faster, so identical steering turns far too tight and swings west. **An
inherited tail is calibrated to the speed that recorded it.** The fix was to
search the dive magnitude, not its timing.

## What is closed

- **The air route.** 43 candidates across the whole proven airborne window,
  scored as a one-way plane crossing on the descending arc: the human arc is the
  east-most of every one. Apex trades against reach at ~2 m per metre. **The
  crawl is mandatory.**
- **Landing further east.** One-sided; the human sits at the optimum.
- **The single-window crawl.** A singleton at tick 2500 — every other start tick
  DNFs, and ten cells at any end 2660-2760 and any magnitude -50 to -127 return
  46.168 to the millisecond.
- **Compound crawl edits.** 40 cells, depths 2 and 3, positive control passing:
  every one is either exactly 46.113 or DNF. **Not one produced a third time.**
- **No embedded author ghost** — zero hits for `CPlugEntRecordData`,
  `CGameCtnGhost`, `CGameGhost`, `RecordData` anywhere in the file.

## The respawn: 6.850 s, mechanically reachable, still blocked

The human respawns once. **A respawn teleports the car to checkpoint 1** —
(231.1, -10.5, 1979.6), frozen 0.75 s — which the live engine says identically
across three insertions. An earlier report put the restore 8.1 m downstream;
that figure came from the *ghost record*, which interpolates across the entity
split at a respawn, and is withdrawn.

Cutting it works mechanically: the car restores, drives the route correctly, and
reaches the far corner 6.03 s after the press against the human 6.25. It then
fails at exactly one thing — **the restore hands the car back at 994 km/h where
pass 1 crosses under its own power at 1064**, and the working launch is
calibrated to the faster departure. 74 cells against it, none launches.

## Files

`replays/TAS_46113.Ghost.Gbx` — validated 5/5 on the untouched map, 8/8
`ghost verify` gates, identity neutralised with the oracle no-op control passing.
