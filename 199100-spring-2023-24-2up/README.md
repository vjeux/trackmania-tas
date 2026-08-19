# Spring 2023 - 24 (2-UP) — author time beaten by 1.4 seconds

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **50195 ms** | **−1407** | **−2007** |
| validated intermediate | 50224 ms | −1378 | −1978 |
| Author time (never beaten by a human) | 51602 ms | — | −600 |
| Human WR | 52202 ms | +600 | — |

TMX map [199100](https://trackmania.exchange/maps/199100) · tags **Reactor,
Plastic, Altered Nadeo** · **only 5 recorded runs** · 51 seconds.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## Field reproduction, and why the build matters

§8 check: **5 of 6 records reproduce exactly.** The sixth is the only pre-2026
build in the set — and that pattern is now understood project-wide.

On another map, 10 of 15 ghosts failed to re-simulate and **every single failure
came from the 2022 game build**, while all five from 2025–26 builds reproduced
to the millisecond, with the oracle tracking a recent ghost's own telemetry to
**8 mm over 68 seconds**. The mechanism is chaotic sensitivity: on that map a
**one-unit (1/127) steering change on a single 10 ms tick** is enough to DNF the
run. An old ghost need only differ by the smallest expressible amount, once.

So a §8 shortfall has two very different meanings, and they must be
distinguished before condemning a map:

- **build-correlated** (old ghosts fail, recent ones exact) → the map is fine;
  exclude the old ghosts and say so.
- **not build-correlated** (recent ghosts fail too, especially the world record)
  → the map is unfalsifiable and should be abandoned.

This map is the first case.

## Files

| file | what |
|---|---|
| `replays/A3_50224.Ghost.Gbx` | validated run |
| `replays/A2_50738.Ghost.Gbx` | earlier stage |
| `replays/K1a_51575.Ghost.Gbx` | keyboard-constrained variant |
| `notes/PLAN_v1.md` | the pre-search analysis |

Work on this map is ongoing; the write-up and driving guide will follow.
