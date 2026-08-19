# 227654 "The Blev Special" — working notes

MapId 227654, uid `bV_szgZIzzKGbW3Zujo8pjxZSC2`, AT **57853**, human WR **147031**
(`ailiei.`), 2 records. TMX tags DesertCar / SnowCar / Bobsleigh.

## The three cheap checks, done first

### 1. Decode the map — NO embedded author ghost

`tmtraj decode map.Map.Gbx` → `CPlugEntRecordData (0x0911F000) chunk not found`,
even though the header says `validated="1"`.

**Positive control, same binary, same session:** `tmtraj decode` on
`228607/map.Map.Gbx` → `version 10 samples 406`, and on `228811/map.Map.Gbx` →
`version 10 samples 412`. So the decoder works and the negative is real. This is
the §9a case (as on 267460): `validated="1"` is necessary, not sufficient.
`atSetByPlugin: true` on unbeaten.at is therefore *uncorroborated* here — we
cannot read the author's lap.

(The hardened `tmmaps` in this build has no `chunks`/`body` subcommand, so the
class-id count of the decompressed body was replaced by the positive control
above, which is the same evidence: one binary, three maps, two ghosts found and
one genuinely absent.)

### 2. §8 field reproduction — PASSES, 2/2 exact

```
rank00001_147031.Ghost.Gbx   147031
rank00002_676640.Ghost.Gbx   676640
```

Both records return their recorded millisecond exactly. The alternate-car tags
do not break the oracle: the oracle IS the game server, and it reproduces
whatever vehicle the map hands the player.

### 3. Does our tooling model this vehicle? YES, exactly.

* **Encoder round-trip is exact.** `tmsearch --template WR --verify` →
  re-validated at **147031**, unchanged. 14854 ticks, one archive.
* **Every packet is mutable.** `tapeinfo`: 14854 packets, **0 frozen slots**,
  all mode 2. Nothing about this map's inputs is outside the searcher's reach.
* **The input alphabet is pure keyboard.** Both human tapes use steer ∈
  {-127, 0, +127} only, gas ∈ {0,1}, brake ∈ {0,1}. WR = 427 change events.
  Nothing analog, nothing car-specific.

So: the toolchain models this map completely. The "alternate car" story is a red
herring for the toolchain — it is only a story about how the map DRIVES.

## The real finding: the world record contains ELEVEN RESPAWNS

`tmtraj decode` shows only 365 samples of a 147 s run because the recording is
split across **27 CSceneVehicleVis entities**. `alltraj` (new, merges them all)
shows why: after CP2 the run is a loop of

```
long attempt ... ends airborne at 460-590 km/h ... respawn ...
1.01 s replay ending EXACTLY at the CP2 crossing state
(943.871, 210.019, 585.062) @ 84.6 km/h  ->  (914.864, 210.021, 587.024) @ 118.384 km/h
```

byte-identical eleven times. The human crossed CP2 at 54329 and then needed
**twelve attempts** at the final section; the successful one took 10531 ms.

### The respawn is a bit in the input tape

`tapecut scan` histograms every packet's state segment. 14796 packets carry
`st[1] mo[1]`; 58 carry an extra 34-bit field. Its values:

| value | meaning |
|---|---|
| `2` (0x2) | the resting value |
| `0x80000002` | **RESPAWN** — appears at exactly the 11 respawn times |
| `0x20402`, `0x8082`, `0x800002` | other action keys, pressed/released in pairs |

So a respawn is bit 31 of a per-packet 34-bit state word. `Replay::build`
re-emits it verbatim, which is why the WR's tape round-trips exactly without the
searcher ever knowing respawns exist — and it is also why the searcher inherits
them and can never move them.

### Splicing them out is EXACT, not an approximation

Because the respawn restores the CP2 crossing state, the state at the end of the
last rollback (t=136500) is the *same state* as the CP2 crossing (t=54340). So
deleting packets [5584, 13800) — 8216 ticks, 82160 ms — must reproduce.

```
s_5584_13800.Ghost.Gbx   64871    <- plain-oracle validated
```

and the neighbours of that diagonal all DNF at cp2, which is the control: the
alignment is exact and one tick either way is wrong. Four independent (p,q)
pairs with q-p = 8216 all give 64871.

**So the human's own driving, with the crashes removed, is 64871 ms.**
The real gap to the author time is not 89178 ms. It is **7018 ms**.

## Where the 7018 ms is

Per-second speed profile of the clean run (`wr_all.csv`):

| window | what happens |
|---|---|
| 0-13 s | accelerate, launch at 617 km/h, land on the y≈201 plateau |
| 13-25 s | **FUMBLE** — the car wanders a 20 m loop at 25-100 km/h, ~8 s of nothing |
| 25-31 s | drive x 1450 → 1210 at 110-175 km/h |
| 32-33 s | crash to 12 km/h |
| 34-37 s | accelerate to 188 km/h |
| 38-52 s | **WEDGED** — from 46.2 s the car is pinned at (959.8, 211.0, 578.x) doing **2 km/h with the gas on and the steering at FULL LEFT** for 5.4 s |
| 52-58 s | escape, accelerate to 148 km/h |
| 58-64.9 s | enter a flat circular bowl at 130 km/h, one lap at full left lock accelerating to **670 km/h**, release, fly to the finish |

The wedge is one input event held too long: `48670 steer=-127` … `52100
steer=0`. Full left, into a wall, for 3.43 s.

## Why the tail cannot simply be shifted

Cutting even **5 ticks (50 ms)** out of the frozen wedge makes the run DNF at
cp2 — it still reaches CP2, it just can no longer complete the final bowl-launch.
Cuts of 160+ ticks fail before CP2 (the wedge escape itself changes).

So the final section is chaotic at the millimetre and must be re-derived for any
upstream change. That is also why the human needed twelve tries: the tail is a
1.55 s full-left hold in the bowl and a release whose timing sets the whole
flight.

The tail's whole program, from the clean tape, is 18 keyboard events.

## The method that works: CUT the prefix, then RE-DERIVE the tail

Two moves, both adjudicated by the plain oracle, nothing else.

### Move 1 — time-warp cut, measured against the CP2 segment map

`tmmaps build` gives `map_seg2.Map.Gbx` (finish moved to CP2; fires ~426 ms
early, so seg2 53903 == real CP2 54329). Sweeping `tapecut splice --cut A:A+D`
over the wedge and scoring on seg2:

```
A=4770 D=480   seg2 49107   <-  -4796 ms
```

That is the whole wedge deleted. Cuts elsewhere do NOT work: 16779 cuts through
the 13-25 s fumble (A 1300..2700, D 20..1200) produced **zero** runs that even
reach CP2. A cut only survives where the car is genuinely STATIONARY, so its
state at the two ends is nearly the same state. The wedge qualifies; a fumble
where the car is still moving does not.

### Move 2 — the tail is a THREE-parameter program, not two

`tailgen` builds candidates by memcpy + bit-patch (32851 candidates in 20 s,
vs 25 minutes for the same count through a full re-encode). The family is

```
ticks [S, a) : the template's own run-up, TIME-SHIFTED by s ticks
ticks [a, b) : steer = -127          (in the bowl)
ticks [b, N) : steer = +127          (the flight)
```

* With **two** parameters (a, b) and no shift, the tail is unrecoverable. Even a
  **50 ms** upstream cut kills it: 261 consecutive release ticks, all DNF.
* Adding the **run-up shift s** recovers it. On the 50 ms cut, `s = -21` finishes.
  On the 4796 ms cut, `s = +8, a = 5540, b = 5648` finishes.

`b` (the release out of the bowl) is the sharp one — on the unmodified tape only
`b = 6136` and a second window at `b = 6224..6276` finish, out of 61 tried; the
window is one to a few ticks wide. That is what the human was failing eleven
times.

### Result so far

| tape | ms | how |
|---|---|---|
| human WR as recorded | 147031 | 11 respawns |
| WR with the respawns spliced out | **64871** | exact splice, plain-oracle validated |
| + wedge cut + re-derived tail | **59912** | plain-oracle validated |
| author time | 57853 | |

All keyboard: `tas_59912.Ghost.Gbx` uses steer ∈ {-127, 0, +127} only, 257
change events, 0 frozen slots.

### Where the toolchain does NOT reach on this map

* **`fk traj` cannot locate the car's state.** "no address tracks the reference
  ghost's path: state not located" — the best candidate address tracks at 2.95 m
  rms. The vehicle entity is destroyed and recreated repeatedly on this map
  (27 CSceneVehicleVis entities in the WR), which is very likely why. So there
  is no per-tick trajectory for a candidate tape, and no fork-server progress
  scoring.
* **`tmmaps probe` cannot relocate a gate**: "map has no relocatable waypoint
  gate item" — the waypoints are BLOCKS here, not items. So no corridor ladder
  in the tail either.
* Consequence: past CP2 the classic search has **no gradient at all**
  (score_dnf is constant once cps == 2), which is why the tail had to be solved
  by an explicit parameter sweep rather than by the searcher.
