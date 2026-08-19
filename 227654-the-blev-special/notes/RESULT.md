# 227654 — "The Blev Special" (BlevTheRealOne)

**AT 57853 · human WR 147031 · 2 records · TMX tags DesertCar / SnowCar / Bobsleigh**

## Verdict

**This IS a real target, and the advertised 89178 ms gap is an artefact.**

The world record is a run with **eleven respawns** in it. Splice them out — which
is exact, not an approximation, because a TM2020 respawn restores the checkpoint
crossing state — and the same human's own driving is **64871 ms**. The true gap
to the author time is **7018 ms**, not 89178.

We closed most of it. Best validated tape at time of writing: **59912 ms**,
keyboard-only, plain-oracle validated.

## The three cheap checks

| check | result |
|---|---|
| decode the map for the author's validation ghost | **absent.** `validated="1"` but no `CPlugEntRecordData`. Positive control with the same binary: 228607 → 406 samples, 228811 → 412 samples. §9a case. |
| §8 field reproduction | **2/2 exact** (147031, 676640). The oracle models this map's vehicles perfectly — it is the game server. |
| does our tooling apply to this vehicle? | **yes, completely** for the tape; **partly not** for the instruments (see below) |

Tooling detail:

* encoder round-trip on the WR: 147031 → **147031**, unchanged
* 14854 packets, **0 frozen slots** — every tick is mutable
* input alphabet is **pure keyboard**: steer ∈ {-127, 0, +127}, gas/brake binary,
  in *both* human tapes. Nothing analog and nothing car-specific.

So "alternate car types" changes how the map *drives*; it changes nothing about
whether we can drive it.

## What the record actually is

`tmtraj decode` reports only 365 samples for a 147 s run, because the recording
is split over **27 CSceneVehicleVis entities**. Merging them (`alltraj`) shows
the run: cross CP2 at 54329, then twelve attempts at the final section, eleven
of them ending airborne at 460-590 km/h followed by a respawn. Every respawn
replays a byte-identical 1.01 s ending exactly at the CP2 crossing state.

### The respawn is bit 31 of a 34-bit per-packet state word

`tapecut scan`: 14796 packets carry `st[1] mo[1]`, 58 carry an extra 34-bit
field. `0x80000002` appears at exactly the eleven respawn times. Other values
(`0x20402`, `0x8082`, `0x800002`) are other action keys, pressed and released in
pairs. `Replay::build` re-emits the whole state segment verbatim, which is why
the WR round-trips exactly while the searcher has no idea respawns exist.

**This is new capability for the project**: respawns are readable, locatable, and
deletable in any tape.

### Splicing them out is exact

Deleting packets `[5584, 13800)` — 8216 ticks, 82160 ms — gives **64871 ms**,
plain-oracle validated. The control: four independent (p, q) pairs with
q - p = 8216 all give 64871, and every neighbour one tick off DNFs at cp2.

## Where the 7018 ms is

| window | what the human does |
|---|---|
| 0-13 s | accelerate, launch at 617 km/h onto the y≈201 plateau |
| **13-25 s** | **fumble** — wanders a 20 m loop at 25-100 km/h, ~8 s lost |
| 25-37 s | drive the plateau, one crash to 12 km/h at 32 s |
| **38-52 s** | **wedged** — from 46.2 s pinned at (959.8, 211.0, 578.x) at **2 km/h, gas on, steering FULL LEFT into a wall for 5.4 s** |
| 52-58 s | escape, accelerate to 148 km/h |
| 58-64.9 s | enter a flat circular bowl at 130 km/h, one lap at full left lock, **130 → 670 km/h**, release, fly to the finish |

The wedge is a single input event held too long: `48670 steer=-127` …
`52100 steer=0`.

## Method

Two moves, everything adjudicated by the plain oracle.

**1. Time-warp cut, scored on the CP2 segment map.** `tmmaps build` gives
`map_seg2` (finish at CP2, fires 426 ms early: seg2 53903 == real 54329).
`tapecut splice --cut 4770:5250` deletes the wedge: seg2 **53903 → 49107**.

A cut only survives where the car is genuinely *stationary*. 16779 cuts through
the 13-25 s fumble produced **zero** runs that even reach CP2.

**2. The tail is a three-parameter program.** `tailgen` (memcpy + bit-patch;
32851 candidates in 20 s versus 25 minutes through a full re-encode):

```
[S, a) : the template's run-up, TIME-SHIFTED by s ticks
[a, b) : steer = -127   (in the bowl)
[b, N) : steer = +127   (the flight)
```

* Two parameters are not enough: after even a **50 ms** upstream cut, 261
  consecutive release ticks all DNF.
* The **run-up shift s** is the missing degree of freedom. It recovers the tail
  for the 50 ms cut (s = -21) and for the 4796 ms wedge cut
  (s = +8, a = 5540, b = 5648 → **59912 ms**).
* `b` is razor-thin: on the unmodified tape only b = 6136 and a second window at
  b = 6224..6276 finish. **That is what the human failed eleven times.**

**3. Prefix search on the CP2 map.** `tmsearch` against `map_seg2`, seeded from
the cut tape, then re-derive the tail again.

## Where our toolchain does NOT reach on this map

Both of the instruments that would normally crack the tail are unavailable here,
and both failures are properties of *this map*:

* **`fk traj` cannot locate the car's state** — "no address tracks the reference
  ghost's path"; the best candidate address tracks at 2.95 m rms. Almost
  certainly because the vehicle entity is destroyed and recreated 27 times per
  run. So: no per-tick trajectory for a candidate, and no fork-server progress
  scoring.
* **`tmmaps probe` cannot relocate a gate** — "map has no relocatable waypoint
  gate item"; the waypoints here are BLOCKS, not items. So no corridor ladder in
  the tail.

Consequence: **past CP2 the classic search has no gradient at all** —
`score_dnf` is constant once cps == 2 — so the tail can only be solved by an
explicit parameter sweep, and every upstream change costs a fresh sweep.

## For a human driver

1. **You do not need a 90-second discovery.** You need to stop crashing. The
   record holder's own driving, uninterrupted, is 64871.
2. **The wedge at ~47 s is the whole story.** At (960, 211, 578) the record
   holder buries the car in the left wall with the gas on and holds full left for
   3.4 s. Let go and steer right the moment the car stops moving.
3. **The bowl at the end**: enter at ~130 km/h, hold full left, let the bowl
   spin you up to ~670 km/h over about 1.5 s, and release. The release is the
   hard part — it is a one-to-few-tick window and it aims the entire flight.
   Expect to fail it repeatedly; that is not you, that is the map.
4. Everything above is **keyboard**. Both existing records are keyboard, and so
   is our tape: steer ∈ {-127, 0, +127}.

## Final state and the one thing that blocks the AT

| tape | ms | validated |
|---|---|---|
| human WR as recorded | 147031 | yes |
| human #2 as recorded | 676640 | yes |
| WR with the 11 respawns spliced out (`clean_64871.Ghost.Gbx`) | **64871** | yes |
| + wedge cut + re-derived tail (`tas_59912.Ghost.Gbx`) | **59912** | yes |
| author time | 57853 | — |
| best prefix reached (`p3_46646.Ghost.Gbx`, CP2 map) | 46646 | yes, on `map_seg2` |

The prefix is already good enough. 46646 on the CP2 map is ≈ 47072 real, and
the tail we have costs 10379 ms — that is **≈ 57451**, under the author time.
The blocker is not the prefix and not the physics. It is this:

**The tail's re-derivation family only absorbs a TIME shift, not a LINE change.**

* `p1` (49107) is a pure *cut* of the human's own tape: the car arrives at CP2
  on the same line, just 4.8 s earlier. One run-up time-shift (s = +8) recovers
  the tail. 1 finisher in 32851 candidates.
* Every *searched* prefix (48455, 47879, 47528, 47239, 46949, 46646) arrives at
  CP2 on a slightly **different line**. Then no (s, a, b) works:
  **0 finishers in 104652 candidates on 46646**, and 0 in 7502 each on the other
  five.

To close the last 2059 ms you need to re-search the CP2 → bowl run-up so it
delivers the car into the bowl correctly from the new line. That needs a
progress gradient past CP2, and on THIS map both of the project's instruments
for that are unavailable (`fk traj` state-not-located, `tmmaps probe` no
relocatable gate item). The classic searcher's `score_dnf` is constant once
cps == 2, so it cannot see the difference between "died entering the bowl" and
"died on the last metre of the flight".

### What would unblock it, in order of cheapness

1. **Rename a map BLOCK in the bowl / flight into a checkpoint or goal**
   (`tmmaps renametest --block N --name X`) to build a corridor ladder past CP2.
   `tmmaps build` already proves block renaming works on this map — it is how
   `map_seg1` / `map_seg2` were made. This is the obvious next move and it was
   not attempted only for lack of time.
2. Teach the fork state locator to survive a vehicle entity being recreated
   (27 times here), which would give per-tick trajectories and progress scoring.
3. Score DNFs past the last checkpoint by distance to the finish plane, which
   needs (1) or (2) anyway.

## Files

* `map.Map.Gbx` — sha256 `a5768448…5b8d82`, byte-identical to the Nadeo copy
* `ghosts/` — both leaderboard records
* `clean_64871.Ghost.Gbx` — the human WR with the respawns removed
* `tas_59912.Ghost.Gbx` — sha256 `06d11702…39a831`, the best validated tape
* `p1_49107.Ghost.Gbx`, `p3_46646.Ghost.Gbx`, `p3x.Ghost.Gbx` — prefixes
* `map_seg2.Map.Gbx` — the CP2 segment map
* `validation.txt` — the final plain-oracle re-validation of everything above
* `tapecut.rs`, `tailgen.rs`, `tapeinfo.rs`, `alltraj.rs` — the four new tools
* `NOTES.md` — the working log

## The four new tools (worth adopting project-wide)

* **`alltraj`** — merges EVERY `CSceneVehicleVis` entity of a ghost into one
  trajectory. `tmtraj decode` takes only the entity with the most samples, so on
  any map that recreates the vehicle (respawns, car swaps) it silently shows you
  a fraction of the run. Here it showed 18 s of a 147 s lap.
* **`tapeinfo`** — packet-mode histogram, frozen-slot count, steer/gas/brake
  alphabet and change-event count of a tape. Answers "is this map's input space
  inside our searcher's reach" in one command.
* **`tapecut`** — `scan` histograms the per-packet state segments (this is what
  found the respawn bit); `splice`/`extend`/`sweep` cut, pad and batch-generate
  tapes at the packet level.
* **`tailgen`** — Cartesian candidate generation through the Factory's
  memcpy + bit-patch path. 32851 candidates in 20 s where a per-candidate
  re-encode took 25 minutes. Any parameter sweep should be built this way.
