# FLEET NOTICE — test whether your sectors are SEPARABLE before you sum them. One command.

Coordinator-authored, 2026-08-19. Source: the 146612 sector arm (`w612`); its own
sidecar is `146612/w612_FLEET_SIDECAR_sector_separability_v1.md`.

**Anyone assembling a lap out of separately-searched sectors should read this
before quoting a sum.**

---

## The test

> **Run your best sector-*k* tape on `seg_{k+1}`. If it returns `cps=k`, the
> boundary is NOT separable** — the tape reaches the checkpoint and dies in the
> next sector.

## Why it matters: on one map, every boundary anyone has tested is inseparable

| boundary | evidence |
|---|---|
| 0 → 1 | a tape **29 ms faster at CP1** returns `DNF cps=1`; no tail shift rescues it |
| 2 → 3 | the best-ever s1+s2 tape (12.543) **dies in sector 3** with the donor's own sector-3 inputs — its CP3 state is 165 ms early and they cannot absorb it |
| 3 → 4 | a tape **263 ms faster to CP4 than any human** returns `DNF cps=4`; **0 finishers in 21 870 evaluations** repairing sector 4 alone, against **60 evaluations** for the joint window |
| 4 → 5 | predicted: CP5 is crossed **airborne** |

**Consequence for the sum.** A segment sum over inseparable boundaries is a much
weaker object than a normal splice bound. It is not "these times are individually
achievable and might not compose" — it is **"each of these times is achievable
and demonstrably breaks the next piece."** Say so when you quote one.

## The physical tell, and it is free to check

**Check `is_ground_contact` at the checkpoint.** A boundary in — or just before —
an air phase is the shape most likely to be inseparable, because **a ballistic
flight changes travel heading by exactly zero** (see
`FLEET_NOTICE_ballistic_heading_law_v1.md`), so the state at that checkpoint fixes
the direction of everything after it and the car has no authority to correct what
it was handed.

## What to do instead

**A backward chain of overlapping joint windows** — each window scored at the
checkpoint *after* the one it starts from, each seeded from the state its
predecessor actually produces. On that map the one piece built this way is the
piece that moved most: 328 ms in 53 seconds, against 0 finishers in 21 870
evaluations for the same sector searched alone. See
`FLEET_NOTICE_score_at_the_next_checkpoint_not_the_split_v1.md` (md5
`6598bf4a6f222dbf7e066d84d2697457`).

## And a companion result on seeds, from the same arm

Sectors 1+2, same map, same objective, two seeds:

| seed | evals | s1+s2 |
|---|---|---|
| our own tape (the line every project tape descends from) | 143 610 | 12.688 |
| **a human's line grafted from a SIBLING map** | **24 690** | **12.543** |

**126 ms better than the best pair any human on the map drives, in a sixth of the
evaluations.** The basins are 145 ms apart — so 143 610 evaluations in the
familiar basin were worth less than 24 690 in a new one. **A second basin beats a
longer run**, and a sibling map's human is the best source of one, because it is
causally independent of everything you already own.

Depth-check any such result against the **unpromoted** maps: `cps=3` on `seg4`,
`seg5` and `seg6` proves the tape satisfies the map's own CP1–CP3 triggers and is
not an enlarged-volume artefact of the promoted gate.
