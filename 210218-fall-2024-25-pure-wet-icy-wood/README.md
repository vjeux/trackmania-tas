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

*Two agents worked this map; the second independently re-verified the first's
96.078 in its own build tree with a human ghost in the same batch, and 48/48
repeated runs of the same tapes returned identical times. The oracle is
deterministic on this map.*

---

## This author time should probably not be called a driven lap

Every other map in this repository has an author time that a person sat down and
hit. This one has three signals pointing the other way, and they agree:

1. **No author ghost in the map file**, though the header says `validated="1"`.
   Decompressing the 11 916 655-byte body and searching gives **zero** hits for
   `CPlugEntRecordData`, for `CGameGhost`, and for every `CGameCtnGhost` chunk
   id. Positive control with the same binary on another map: found at body offset
   607 759, and it decodes.
2. **`atSetByPlugin: true`** on unbeaten.at.
3. **The map's own author sits 4th on the leaderboard at 105.172** — 10.695 s
   slower than the author time attributed to them.

That does not change the target, and we are not claiming the time is impossible.
It means the "how would a human do this" section below reconstructs a technique
**from the field**, not from the author.

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
