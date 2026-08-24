# The Magnet Trial

**The Magnet Trial** — TAS **793.893** (−1746.748) | AT 2540.641 | WR 2575.154 by keby

https://github.com/user-attachments/assets/2c4e4a87-fd37-471d-b709-084e9aa2422c

Thirteen minutes and fourteen seconds, and the whole run is in it. The panel is
this run's own inputs from the 10 ms input chunk.

**One car, and that is a departure from this project's filming rule**, which is
that both runs go in one scene with the human as the opponent. It is not a
preference. The MediaTracker renders a clip as long as its longest entity block,
and the block for keby's imported ghost reads 2575.15 s however the file is
trimmed — record span, declared time, checkpoint list and every entity all say
796.000 and the block still says 2575.15. The two-car scene therefore renders
2575 s of video for a 793.893 s run: **5 h 43 m by the game's own estimate**.
With one ghost the clip is 793.9 s, which is the run.


**You do not need to drive this map faster than the record holder does — you
need to fall off it less. Their gap to the author time is 34.513 s, and eight of
their failed attempts each lasted longer than that.**


> ### There was a car in the file all along, and it was parked at the origin
>
> This page said `BEST_793893` carried **no `CSceneVehicleVis` entity at all**.
> That was wrong, and the wrong diagnosis is why it sat unfixable for days: the
> file has one, 15 533 samples spanning 0.000 → 793.850 — and every position in
> it is `(0, 0, 0)`. A zeroed memory slot, not a missing entity. One line of
> `tmtraj check` says so: *"the car travels 0.0000 m over 1 distinct points"*.
>
> **It is repaired.** `replays/BEST_793893.Ghost.Gbx` is regenerated from engine
> state: 15 878 samples on its own 50 ms grid, 10 881.6 m of driven path, span
> 0.000 → 793.893, first sample on the start block to 0.000 m and 0.008° of
> keby's own recording. The oracle re-simulates the written file to 793.893.
>
> Eleven per-sample channels this pipeline cannot yet read out of the engine —
> rpm, per-wheel ice and dirt, ground contact, gear — are written as **zero**
> rather than inherited from the container, and named as such. The line and the
> speed are this run's; the tyre and contact effects are absent rather than
> somebody else's.
>
> **Re-shot 2026-08-24, and the line above about there being no video is
> retired** — the clip at the top of this page is that render, and it is the
> longest in the project at 793.900 s.
>
> **It is shot on the COCKPIT camera (`shootctl --cam 1`), and that is part of
> this map's treatment: reuse it.** This is a magnet map, so the car spends
> most of the run on walls and ceilings, and the stock chase camera (`--cam 2`)
> keeps the WORLD's up-vector — you watch an upside-down car in a level world
> and cannot read what it is doing. The cockpit camera rolls WITH the car, so
> ceiling-driving reads as driving.
>
> **Picked by looking, not by argument.** Three partial renders, same three
> instants, `clip frames --stream`:
>
> | | race 4.000 | race 8.000 |
> |---|---|---|
> | `--cam 2` External, the old default | car visible but small and far, reads as a car glued sideways to a pillar | **no car in frame at all** |
> | `--cam 6` Ext2 | the best single image on this map — closer, livery legible, wall-driving obvious | **no car in frame at all** |
> | **`--cam 1` Internal** | nose and front wheels, road ahead legible, world rolled with the car | **car and road both in frame** |
>
> Cam 6 takes the prettiest frame here and it is not the one to use: at race
> 8.000 **both** external cameras lose the subject entirely, which is this map
> rather than the mode. The cockpit never does. The trade is that you do not see
> the car's own body — on a magnet map the information is the attitude and the
> line, and neither external camera can hold either.
>
> `ghost verify` on the filmed file: kappa **1.000** (15878 of 15878 samples),
> the plain oracle re-simulating the WRITTEN file to **793.893**, telemetry
> 0.000 .. 793.850 inside a span ending 793.893. Trajectory worst **0.0462 m**
> against the file it replaces, over 10.9 km of driving.
>
> Encoded at **crf 32**: GitHub's attachment store refuses anything over 100 MB
> and 794 s at the default crf 19 does not come close to fitting. The render
> itself took about forty minutes and came out **793.866 s** — its own length,
> not the map's 2540.641 author ghost, which corrects a claim this project's
> `RENDER-BOX.md` briefly carried.

| run | time | vs author time |
|---|---|---|
| **best** | **793.893** | **−1746.748 (−68.8 %)** |
| the sixteen sector cuts, before input minimisation | 795.034 | −1745.607 |
| **the human record with one attempt removed** | **2501.894** | **−38.747** |
| Author time (never beaten by a human) | 2540.641 | — |
| Human WR — keby | 2575.154 | +34.513 |

Map 186935 · author **Taxonomon** · **7 recorded runs** (board 2026-08-24,
unchanged) · 16 checkpoints and a
finish.

## Where the time is

The record is not a slow lap, it is a survival problem. **68.5 % of its recorded
time is failed attempts** — 221 respawn presses, 115 separate events, 106 of them
from a standstill, spread over 25 obstacle regions. Take the same driving with
every failure removed and it is worth **792.431**, so the pace was never the
issue.

The map concentrates almost all of it in a handful of places:

| | |
|---|---|
| one magnet climb, at (1024, 315, 716) | **639.194 s across 35 attempts — a quarter of the entire race** |
| the five worst obstacles together | **51.0 %** of the run |

That single climb is the map. Everything else is a rounding error next to it.

And the author time is closer than the leaderboard makes it look. keby is
34.513 s outside it. **Delete exactly one fall — a 73.260 s loss — and their run
finishes in 2501.894, beating the author time by 38.747 s with every other input
untouched.** Eight of their failed attempts individually cost more than the gap.
So: learn the magnet climb, get through it on fewer tries, and the author time is
gone.

**The author time on this map measures patience, not driving.**

## The fast run

The 793.893 is keby's own driving with sixteen sector-length failures cut out of
it, then trimmed from 20 365 input events to 16 397 — which made it a further
1.141 s faster, because on a magnet trial removing a steering change lets the car
hold a line it was being steered off. **No driving search was run at all.** There
is no faster line here to copy, only a cleaner run of the same one.

The sector-by-sector guide for this map is not written yet.

## How forgiving it is

Not measured in a form a driver can use. What is known is that the finished run
is not brittle: of 1 019 single inputs deleted from it one at a time, 235 still
finished inside the author time, and none of the deletions made it slower — a
change here either costs nothing or ends the run. Much of the tape is
walking-pace magnet climbing where a single 10 ms input is not load-bearing.

## Files

| file | what |
|---|---|
| `replays/BEST_793893.Ghost.Gbx` | **the best run** — regenerated, its own telemetry, one car in the file |
| `replays/CUT_795034.Ghost.Gbx` | the sixteen sector cuts, before input minimisation |
| `replays/ONE_ATTEMPT_DELETED_2501894.Ghost.Gbx` | **keby's own run with one 73.260 s fall removed — under the author time** |
