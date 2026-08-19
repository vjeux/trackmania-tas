# Map 285885 — "finish is on the roof to your right" (tag LOL)

unbeaten.at MapId 285885 · uid `bCvuulofsP5dkPvVma8nclZKZ2l` · author **lasyopp**
AT **43079** · human WR **61229** (`lasyoppwtf`) · 3 records · gap 18150 ms.
TMX 285885: Difficulty Expert, StyleName LOL, no comments, no awards, no
replays, UpdatedAt == UploadedAt (2025-12-29), HasGhostBlocks true.

## §9 decode: NO embedded author ghost (verified, positive control)

Header says `validated="1"`, but `tmtraj decode map.Map.Gbx` →
`CPlugEntRecordData (0x0911F000) chunk not found`.

Verified per ACQUISITION §9a on the LZO-decompressed body (831 216 bytes,
`bodyscan`):

| class id | 285885 | 228607 (control) |
|---|---|---|
| 0x0911F000 CPlugEntRecordData | **0** | 2 |
| 0x0309201D ghost inputs | 0 | 0 |
| 0x0303F005 | 0 | 0 |
| 0x03092000 | 1 (coincidental, as on 267460) | 1 |

The same binary decodes 228607's embedded 406-sample lap, so the decoder works:
this map carries no validation lap. `inPlugin: true`, medals round
(46000/52000/65000), and the author's own online PB is 18150 ms SLOWER than the
AT they set.

## §8 field reproduction: 3/3 EXACT

61229 / 88209 / 97769, all to the millisecond. Encoder identity control: the WR
rebuilt through the search's own codec (6274 ticks) re-validates 61229.

## Map shape

Spawn (144, 10, 656); one other waypoint: item#0 `GateFinishCenter8mv2` at
**(419.0277, 144, 1704.6367)**, yaw 0. No intermediate checkpoints. The 113
placed blocks are decoration (`TechnicsScreen1x1Straight`) + the start; the real
geometry is baked blocks + 667 KB of embedded data.

Route all three humans drive: highway blast east to (1584, 9, 1490) at up to
640 km/h (14 s) → climb to y≈73 → slow loop → fast westbound at y=31, 400 km/h
→ **a ramp at x≈630→520, z≈2015→1970 throws the car airborne at (507, 58, 1953)
with vy ≈ +38 m/s** → lands on a roof at y≈106 → a 15 s gear-1 crawl up the
roofs to y≈145 → the finish patch.

## THE FINISH IS A SUNKEN GATE — this is the whole map

All three runs drive over the gate repeatedly without finishing: the WR passes
1.7 m from the gate centre at 51.37 s and finishes only at 61.23 s; rank 2 sits
on the spot at 68.8 s and finishes at 88.2 s.

Map surgery (`tmmaps moveitem`, item 0, position only — a rebuild at the
original position reproduces 61229/88209/97769 exactly, so the surgery itself is
clean):

| gate y | rank1 | rank2 | rank3 |
|---|---|---|---|
| 144 (control) | 61229 | 88209 | 97769 |
| **145** | **50639** | **51009** | **56309** |
| 146 / 147 / 148 | 50639 | 51009 | 56309 |
| 152 | DNF | 51559 | DNF |

**One metre of gate height is worth 10.6 s to the WR and 37.2 s to rank 2.**
The gate sits ~1 m below the roof surface the car rests on, so it only fires
when the body dips into it. That is the LOL, and it is the entire 18 s gap:
earliest arrival at the finish patch on any human line is 50639 ms.

## The route search

Seeded from the WR, searching against a raised (easy-trigger) copy of the map to
score "arrival at the finish patch":

| what | time |
|---|---|
| human WR arrival at the patch | 50639 |
| after 4 min of search | 41359 |
| best route found | **40773** (gate raised), **41024** (gate at real height, 0.8 m off in x) |

The gain is two things: **10 s of not fumbling at the gate**, and **10 s of
driving the rooftop climb at 150-190 km/h instead of 30-100 km/h.** The first
35 s (highway, ramp, launch) is within a metre of the WR's line — the WR drives
that part essentially optimally. Everything after the launch landing is new:
where the humans lift and crawl (30-100 km/h), the TAS holds full throttle up
the roofs and crosses the finish patch at 180 km/h.

**So the route is worth ~41.0 s, i.e. ~2 s under the 43079 AT.**

## What is still missing: the trigger at speed

The fast line passes the finish patch climbing at +9 m/s. Measured by moving the
gate (all at the REAL height y=144):

| gate position | fast tape | WR |
|---|---|---|
| (415, 1707) | **40988** | 50258 |
| (413, 1709) | DNF | 49909 |
| (413 / 415 / 417, 1704.64) | DNF | 50589 |
| (418.61, 1704.87) | **41059** | — |
| (418.70, 1704.82) | 55689 (slow ending only) | — |
| **(419.0277, 1704.6367) = REAL** | **DNF** | 61229 |
| (419.0277, 1704.6367) at y=144.08 | **41079** | — |

So the fast line misses the real trigger by **~0.4 m in x / 0.23 m in z, or
8 cm in height** — and that last 40 cm resists: a homotopy that walked the gate
from (415,1707) through five rungs to (418.61,1704.87) kept clearing at ~41.0 s,
then stalled. ~1.5 M oracle evaluations, 4140 hand-designed tail variants
(lift / brake / both / steer pulses over the last 2.3 s), a height ladder
(144.10 → 144.08 cleared, 144.06 refused) and a lateral ladder all failed to
convert it.

The surface climbs toward +x, so shifting the line +0.4 m in x also raises the
car — the two ways of closing the gap trade off against each other. Slow
endings DO trigger it (the searches produce 55 s finishers that brake and come
back), but they cost more than the 2 s of margin.

---

## Corrected trigger model (jointly with the trigger-half agent, session a83ff4c7)

**Fire iff the car's y ≤ gate_y + 1.25 m while inside a horizontal footprint —
a ceiling, with no lower bound.** Evidence, all plain-oracle, both directions:

- gate dropped to 143.9 / 143.4 / 142.5 / 141.5 / 140.0 at the true x/z: **every
  tape DNFs, including the world record** — the ceiling has come down past the car.
- gate raised to 144.5 / 145 / 146 / 148 / **150**: both tapes still fire, at a
  saturated 41037 (mine) and 50639 (WR) — so there is no floor within 6 m.
- 1 mm bisection on my fast tape (their instrument): clears **144.070**, refuses
  144.069 → its lowest point inside the footprint is **y ≈ 145.320** against a
  ceiling of 145.250. The human WR's is **145.2506** — it misses by under 5 mm,
  three times over, which is the whole 10.6 s.
- Footprint boundary, gate displacement (dx,dz) against my tape:
  `−dx + dz ≳ 0.6` fires. My stalled lateral ladder sat at
  (418.6138, 1704.8683) = `−dx+dz = 0.646`, i.e. exactly on that boundary —
  two independent instruments agreeing.
- Trade curve: **every 10 mm of height buys ~0.07 m of the (+x,−z) diagonal**,
  and all of it fires at 41.03-41.07 s, so the choice is free in time.

**Caveat on my own instrument:** `fk btraj` absolute tick labels appear ~300 ms
fast on this build (hardened-build defect #3, fork child tick labelling). The
trajectory *shape* is sound — it is a live engine readout, md5-distinct from the
WR's telemetry, 65 m and 131 km/h away from the WR at race 40.0 s — but do not
trust its absolute race times without calibrating against an identity run.
Separately: **a candidate ghost written by the search carries the TEMPLATE's
`CPlugEntRecordData` unchanged**, so decoding a candidate gives the seed's
trajectory, not the run's. That trap cost the other agent an hour.

## Final position

| | time | validated on |
|---|---|---|
| human WR | 61229 | untouched map |
| **`seedY.Ghost.Gbx`** | **50229** | **untouched map, plain oracle, WR control 61229 in the same batch** |
| author time | 43079 | — |
| `bis_418.6138_best.Ghost.Gbx` (the fast route) | **41074** | gate at true x/z, y = 144.07 |
| same tape | 41037 | gate at true x/z, any y ≥ 144.5 |
| same tape | **DNF** | **untouched map** |

**The AT is a real target and this map is not a joke: the route to the finish
patch is worth ~41.04 s, about 2 s under it.** What is missing is 70 mm of
height (or 0.42 m of line) at the trigger, and that has resisted ~50 000
designed tapes, ~1.5 M shaped search evaluations, a height ladder, a five-rung
lateral ladder and the other agent's disjoint search window.

**Best remaining idea, untried:** the footprint is much longer in `−dx/+dz`, so a
line that enters at the `(+x,−z)` corner and runs ALONG the diagonal instead of
across it stays inside longer; at the measured 1.1 m/s sink rate, ~65 ms more
inside the footprint covers the 70 mm with no change of entry height.

**Height ladder result (final):** the 10 mm ladder from the peer's measured
boundary **failed on its first rung** — gate y = 144.06 at the true x/z, seeded
from `bis_418.6138_best`, 168 workers, 50 430 evaluations in 6.4 min, **0
finishers, 0 % finish rate**. So even 10 mm of the 70 mm does not come from
perturbing this line: the last rung is not a gradient, it is a wall. Anything
further should attack the footprint chord (enter at the `(+x,−z)` corner and run
ALONG the diagonal), not the entry height.
