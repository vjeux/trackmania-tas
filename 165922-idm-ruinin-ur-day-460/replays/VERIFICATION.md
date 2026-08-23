# 165922 — nine regenerated ghosts, verified by the integrity arm

These replace nine published files whose **vehicle entity was present and
zeroed**: the record node is there (v11, 7 descriptors) and the car in it does
not move. The original note here said *"zero entities, so there is no car in
them"* — **that wording is corrected**. It mattered: "no car" reads as
unrecoverable and "a zeroed slot" reads as regenerable, which is what these
nine turned out to be, and the same wrong word cost 186935 two days. The one
line that names it, on any file, is `tmtraj check FILE`, whose C2 prints *"the
car travels 0.0000 m over N distinct points"* — with the sample count right
beside it. The originals validated to their exact times — the oracle reads the
input archive — and could not be filmed.

Verified by `intg_accept.sh` (tools `intg_tools_v10.tgz`, F1994618338), which
tests IDENTITY rather than agreement:

| test | result |
|---|---|
| **SPAWN** — first sample at the map's spawn, from a *downloaded* human recording | 9 of 9 pass |
| **C2** — the car travels a real distance | 9 of 9 pass (the published nine: 0) |
| **the gate** — C-checks, contamination, oracle, manifest | **PUBLISHABLE, 9 of 9** |
| **alignment-free vs `rank00001_8790769`** (every integer lag) | **CLEAN, 9 of 9** |
| **cross-file** — one run published twice? | 35 shared-prefix pairs, 1 review, **0 refusals** |

The cross-file line is the one worth reading: **these are nine distinct runs**,
not one trajectory under nine names, which is the defect that affected 227654
and 238835.

## NOT cleared on two-run agreement

Deliberately. Five regenerations of one 134672 tape produced the car once and
four wrong picks, **two of which agreed with each other to the metre** — so
agreement would have rejected the only correct run as the outlier, and all five
reported 99 % coverage. A reproduction count is a majority, and a majority must
never outrank a test that can identify the answer.

## What is still inherited

Every file carries the carrier's `rpm`, `gear`, `wheel_rotation`,
`suspension_dampen`, `turbo`, `ice`, `dirt` and `wetness`, as declared in each
manifest's `fields_inherited`. Position, orientation, speed, velocity direction
and the input echo are regenerated from engine state.
