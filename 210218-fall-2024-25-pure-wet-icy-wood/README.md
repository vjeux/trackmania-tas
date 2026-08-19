# Fall 2024 - 25 (Pure Wet Icy Wood) — the world record falls by 0.213 s; the author time is 1.591 s away

**Author time 94.477 · human world record 96.281 · best validated 96.068.**

| tape | validated | vs human WR | vs AT |
|---|---|---|---|
| [`TAS_96068`](replays/TAS_96068.Ghost.Gbx) | **96.068** | **−0.213** | +1.591 |
| [`TAS_96078_1minimal`](replays/TAS_96078_1minimal.Ghost.Gbx) | 96.078 | −0.203 | +1.601 |
| author time | 94.477 | — | — |
| human WR, iambeeen *(control)* | 96.281 | — | +1.804 |

TMX map [210218](https://trackmania.exchange/maps/210218) · uid
`VHalgyxHqys7loscd1RQhgqp3Fe` · **30 recorded runs** · Water / Altered Nadeo /
Wood.

**Not submitted to any Nadeo leaderboard, and it never will be.**

*A search arm is still live on this map and reports 96.065; that tape is not yet
archived, so it is not published here. The figures above are what re-validated
on an independent build with a human control in the same batch.*

**The durable result on this map is not our lap time.** It is that **the sum of
per-sector minima across the field is 91.826 — 2.651 UNDER the author time** —
and that **93.847 survives** even after discarding every sector that could have
inherited speed from the one before it. Every sector of a lap comfortably inside
94.477 has already been driven by a human. Nobody has put them together.

*(Those are sums of **real driven sector times**, each one a clock reading from a
human's own lap. They are not a pointwise speed envelope, which is a different
construction and one this project has since found to be biased — see
[`FINDINGS.md`](../FINDINGS.md). What a sector-minima sum is vulnerable to is
**separability**, which is why the 93.847 figure is given alongside: it is the
same sum after discarding every sector that could have carried speed in from its
predecessor. It also clears the **provenance** gate, which is the other thing an
envelope-like figure has to pass: these are records set on this map's own uid,
not times transferred in from a sibling or an original, and **29 of the 30
re-simulate to their recorded times here** (the one exception is flagged and
excluded from the field statistics below). That gate is not academic on this
map — the official field of the map it was altered from returns `DNF cps=0` on
this geometry, so those 29 274 times are inadmissible here no matter how similar
the layout looks. **Re-simulation on the uid you are analysing is the test, not
the Altered Nadeo tag.**)*

*Two agents worked this map; the second independently re-verified the first's
96.078 in its own build tree with a human ghost in the same batch, and 48/48
repeated runs of the same tapes returned identical times. The oracle is
deterministic on this map.*

---

## This author time has no ghost in the map — but that is not evidence it was never driven

**A note published earlier on this page argued that 94.477 "should probably not
be called a driven lap". That inference is retracted.** The three signals behind
it are each still true, and they are worth stating, because they change what we
can *read* off this map:

1. **No author ghost in the map file**, though the header says `validated="1"`.
   Decompressing the 11 916 655-byte body and searching gives **zero** hits for
   `CPlugEntRecordData`, for `CGameGhost`, and for every `CGameCtnGhost` chunk
   id. Positive control with the same binary on another map: found at body offset
   607 759, and it decodes.
2. **`atSetByPlugin: true`** on unbeaten.at.
3. **The map's own author sits 4th on the leaderboard at 105.172**, 10.695 s
   slower than the author time attributed to them.

What does not follow is the conclusion. A census of that author's **other**
Pure Wet Icy Wood conversions settles it:

| TMX | author time | human WR | by | margin |
|---|---|---|---|---|
| 205229 | 79.637 | **60.114** | tuduttuduu | −19.523 |
| **208961** | 25.377 | **23.908** | **R4igekon — the author himself** | −1.469 |
| **208800** | 47.167 | **46.566** | **R4igekon — the author himself** | −0.601 |
| 210217 | 98.473 | **95.805** | kjszrqhczxn | −2.668 |
| 208804 | 77.778 | **76.372** | n00bdax | −1.406 |
| 208802 | 77.588 | **76.972** | A------------ar | −0.616 |
| 208965 | 28.941 | **28.594** | n00bdax | −0.347 |
| 208801 | 57.428 | **57.313** | iambeeen | −0.115 |
| 208964 | 35.620 | **35.584** | thgiN_ | −0.036 |

**Nine of nine of this author's other author times are beaten by a human, and on
two of them the human who beats it is the author.** Those runs re-simulate — 9 of
10 tested exact, including all five rank-1 author-time-beaters.

So this author drives his own surfaces to within about 2 % of a good human, and
**a 1.804 gap on 210218 is an ordinary margin for the series, not a freak.** His
4th place online says how much he replayed *this* conversion, not whether he can
drive its author time.

> **The absence of an embedded ghost means we cannot READ the author's line off
> this map — not that no such line exists. 94.477 is a normal target.**

That is a correction worth making carefully, because the retracted version was
the more dramatic claim and it was three-quarters right. Each signal was a real
measurement; the inference stacked on top of them was not.

## Five validated human runs on this exact surface — a reference, not a seed

The same census banked five human runs on wet icy wood, all of them beating
their own maps' author times, two of them by the author. Grafted onto 210218
with controls, they **bind and drive and then die at `cps=1`**:

| tape | on 210218 |
|---|---|
| the five translated sibling runs | **DNF `cps=1`** |
| identity control (a native ghost grafted into another native ghost) | 102.601 exact |
| native rank 1 | 96.281 exact |

The checkpoint count is present in every row, so these are *driving* failures,
not binding failures — as they must be, since each sibling is a different
campaign layout wearing the same surface.

**An answer key tells you what to optimise, not what to copy.** That is now the
third map in this repository to reach that conclusion, after
[YOU LOVE WATER](../284238-you-love-water) and
[Spaghetti Nights 2](../146612-spaghetti-nights-2).

## What this map does to a TAS toolchain: nothing works

Two measurements decide what is available here.

**The car model explains 1.6 % of yaw.** All 30 field ghosts, 96 888 samples.
Fitted per-run it is 12.4 % for the world record and 16.0 % for rank 21, so the
pooled figure is not a pooling artefact. For scale, the same measurement is 71 %
on a normal map and 2.7 % on the worst one previously seen.

**Every steering-based prior, corridor and predicate this project owns is void on
this map, and none of them were used.**

**Perturbation response is lethal, or exactly neutral, and nothing else.** One
tick changed, everything else identical, swept over the whole tape:

| operator | probes | DNF | survivors that changed the time |
|---|---|---|---|
| steer ±1 (1 unit of 254) on the incumbent | 96 | **69 %** | 3, all in the last sector, all slower |
| steer ±1 on the rank-21 keyboard tape | 174 | **55 %** | 7, all in the last sector |
| accel flip | 96 | **71 %** | 7, all after tick 8900, all slower |

There is no "slower but alive" region and almost no neutral region. A tape here
is either the one that works or it is dead.

## Field reproduction: 29 of 30 exact, zero wrong times

All 30 leaderboard ghosts re-simulated against the map fetched from Nadeo's own
endpoint (10 025 757 bytes). **29 return their recorded millisecond exactly, and
zero return a *different* finish time** — which is the failure mode that matters,
and it is absent.

The exception does not finish in the oracle. It re-downloads byte-identical, and
its ghost is structurally normal — one player record, 17 checkpoints, 3 590
samples. Flagged, excluded from the field statistics, unexplained. One anomalous
ghost with no wrong-time divergences is a healthy map with one odd record, not a
broken one.

## Do not read splits off our tapes

`tmtas splits` on our 96.078 returns `race_time = 96281` and the **world
record's** seventeen splits, byte for byte. A synthesised tape carries its
template's telemetry in the header, so a per-sector audit built on it is an audit
of the seed. Everything in the write-up that needed splits used either a
downloaded human ghost or the plain oracle. See [`FINDINGS.md`](../FINDINGS.md).

## Notes

* [`RESULT.md`](notes/RESULT.md) — the first agent: field reproduction, the absent author
  ghost, the map's structure
* [`RESULT_second_agent.md`](notes/RESULT_second_agent.md) — independent verification, the car-model
  and perturbation measurements, and the minimisation

Four search arms were still running when this was written up. If the number moves,
it moves down.

## This map is an Altered Nadeo copy of **Fall 2024 - 25**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **29 274
players** on this geometry.

**This one is a surface swap, and its field is a corridor and never a time.** `name_agree` is **0.5909** — the road was re-skinned and the structure kept, which is exactly why it scores 0.59 rather than ~0. Wet icy wood is not the surface those 29 274 humans drove, so their times say nothing about this map. It also explains why the sibling sweep on this map's author found references rather than seeds.

**And that is now measured rather than assumed.** With the graft recipe chosen properly — three-chunk here, lossless control exact at 103.915 in the same batch — the official humans still return `DNF cps=0`. On maps where `name_agree` is 0.98+, the same procedure reproduces official times to the millisecond. **`name_agree` predicts the transfer exactly**: 0.9857 → the tapes run; 0.5909 → they do not, because the physics changed.
