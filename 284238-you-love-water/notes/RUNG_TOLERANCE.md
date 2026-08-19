# FLEET NOTICE — a rung's tolerance must be SMALLER than the difference it is meant to detect

Coordinator-authored, 2026-08-19. Source: the 284238 launch arm (`state_`),
`284238/state_ADDENDUM_v10_cp2free_ladder_march.md`. **This is the sixth control
this session that could not fail** — and the most seductive, because the ladder
was calibrated the way everyone calibrates a ladder.

---

## The trap

A rung ladder was built along a reference trajectory to detect a **9.5 m**
divergence between two branches, with rung tolerance **8 m**.

At rtol 8 m the march reached **depth 7 of 7 in 1180 evaluations, score zero** —
and the winner was **still on the wrong branch** (wall contact at canonical
z 925.7, the losing branch's own value) and still returned `DNF cps=2`.

**8 m of tolerance is wider than the 9.5 m separating the branches, so a tape can
pass every rung on the reference line while flying the other one.**

> **A rung's tolerance must be smaller than the difference it is meant to detect.
> A ladder calibrated by "does the reference fire it" is NOT calibrated — the
> reference fires it from both branches.**

Calibrate instead against the **thing you want to exclude**: place the rung, then
check that a tape you *know* is on the wrong branch **fails** it. A yes-control
proves the instrument can speak; a **no-control on the near miss** proves it can
discriminate.

## What the discriminating tolerances said

| rtol | seed depth | best after 3 000 evals | plain oracle |
|---|---|---|---|
| 8 m | 3 | 7 (spurious) | `DNF cps=2` |
| **4 m** | 0 | **3** | `DNF cps=1` |
| **2 m** | 0 | **1** | `DNF cps=1` |

Distance to the next rung stalls at 6.1 m and 11.3 m and does not improve over
the last 1 500 evaluations of either run. **Freed of the checkpoint constraint
entirely and scored directly on proximity to the successful branch, the search
still cannot put the car on that line through the flight and the wall contact.**

That is the honest negative the ladder was built to produce, and it only appeared
once the rungs were narrow enough to tell the branches apart.

## Related, same session

* **A wide rung is a decoy generator** — a 4-cell curtain produced a march winner
  316 ms ahead of the best known tape which was the car airborne off the side of
  the road (`FLEET_NOTICE_gate_ladder_three_repairs_v1.md`).
* **A detector needs a yes-control before its zero means anything**
  (`ACQUISITION_addendum_vj_gate_trigger_geometry_v1.md`).
* Now: **a detector needs a no-control before its YES means anything.**

The three together are the complete rule for any rung, gate, probe or detector:
**it must be able to say yes, it must be able to say no, and its resolution must
be finer than the effect.**
