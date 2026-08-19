# The Blev Special — the author time falls, and the trick is to arrive at a dead stop *sooner*

**Author time 57.853 · best validated 57.573 · the same human's own driving,
retries removed, 64.871.**

| tape | validated | vs AT | what it is |
|---|---|---|---|
| [`TAS_57573`](replays/TAS_57573.Ghost.Gbx) | **57.573** | **−0.280** | reach the wedge ~7 s early, then replay the human's own escape |
| [`TAS_57577`](replays/TAS_57577.Ghost.Gbx) | 57.577 | −0.276 | sibling from the same splice sweep |
| [`TAS_59912`](replays/TAS_59912_watchable.Ghost.Gbx) | 59.912 | +2.059 | the previous best, keyboard-only |
| [`HUMAN_WR_retries_cut_64871`](replays/HUMAN_WR_retries_cut_64871.Ghost.Gbx) | 64.871 | +7.018 | **the world record with its eleven respawns spliced out** |
| author time | 57.853 | — | — |
| human WR, Blev.. *(control)* | 147.031 | — | contains 11 respawns — see below |
| human #2 *(control)* | 676.640 | — | — |

TMX map [227654](https://trackmania.exchange/maps/227654) · uid
`bV_szgZIzzKGbW3Zujo8pjxZSC2` · author **Blev..** · **2 recorded runs** ·
DesertCar / SnowCar / Bobsleigh.

**Not submitted to any Nadeo leaderboard, and it never will be.**

*Current best, not final. The owning search is still improving — a 57.537 tail
and a 46.601 prefix on a segment map are in hand at the time of writing — and
the two mandatory follow-ups (the human-route story and the low-input family)
are in progress. Expect this page to be revised downward.*

---

## Read the gap correctly: 7 seconds, not 89

unbeaten.at shows this map with an 89.178 s gap between the author time and the
world record, which makes it look like a joke map. **It is not, and quoting
57.573 against 147.031 would badly overstate what was done here.**

The world record contains **eleven respawns**. Splice them out — which is exact,
not an approximation, because a TM2020 respawn restores the run's own checkpoint
crossing state — and the same human's own driving is **64.871**. That is the
number this result should be read against.

The real gap was **7.018 s**, and it is now closed by 0.280.

This is the same lesson as [`[Turtle Trial] Leto`](../286279-turtle-trial-leto)
and [`YOU LOVE WATER`](../284238-you-love-water), and it is worth stating as a
rule: **on any map where the clock runs through respawns, read the leaderboard
number as clean driving plus every retry before you decide the map is silly —
and before you quote your own margin against it.**

## What the map does to you: nine seconds to travel eighty metres

Decode the record's telemetry and the middle of the map is a wall. At 37.75 s
the car is at x = 1040 doing **198 km/h**. It then brakes, crawls at 20–50 km/h
for nine seconds, and finally noses into a corner at 46.9 s. Once there it is
genuinely stuck:

```
47.000 - 51.750 s   x = 959.83 ± 0.01   y = 210.96 ± 0.02
                    speed 1.7 - 3.9 km/h, steer -127, gas on
                    sliding only in z: 577.86 -> 578.88, one metre in 4.75 s
```

That is a **state collapse**. For four and three-quarter seconds the car's entire
configuration is one number — how far it has slid in z inside a wedge — and
everything the driver did before it is thrown away.

## The move: keep the tail byte-identical, arrive earlier

That collapse is also the opening. If the state at the bottom of the wedge is
one-dimensional, then the whole run after it is a *function* of the wedge, and
the human already has a working program for it. So do not re-derive the escape,
the checkpoint, the run-up, the bowl, or the 717 m ballistic arc to the finish.
**Splice.**

```
W[0, k)  ++  clean_64871[m, end)          finish = 64.871 + 0.010·(k − m)
```

where `W` is any candidate that gets wedged early and `m` is a tick inside the
human's own wedge dwell. To beat 57.853 the candidate has to be wedged about
**7.0 s earlier than the human — by ≈ 40.0 s instead of 46.9 s**. The `(k, m)`
sweep then measures how much of the dwell actually has to be matched tick for
tick, which turned out to be less than all of it.

The search that produced `W` could not use the finish as its objective — nothing
in that window finishes. It used a **relocated finish gate placed at the corner
itself** (`x = 932, y = 208, z = 578.0`, yaw 0), which fires only when the car is
at x < 964 with z ≥ 578, i.e. only when it is actually in the corner. The gate
reads out with float precision and monotonically along the road:

```
x=929 -> 53.903   (= the untouched segment map exactly, the origin control)
x=937 -> 53.620
x=945 -> 53.308
x=953 -> 52.907
x=956 -> 52.703
```

The real map with the real finish is still the adjudicator — the gate is only a
pre-filter, because a candidate could in principle clip the corner at speed and
fire it without wedging.

## Three things this map settled that transfer

**1. A respawn restores *your own* crossing state, not a canonical one.** On
[165922](../165922-idm-ruinin-ur-day-460) a respawn manufactures a state that is
identical no matter what the car was doing before it, which is what made a
transplant legal there. Here, grafting the record's own last respawn and winning
tail onto its own prefix works perfectly and lands exactly on the arithmetic
(+0.200 per +20 packets, identity control 147.031) — but grafting the *same tail*
onto a searched line finishes **0 of 31** times, and 0 of 124 over a
(k, tail-shift) sweep. Both statements are true; they are about different maps.
Do not assume portability. See [`FINDINGS.md`](../FINDINGS.md).

**2. Waypoint *count* is the thing a map may not change.** Renaming any spare
block into a waypoint model (`GateFinish`, `RoadTechFinish`,
`RoadTechCheckpoint`) breaks every run on the map — the ghost declares three
waypoints and now finds four. Renaming to a non-waypoint model is harmless
(147.031 unchanged). *Moving* CP2 keeps the count at three, which is exactly why
the ladder above is legal. And a hole left in the road cannot be plugged with a
renamed spare free block: tried three ways, all DNF.

**3. The trajectory in this ghost is split across 27 entities.** `tmtraj decode`
reports 365 samples for a 147 s run because the recording is spread over 27
`CSceneVehicleVis` entities — one per life. Merging them is what showed the run
for what it is: cross CP2 at 54.329, then twelve attempts at the final section,
eleven of them ending airborne at 460–590 km/h followed by a respawn. Reading the
stock decoder here would have described a fragment.

## Checks

| check | result |
|---|---|
| field reproduction | **2/2 exact** (147.031, 676.640) |
| author's validation ghost embedded in the map | **absent** — `validated="1"` but no `CPlugEntRecordData`; positive control on two other maps found theirs |
| encoder round-trip on the world record | 147.031 → 147.031, unchanged |
| frozen slots in the tape | **0 of 14 854 packets** — every tick is mutable |
| input alphabet of both human tapes | steer ∈ {−127, 0, +127}, gas/brake binary — **pure keyboard** |

Alternate car types change how this map *drives*. They change nothing about
whether the tape can be edited.

## Validation — three independent toolchains

```
map sha256 a5768448d61edfc32da243a74c098b18314724342f9e0ce1895a872eb05b8d82
TAS_57573  sha256 365d822130e49379ea8eb47d3c5477ab4135a7bcc60490968da2d301d694af41

blev2_tas_57573_v1.Ghost.Gbx     57573      <- the result
blev2_tas_57577_v1.Ghost.Gbx     57577
blev2_tas_57580_v1.Ghost.Gbx     57580
clean_64871.Ghost.Gbx            64871      control (the record with retries cut)
tas_59912.Ghost.Gbx              59912      control (previous best)
rank00001_147031.Ghost.Gbx      147031      KNOWN-ANSWER CONTROL (human WR)
rank00002_676640.Ghost.Gbx      676640      KNOWN-ANSWER CONTROL (human #2)
```

Seven of seven exact, on **three separate toolchains** built independently of one
another: the claiming search, an auditor who worked from the archived files
alone with hashes checked before validation and the claimant's transcript read
only afterwards, and the publishing pass. No disagreement of any kind between
them, on any row.

That level of checking was applied because this map's verdict was briefly in
doubt in the project's own records — an earlier agent's closing state said the
author time had **not** fallen, which was true when they wrote it and was
superseded within hours. The earlier note is preserved in the working record
rather than deleted.

## Still open

The low-input family. The previous agent's 59.912 is already keyboard-only and
the humans drive this map on three steer values, so the alphabet is not the
question here — the question is the **event count** of the splice tape, which has
not been minimised. `TAS_57573` is a search product, not a drivable script yet.

## Notes

* [`RESULT.md`](notes/RESULT.md) — the first session: field reproduction, the respawn
  splice that produced 64.871, and the route to 59.912
* [`METHOD_wedge_splice.md`](notes/METHOD_wedge_splice.md) — the wedge measurement, the gate
  ladder and the splice arithmetic
* [`VALIDATION.txt`](notes/VALIDATION.txt) — the oracle transcript
