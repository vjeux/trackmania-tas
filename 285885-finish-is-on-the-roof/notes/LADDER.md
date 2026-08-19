# 285885 — the arrival-time instrument, and what it already says

*Sidecar v2 from the trigger-half agent. Supersedes nothing in
`bis197047_TRIGGER_v1.md`; it adds the instrument the coordinator asked for and
the first reading off it. Everything I have written here is `bis197047_`-
prefixed and I have touched none of the route agent's files.*

## What it is

`tmmaps ladder` — place the finish gate at each of N stations along the route,
and read off the millisecond at which each run first reaches each station.
On a map whose only waypoint is the finish, this is the checkpoint ladder the
map does not have.

```bash
tmmaps ladder MAP.Map.Gbx --item 0 \
  --stations-file stations.txt --out /tmp/ladder \
  --ghosts A.Ghost.Gbx B.Ghost.Gbx ... -j 48
```

`stations.txt` is one `x,y,z` per line, `#` comments allowed (no `;` inside a
comment — `;` is also a station separator). Put each station **exactly on a
trajectory sample** of a run you know reaches it, so the car's origin coincides
with the gate origin and the trigger cannot be missed vertically.

Source: `bis197047_tmmaps_main_ladder.rs`. Stations used below:
`bis197047_stations.txt`. Raw output: `bis197047_ladder_table.txt`.

## The control is baked in and cannot be skipped

Before it builds a single station, `ladder`:

1. runs the **untouched** map to get the expected answer for every ghost;
2. rebuilds the map with the gate at its **original position** by the same
   surgery it will use for the stations, and runs that;
3. **aborts with exit 9** unless every ghost returns exactly its untouched time.

This exists because the naive version of this instrument lies. `tmmaps gate` /
`segments::move_gate` swaps the item model to `GateFinish32m` before moving it,
which on this map replaces an 8 m gate with a 32 m one; the origin control then
returns **50589 where the untouched map gives 61229**, and every station time is
an artefact in the "look, it fires early!" direction. The ladder uses
position-only surgery (`moveitem`) and proves it each run:

```
ladder: measuring the UNTOUCHED map first
control OK: rebuilt-at-origin reproduces the untouched map for all 4 ghosts
```

If you ever see that line missing, throw the numbers away.

## The first reading — 16 stations along the roof climb

Stations are on the world record's own path. `-` means that run never entered
that station's trigger volume.

| st | x | y | z | rank1 61229 | rank2 88209 | rank3 97769 | my 50509 |
|---|---|---|---|---|---|---|---|
| 0 | 343.46 | 106.01 | 1834.94 | **36049** | – | – | 36049 |
| 1 | 311.37 | 104.49 | 1829.38 | – | – | – | – |
| 2 | 293.49 | 111.80 | 1799.35 | 38054 | – | – | 38054 |
| 3 | 296.61 | 122.40 | 1770.15 | 39471 | – | – | 39471 |
| 4 | 301.12 | 122.81 | 1762.72 | **40074** | – | – | 40074 |
| 5 | 306.63 | 124.99 | 1755.07 | 41709 | – | – | 41709 |
| 6 | 317.60 | 127.17 | 1750.68 | 42639 | – | – | 42639 |
| 7 | 330.60 | 129.26 | 1747.14 | 43659 | – | – | 43659 |
| 8 | 343.32 | 131.68 | 1740.89 | **44789** | – | 48488 | 44789 |
| 9 | 355.70 | 134.25 | 1734.05 | 45789 | – | 50189 | 45789 |
| 10 | 369.44 | 136.55 | 1728.78 | 46699 | – | 51148 | 46699 |
| 11 | 382.98 | 139.01 | 1721.68 | 47819 | 71338 | 52332 | 47819 |
| 12 | 396.83 | 141.55 | 1714.83 | 48809 | 70289 | 53919 | 48799 |
| 13 | 409.27 | 143.59 | 1710.39 | 49569 | 50809 | 55189 | 49549 |
| 14 | 415.49 | 145.21 | 1704.65 | 50579 | 51009 | 56299 | 50439 |
| 15 | 419.03 | 144.00 | 1704.64 (**the real gate**) | 61229 | 88209 | 97769 | **50509** |

## Three things it says immediately

**1. The hairpin is the whole deficit, and it is bigger than the deficit.**
The world record lands at station 0 — `(343, 106, 1835)` — at **36.049 s**. It
then drives *away* from the finish, west and downhill, to the apex at station 4
at **40.074 s**, turns around, and does not get back to the same x (station 8,
`(343, 132, 1741)`, 25 m higher) until **44.789 s**.

**That excursion costs 8.74 s. The map needs 7.4 s.** Everything required to
beat the author time is inside one feature, and it is not on the climb — it is
the U-turn before the climb.

**2. So the route question is a landing question.** The car has to arrive on the
climb line rather than below it. Stations 5–8 are the entry to the climb at
y = 125–132; the current landing is at y = 106. Land at station 5 at ~36 s
instead of 41.7 s and you have 5.7 s; land at station 8 and you have 8.7 s,
which is the whole map. That reframes the search from "optimise the crawl" to
"where can the launch put the car", which is a much smaller search and one the
gate ladder can score directly — put stations on *candidate* landing sites, not
just on the WR's path, and the ladder tells you which ones a launch can reach
and when.

**3. Rank 2 already does something different, and it is worth reading.**
Rank 2 never enters stations 0–10, reaches **station 13 at 50809** and
**station 14 at 51009** — then only passes stations 11 and 12 at 70289 and
71338, i.e. twenty seconds *later*, on the way back down. So rank 2 arrives at
the patch by a route that is not the world record's climb at all, and arrives
essentially as fast (51.009 s vs 50.579 s) despite being 27 s slower overall.
**Whatever rank 2 does between 36 s and 50 s is a second, independent approach
to the patch, and nobody has looked at it.** If it skips the hairpin, it may
already be most of the answer.

## How I would use it next (yours to take or ignore)

- Put stations on the *flight*, not the path: a grid of candidate landing points
  across the roofs at y = 120–140, and ask the ladder which ones any existing
  run reaches at all. Cheap, and it bounds what a launch can do.
- Then score launch variants directly against the earliest station they reach.
  A launch that reaches station 8 before ~36.5 s beats the author time on the
  strength of §1 alone, without touching the climb.
- Remember §0.6 / `TRIGGER_v1` §2: anything the ladder produces is a hypothesis
  until the plain oracle validates it **on the untouched map**.

## Files

| file | what |
|---|---|
| `bis197047_tmmaps_main_ladder.rs` | `tmmaps ladder` + `tmmaps moveitem` source |
| `bis197047_stations.txt` | the 16 stations above |
| `bis197047_ladder_table.txt` | raw output including the control line |
| `bis197047_TRIGGER_v1.md` | the trigger result this sits on |
