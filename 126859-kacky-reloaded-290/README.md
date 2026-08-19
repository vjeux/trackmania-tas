# Kacky Reloaded #290 — author time beaten by 600 ms

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **23462 ms** | **−600** | **−880** |
| validated intermediate | 23498 ms | −564 | −844 |
| Author time (never beaten by a human) | 24062 ms | — | −280 |
| Human WR | 24342 ms | +280 | — |

TMX map [126859](https://trackmania.exchange/maps/126859) · 22 recorded runs ·
24 seconds.

**Not submitted to any Nadeo leaderboard, and it never will be.**

Validated with `IsValid: true` and `NbRespawns: 0` — worth stating explicitly on
a Kacky map, where respawn-heavy play is the norm: this is a clean run through,
not a respawn-assisted one.

## What a Kacky map is, and why it needed checking first

Kacky maps are precision content built around a few brutal obstacles, usually
with respawns as the intended mechanic. That makes two questions decisive before
any search:

- **Does the field reproduce?** (§8 — yes here.)
- **Are respawns part of the route?** The validator prints `NbRespawns` in both
  the declared and the validated result and will happily re-simulate a run
  containing them — on another map in this collection, a human record with
  **198 respawns** re-simulated exactly. So a respawn-free result is a
  *property of this run*, not a constraint of the tool.

## Files

| file | what |
|---|---|
| `replays/TAS_23462_v1.Ghost.Gbx` | the run |
| `replays/TAS_23498_v1.Ghost.Gbx` | earlier stage |
| `notes/VALIDATION.md` | the oracle transcript |
| `notes/PLAN_v1.md` | the pre-search analysis |

Work on the human-reproducibility half is ongoing; the obstacle-by-obstacle
guide will follow.
