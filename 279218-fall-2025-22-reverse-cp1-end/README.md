# Fall 2025 - 22 Reverse CP1 End — author time beaten by 3 ms

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **5347 ms** | **−3** | **−8** |
| TAS, earlier tape | 5348 ms | −2 | −7 |
| Author time (never beaten by a human) | 5350 ms | — | −5 |
| Human WR — Matik_K | 5355 ms | +5 | — |

TMX map [279218](https://trackmania.exchange/maps/279218) · uid
`_Toadb_vTfXnT7PfAIpHypSJClk` · author **in-.-** · **339 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The map

201.5 m, 5.355 s, a 559-tick tape. Two waypoints — a road start and a
free-standing finish gate at (971.3, 50, 400) — and **no checkpoints**, so a
failed run returns no information at all.

**Gas is on and the brake is off on every tick of all 40 downloaded human
runs.** Steering is the only control. The map is one long full-lock left-hander
into a short straight.

Two measurements that shaped everything:

- **The finish-gate vernier is exact and linear at 17.0 ms/m**, so **1 ms =
  1.7 cm** and the whole 5 ms gap to the author time is **8.5 cm of reach**.
  That converts a timing problem into a geometry problem.
- **All the dispersion is in the corner, 110–190 m.** Ranks 1 through 344 are
  within a few milliseconds and about a metre of each other for the first 90 m.
  Among the top 15, ranks 5 and 9 are ~2 ms *ahead* of the world record through
  the corner and lose it all in the last 20 m — **the world record wins on
  exit**, not on the corner.

## The author time is a driven lap

Medals are 5350 / 6000 / 7000 / 9000 — the gold, silver and bronze are round
thousands, i.e. placeholders, so the author time is not derived from them.
Author `in-.-` also sits **rank 5 on their own board with 5358**, eight
milliseconds slower than the time they published.

So 5350 was driven, in the editor, by a person who could retry indefinitely —
and it is 5 ms better than 339 players have managed online. A human-repeatable
technique exists, because a human executed it.

## Validation

**40 of 40 human ghosts** — the top 15 plus five each from leaderboard offsets
25 / 50 / 100 / 200 / 339, spanning rank 1 to rank 344 — re-simulate to their
exact recorded millisecond. The candidate factory round-trips rank 1 to 5355.
Rank 1 travels in every batch as the identity control, and every published tape
re-validates on the untouched map.

Worth recording: the node doing this work **died mid-investigation** with two
hours of lease left. Everything survived because the map, the ghosts and the
analysis had been banked to shared storage; the work resumed on a replacement
box and the identity control was re-run from scratch there before anything was
trusted.

## Files

| file | what |
|---|---|
| `replays/best_pF_5347_32087.Ghost.Gbx` | the run |
| `replays/best_pC_5348_32098.Ghost.Gbx` | an independent arm's 5348 |
| `notes/PLAN.md` | the full pre-search analysis, all measured on this map |

## This map is an Altered Nadeo copy of **Fall 2025 - 22**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **44 128
players** on this geometry.

This is a **Reverse** variant: same physics, but those humans drove the route **backwards**. The official field gives you geometry and a corridor, **not a line** — and nobody has yet tried reversing it into one.

**Official tapes demonstrably run on this map.** Twenty official human ghosts have been grafted onto altered copies and each returned its own official time or split to the millisecond, so this is a demonstrated pipeline rather than a statement about physics. The graft recipe is map-dependent — carry the inputs chunk only, or all three, and pick whichever one's lossless control passes in the same batch.
