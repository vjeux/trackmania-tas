# YEET Fall 2024 - 04 — the author time falls by 694 ms, on a map we had written off

| | time | vs AT | vs human WR | alphabet |
|---|---|---|---|---|
| **TAS** | **10.640** | **−0.694** | **−1.443** | analog |
| **keyboard flight** | **10.743** | −0.591 | −1.340 | **3 values, 14 presses** |
| action-key variant | 10.717 | −0.617 | −1.366 | small ladder |
| Author time (never beaten by a human) | 11.334 | — | −0.749 | — |
| Human WR | 12.083 | +0.749 | — | — |

TMX map [203072](https://trackmania.exchange/maps/203072) · **272 recorded
runs** · 1 checkpoint.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## This map was on the abandoned list, and the verdict was wrong

The project's own acquisition guide named 203072 as *the* canonical unusable
map — human ghosts would not re-simulate, so it was declared unfalsifiable and
no search was ever run on it. Re-examining that verdict was most of the work
here, and it turned out to be wrong in three separate ways:

- **The oracle is faithful on this map: 1.7 mm** position RMS against ghosts'
  own telemetry, on three ghosts from three different game builds. (The
  precedent that exonerated another map was 8 mm.)
- **Nadeo's own map file is sha256-identical to ours**, which kills the "the map
  was edited in place" theory outright.
- **The failures are a bounded window of game builds**, 2026-01-18 → 2026-04-28.
  Checked across the whole field, n=270 of 272: **outside the window, 80/80 —
  100%. Inside it, 92/190.**

The earlier "old builds fail, new builds pass" reading came from a **34-ghost
sample in which the post-window builds had n=1.** The full-field check inverted
it.

**One structural caveat, stated plainly: the human world record was set inside
that window and does not re-simulate for us**, so every seed used here came from
outside it.

## What 272 people are missing: everybody climbs

Our line sits **7.7 m from the world record's own path** — this is not a route
discovery. It is what the car does in the air.

Thrust on this map is **71–76% along the car's nose** (unlike the other YEET map
in this collection, where it fires through the belly). So attitude decides
whether the boost is spent going *up* or going *forwards*:

| | apex | what the boost does |
|---|---|---|
| human WR | **99.3 m** | 1.5 s of it is spent coming back down |
| this run | **81.8 m** | spent going forwards |

## The trick is a combination lock, which is why no human found it

The final tape differs from its predecessor in **three places**: a 40 ms
throttle lift, a 10 ms throttle lift, and one wheel you *do not* unwind into the
launch.

A full 2³ factorial was run on those three changes:

> **Every proper subset either DNFs or is slower. All three together are worth
> 566 ms.**

No incremental refinement finds that, human or machine, because **the gradient
points away from the answer in all three directions**. You have to make three
apparently-wrong changes at once.

This is the third map in this collection where the winning move is a
non-separable interaction — see [`FINDINGS.md`](../FINDINGS.md).

## Also measured

- **An 840 ms dead zone** in the early flight where steering does *literally*
  nothing: 15 of 15 constant substitutions return the identical millisecond.
  Everywhere else the map is hypersensitive.
- **Best-of-field splicing does not work here** — 60 splices, 10 donors × 6
  boundaries, all DNF. Not a container-portability problem: the lap is a single
  integral, and no donor's state is compatible mid-flight.

## Two mistakes worth publishing

- **The search window was set from the telemetry rather than from the tape.** The
  tape's clock offset is **−1560 ms**, so the finish is at tick 1380, not 1224 —
  which **froze the last 1.5 seconds of every candidate for 45 minutes**.
  Unlocking that region is what produced the breakthrough. The project's brief
  warns about exactly this trap, and it still cost an hour.
- `pkill -f` killed the agent's own shell twice — the second time because a
  `sed` had put the script's own name onto its command line.

## Validation

Every reported tape cold-re-validated in a second process with a different
binary, known-answer controls in the same batch. **Zero phantoms all session.**
Raw server output confirms `NbRespawns: 0`, `NbCheckpoints: 1`, correct MapUid.

## Files

| file | what |
|---|---|
| `replays/TAS_10640.Ghost.Gbx` | the fastest run |
| `replays/KEYBOARD_10743.Ghost.Gbx` | **3 values, 14 presses — the one to study** |
| `replays/ACTIONKEY_10717.Ghost.Gbx` | small action-key ladder |
| `notes/SECTION8-RESOLVED.md` | **why the map was wrongly abandoned, and the full-field build analysis** |
| `notes/RESULT.md`, `notes/RESULT-amendment.md` | the write-up and the 10.640 amendment |

## This map is an Altered Nadeo copy of **Fall 2024 - 04**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **600 000
players** on this geometry.

Geometry **and** surface are preserved (`name_agree` 0.9914), so those humans drove the same car over the same road: their times are directly comparable and their lines are usable as references.

**Official tapes demonstrably run on this map.** Twenty official human ghosts have been grafted onto altered copies and each returned its own official time or split to the millisecond, so this is a demonstrated pipeline rather than a statement about physics. The graft recipe is map-dependent — carry the inputs chunk only, or all three, and pick whichever one's lossless control passes in the same batch.
