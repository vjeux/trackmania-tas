# The Blev Special — the author time falls, and the trick is to arrive at a dead stop *sooner*

**Author time 57.853 · best validated 57.503 · the same human's own driving,
retries removed, 64.871.**

| tape | validated | vs AT | what it is |
|---|---|---|---|
| [`TAS_57503`](replays/TAS_57503.Ghost.Gbx) | **57.503** | **−0.350** | reach the wedge ~7 s early, then replay the human's own escape |
| [`TAS_57518`](replays/TAS_57518.Ghost.Gbx) | 57.518 | −0.335 | |
| [`TAS_57537`](replays/TAS_57537.Ghost.Gbx) | 57.537 | −0.316 | |
| [`TAS_57573`](replays/TAS_57573.Ghost.Gbx) | 57.573 | −0.280 | the first tape to beat the author time here, and the independently re-verified figure |
| [`TAS_59912`](replays/TAS_59912_watchable.Ghost.Gbx) | 59.912 | +2.059 | the previous best, keyboard-only |
| [`HUMAN_WR_retries_cut_64871`](replays/HUMAN_WR_retries_cut_64871.Ghost.Gbx) | 64.871 | +7.018 | **the world record with its eleven respawns spliced out** |
| author time | 57.853 | — | — |
| human WR, Blev.. *(control)* | 147.031 | — | contains 11 respawns — see below |
| human #2 *(control)* | 676.640 | — | — |

TMX map [227654](https://trackmania.exchange/maps/227654) · uid
`bV_szgZIzzKGbW3Zujo8pjxZSC2` · author **Blev..** · **2 recorded runs** ·
DesertCar / SnowCar / Bobsleigh.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## Read the gap correctly: 7 seconds, not 89 — and the cut is EXACT

unbeaten.at shows this map with an 89.178 s gap between the author time and the
world record, which makes it look like a joke map. **It is not, and quoting
57.518 against 147.031 would badly overstate what was done here.**

The world record contains **eleven respawns**. Splice them out and the same
human's own driving is **64.871**. That is the number this result should be read
against, and the real gap was **7.018 s**.

**The splice is exact rather than approximate, and that is the point.** A
respawn restores the crossing state exactly, so deleting an entire retry span is
arithmetic:

```
finish = base − 10 ms × (packets deleted)
```

All eleven respawn packets on this record sit inside the single span the cut
removes. Audited here with the packet enumeration rather than assumed:

```
rank00001_147031.Ghost.Gbx    11 respawns   at packets 6203,6913,7474,8195,9262,
                                            9962,10799,11496,12198,12951,13698
clean_64871.Ghost.Gbx          0 respawns
blev2_tas_57518_v1.Ghost.Gbx   0 respawns
```

So the retry schedule was stripped before any search ever saw it — which matters
for more than tidiness, because a search that writes steer, gas and brake
**cannot see or remove a respawn** and would have inherited every one of them.

This is the same lesson as [`[Turtle Trial] Leto`](../286279-turtle-trial-leto)
and [`YOU LOVE WATER`](../284238-you-love-water), and it is worth stating as a
rule: **on any map where the clock runs through respawns, cut the retry spans
first. It is exact, it is cheap, and it usually dwarfs anything the search will
find** — here it was 82 of the 89 seconds.

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
TAS_57503  sha256 3df9108d6d94b1f325be71c4ba8c8b2c790c15f99838763618c5d19ad024f50c

blev2_tas_57503_v1.Ghost.Gbx     57503      <- the result
blev2_tas_57518_v1.Ghost.Gbx     57518
blev2_tas_57537_v1.Ghost.Gbx     57537
blev2_tas_57573_v1.Ghost.Gbx     57573
clean_64871.Ghost.Gbx            64871      control (the record with retries cut)
tas_59912.Ghost.Gbx              59912      control (previous best)
rank00001_147031.Ghost.Gbx      147031      KNOWN-ANSWER CONTROL (human WR)
rank00002_676640.Ghost.Gbx      676640      KNOWN-ANSWER CONTROL (human #2)
```

Every decisive row was re-run **singly** — `--jobs 1`, one ghost per invocation,
a fresh process each time — as well as in a batch, and the two agree on every
row. Reproduced here on a separate toolchain the same way, with both human
records exact.

Nine of nine exact, on **three separate toolchains** built independently of one
another: the claiming search, an auditor who worked from the archived files
alone with hashes checked before validation and the claimant's transcript read
only afterwards, and the publishing pass. No disagreement of any kind between
them, on any row.

That level of checking was applied because this map's verdict was briefly in
doubt in the project's own records — an earlier agent's closing state said the
author time had **not** fallen, which was true when they wrote it and was
superseded within hours. The earlier note is preserved in the working record
rather than deleted.

## No low-input finisher — and this is now a real negative, not an open question

An earlier version of this page said there was no keyboard finisher *yet*, and
was careful to call that a statement about how little we had searched rather
than about the map: 436 keyboard grafts reached the bowl launch **6.16 s ahead
of the human** and died on the arc, and the analog family had hit the identical
wall until its graft alignment was swept at one-tick resolution — **7 826
grafts**. The low-input search had not had that sweep. That wording is now
retired, because the sweep has been run, at triple the depth:

| | grafts | launch-reaching | release candidates | finishers |
|---|---:|---:|---:|---:|
| earlier state (under-swept) | — | 436 | — | 0 |
| the sweep, k ∈ [4890, 4990] step 1, j−k ∈ [560, 800] step 1 | **24 341** | **1 782** | **19 601** † | **0** |
| cumulative low-input campaign on this map | | | **79 840** | **0** |

† the release tick `b` swept every tick on the 400 earliest launch-reaching
grafts.

So the dimension that unlocked the analog family — graft alignment at one-tick
resolution — has now been swept here three times as deep as it was there, and it
produced four times the entries into the bowl (1 782 against 436) and no
finisher at all.

**And the negative is defended against the instrument.** The failure mode that
would make all of this meaningless is a harness that never had a chance of
finishing: a mis-built tape, a wrong map, a graft that desyncs at the start. Any
of those shows up as `cps=0` or `cps=1`. Across all **80 434** rows of this
campaign there is **not one `cps=1`** — every single negative reads *reached
some checkpoints (2 of 3)*, meaning the tape drives the map, reaches the bowl,
and dies on the arc, which is exactly the failure the analysis predicts. Analog
batches from the *same* pipeline carry hundreds of finishers. The pipeline works;
the route does not exist in this family.

> **"We didn't find one" and "it isn't there" are different claims, and the
> difference is a sweep plus a control that could have failed.**

The published time is unaffected: **57.503 stands**, from the analog family.

### Why the zero is the expected answer here

Nor is the alphabet the interesting question on this map: the humans already
drive it on three steer values, and so does the previous best at 59.912. And
there is a reason the low-input family may not help even when it lands —
**the binding input is the launch**, which is keyboard in *every* family
including both humans'. Its release window is **three ticks**, and across 355
measured bowl entries **not one is wider**. Making the rest of the tape sparser
does not touch the input that decides the run.

That makes this the fourth map where "fewer inputs is easier" fails, and the
fourth *distinct* mechanism — see [`FINDINGS.md`](../FINDINGS.md).

## Notes

* [`RESULT.md`](notes/RESULT.md) — the first session: field reproduction, the respawn
  splice that produced 64.871, and the route to 59.912
* [`METHOD_wedge_splice.md`](notes/METHOD_wedge_splice.md) — the wedge measurement, the gate
  ladder and the splice arithmetic
* [`VALIDATION.txt`](notes/VALIDATION.txt) — the oracle transcript
