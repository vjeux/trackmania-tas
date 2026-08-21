# Snowcarmc free 42

**Snowcarmc free 42** — TAS **18.690** (−0.120) | WR 18.810 by Roquett, best of 63 | AT 40.074, beaten by every human on the board

https://github.com/user-attachments/assets/d6fc2ce8-70b3-423b-bc3e-67681e44b804

**Two snow cars, 0.120 apart, and the difference is only visible in the
corners.** Both runs are on screen; the camera follows Roquett's record. On the
straights the cars are inside one another — separation over the whole run is
**mean 1.02 m** — and then at 10.3 s, 14.4 s and 16.5 s they come apart into two
distinct cars, each on its own line through the turn, before rejoining. The
widest they ever get is **4.28 m, and it happens at 18.650**, in the last tenth
of the run: the gap does not exist until the end, and then it is the result.

This is the whole field's problem in one picture. All 63 recorded runs drive the
same line to a **2.56 m RMS lateral spread**, with **not one brake input**
anywhere in any of them, so nobody wins this map by finding a different route.
The 120 ms come from where the two cars are *not quite* the same in the corners.

**TMX [156451](https://trackmania.exchange/mapshow/156451)** · uid `7jgWLFiAFRQ09bAnRA6DeGFnm_e` · by **Bapdada**, 2024-02-23 · snow car

| | time |
|---|---|
| Author time | 40.074 |
| Human world record — **Roquett**, of 63 records | **18.810** |
| **Our TAS** | **18.690** |

**Lead with the world record, not the author time.** Every one of the 63 human
records already beats this author time by about a factor of two, and the author
himself sits **twelfth on his own map at 19.610**. Saying "author time beaten"
here would be true and useless. The result is that our tape is **0.120 under the
best of 63 humans**, and 0.205 under the second-best.

The whole leaderboard runs 18.810 to 21.830.

---

## The map

**No checkpoints at all.** One Spawn item and one Goal item, both from a custom
`DirtHill` pack, and nothing in between that has to be collected:

```
DH-Waypoint\DH-Start1        992, 16, 576     Spawn
DH-Transitions\DH-Transi6.b 1024, 16, 639
DHC-Turns\DHC-Turn4         1056, 24, 703
DHC-Turns\DHC-Turn3.c        896, 24, 831
DHC-Transitions\DHC-Transi4.c 928, 24, 831
DH-Turns\DH-UTurn1           992, 32, 863
DHC-Turns\DHC-Turn3.c        960, 32, 864
DH-Transitions\DH-UptoFlatR  992, 32, 864
DH-Turns\DH-UTurn2          1120, 40, 864
DH-Transitions\DH-Transi6.b 1152, 40, 864
DH-Waypoint\DH-Finish2      1120, 48, 800     Goal
```

Those eleven pieces are the entire course. Around them: **92
`SnowGateGameplay` items** — the curtain that swaps the car to the snow car, at
y = 26 spanning x 0–1376 and z 512–1340 — on a base of **521
`DecoPlatformIceBase` blocks** with an 88-block ice-cliff perimeter, all at cell
y = 9. The map's header carries an empty `<playermodel id=""/>`; the car change
is entirely those gate items.

It is a **terraced hill climb**: the car's height goes 21 → 26 → 34 → 42 → 50 m,
the finish sits on the top terrace, and the only links between terraces are the
two U-turn ramps. The ice base is about 40 m below the finish.

**The finish is a plane at z = 787.65.** Measured without touching the map, by
extrapolating each of the 63 ghosts' last telemetry sample along its own velocity
to its own declared finish time: all 63 land within ±0.1 m, crossing in −z at
about 40 m/s at y = 50.17, with x spread 1133.6 to 1141.8. So the lateral
aperture is at least 8 m wide, and **one millisecond is worth four centimetres**.

The map is `validated="1"`, but there is **no embedded author ghost** — no
`CPlugEntRecordData` and no input chunk anywhere in the decompressed body. The
usual shortcut of reading the author's own validation inputs is not available
here.

## The field: one line, and nobody ever lifts

The striking fact about this leaderboard is how little there is to choose
between the runs.

* **Not one brake input in the entire field of 63.** No throttle lift in the top
  six either. Every run is flat out from lights to line.
* Path length 714 to 769 m at 126 to 166 km/h, so **the time is essentially path
  length divided by about 38 m/s.**
* Mean pairwise RMS lateral separation **2.56 m** (min 0.54, max 4.48) — one
  line, a narrow road.
* The world record is **not** the most central run (rank 49 of 63 by centrality);
  its nearest neighbour is rank 2 at 0.54 m. It wins on speed, carrying +4 to
  +9 km/h over the field median through the whole second half.

## Our run

18.810 → 18.753 → 18.750 → 18.743 → 18.730 → 18.725 → 18.711 → 18.702 → 18.700 →
18.691 → **18.690**, seeded from the downloaded human world record, about 25
million oracle evaluations over five hours on 176 cores.

Verified: three runs of the raw search tape, then **six runs of the finished
ghost across two untouched copies of the map — one freshly downloaded from the
Nadeo CDN at verification time** — all returning 18690, plus the raw dedicated
server reporting `IsValid: true`, `Time 18690`, `NbCheckpoints 1`,
`NbRespawns 0`, with the declared result identical. The TMX copy of the map and
the Nadeo CDN copy are byte-identical (sha256 `28098514…e9a947`).

The published ghost's telemetry is its own run, regenerated sample by sample out
of engine memory, so it plays back as what it is rather than as the run it was
built on.

**Tolerance, and a caveat we would rather publish than hide.** Perturbing every
input event by ±1 tick, no-op corrected: the **18.700** tape survives **68.9 %**
of real moves, and the **18.690** tape that replaced it survives **48.5 %**. Ten
milliseconds faster, twenty points less forgiving. Four earlier maps in this
project found the fastest tape was also the most forgiving; this pair is a
counterexample, so treat that as conditional rather than settled. 48.5 % is still
mid-range here, so the shipped tape remains a plausible human target.

## What does not work — and how we know the test would have seen it

Each of these is a negative, and each is quoted with the control that would have
detected a positive.

**The start trick is unavailable: race tick 0 is inert on this map.** The tape
starts at race zero with the throttle already on. Turning the gas off, applying
full lock, or pressing the brake at race 0.000 each return **the identity's own
time, to the millisecond**. The yes-control is the same three edits one tick
later, at 0.010: **all three DNF**. The inputs are delivered and decisive at tick
1, so the tick-0 slot genuinely has no effect — starting on the second tick is
already the only thing this map does.

**No brake and no throttle lift helps, anywhere on the lap.** 748 candidates — a
20 ms and a 50 ms pulse of brake-on and of gas-off at every 100 ms of the run —
gave 318 finishers, 431 DNF, and **best = the identity**. The only ties are brake
pulses at 9.4 to 9.6 s, in the hairpin, where the car is grip-limited rather than
power-limited. At fine resolution over the launch — a lift of 0, 10 or 20 ms at
each of the first 150 ticks — **every lift except the tick-0 no-op DNFs.**

**The route is not enforced, and no shortcut exists that we could find.** With no
checkpoints, nothing makes you drive the course, so this had to be tested rather
than assumed: 78 diverted tapes (full lock left and right at ±40, ±83 and ±127
units, held to the end, from each of 13 divert times spanning 11.5 to 17.5 s).
**Every one DNF** except two late shallow diverts that wander and rejoin, at
20.840 and 18.959; the identity in the same batch returns 18.702. The geometry
agrees — the terraces are joined only by the two ramps and the finish is on top.
State it as *the author time is not route-enforced*, not as *a cut exists*, and
not as *we proved none does*.

## Re-checking this map: the validation is BUILD-DEPENDENT

**Anyone re-running the field check here without splitting by game build will
conclude our oracle is broken.** Re-simulating all 63 ghosts gives 44 exact, 10
different and 9 DNF — a raw 70 % that looks like a failed map. Split by the game
build each record was set on:

| build the record was set on | exact | different | DNF |
|---|---|---|---|
| **2026-02-02 `git128149`** (current) | **9** | 0 | 0 |
| no build string | **3** | 0 | 0 |
| 2025-07-04 | 7 | 5 | 1 |
| 2024-12-12 | 9 | 5 | 5 |
| 2024-01 … 2024-09 | 10 | 5 | 3 |

**Twelve of twelve exact for everything recorded on the current build, the world
record included, to the millisecond.** Every failure is a 2024 or 2025 recording.
The snow car's physics changed between then and now; the simulator agrees with
the game as it is today, which is the physics that matters for a record set
today.

## What is left

The best-sector splice over the human field is **18.528** — 20 equal-arclength
stations, forward-only projection, each sector taken from whichever run is
fastest through it. That is a bound and not a lap, and it is biased toward runs
that cut inside a station pair, but it is 0.162 under our time.

Our run is fastest-in-field in 8 of those 20 sectors. **The rest sits with rank 6
(19.093), which owns six of them** including the last by 56.8 ms. Its line
through the final U-turn is **wider and 3 to 5 km/h faster all the way to the
line** — apex at z 892.6 against our 888.1, reaching x 1166.3 against our 1162.2.
A search seeded from rank 6 does not get there; it plateaus about 120 ms behind.
Moving our line onto rank 6's U-turn without giving the time back in the first
third is the open question on this map.

One method note for whoever picks it up: after the single-move neighbourhood was
measured empty — 0 improvements in 49,440 unbiased single moves, and none in
1,116 systematic single-window steer biases — the only operator that moved this
tape again was **multiple mutations per candidate**, which walked eight
consecutive improvements from 18.699 to 18.690.
