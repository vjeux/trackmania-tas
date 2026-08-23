# How to read a claim in this repo

Every load-bearing statement in these documents is in one of four states, and
you should be able to tell which at a glance. Five maps lost days this month to
statements that looked like facts and were readings.

| tag | what it means | what has to be beside it |
|---|---|---|
| **MEASURED** | an instrument was run and produced this number | **the control**, named inline: the thing that would have come out differently if the instrument were lying |
| **INFERRED** | measurement plus an argument | the argument, in one clause — "so", "which means", "at that rate" |
| **UNKNOWN** | nobody has settled it | what would settle it. An open task, not a silence |
| **SUPERSEDED** | a newer result replaces it | a pointer **forward** to the file that replaces it. Never delete the old number — rule 4 of the project |

Untagged prose is narrative, route description and driving advice. Tag the
things a future arm could act on and get hurt by.

## The rule that generated this file

**Never report a harness limit as a physics limit.** When our reader does not
find X, the honest sentence is *"we have not found where X lives yet"* — which
is UNKNOWN, and is a task. It is not *"there is no X"*, which is MEASURED about
the world and is usually false.

Five costs paid for this, all in the same month:

| what was written | what was true | cost |
|---|---|---|
| 186935: *"no `CSceneVehicleVis` entity at all"* | one entity, 15 533 samples, every position (0,0,0) — a zeroed slot, which `tmtraj check` names in one line | 2 days, file looked unrecoverable |
| 227654: *"the carrier is truncated"* | one car split into **27 entities** at the respawns, tiling 0 → 147.000; every reader takes the largest | 3 attempts read "38 s of the race is unrecordable" |
| 173691: *"the finish is on the upper deck, the lower canopy is sealed"* | ten finish gates spanning y 130…194, **both rows fire**, lowest firing 133.97 | the target was 15.7 m up, not 48 |
| 285885: *"no rotation source within 82.6 m"* | a 797-probe survey whose window could not see the airborne roll that exists | a closed lead reopened |
| 134672: *"two cosmetic wheel channels"* | not cosmetic — another player's run, driving the tyre effects in a published video | shipped |

## An absence is a special case, and it has its own bar

**"N evaluations found nothing" is not a result without a positive control that
fired in the same batch, on the same budget, through the same code path.**
Without one it is UNKNOWN, however large N is.

A control that is not in the same batch is weaker than it looks: `tmsearch`
shipped with a broken `FINISH_BASE` for weeks while every driver-side control
passed, because the driver and the search were different code (`tools/LINEAGE.md`).

Three ways a null has been manufactured in this project, all real:

* **the instrument could only read the files that agree with you** — two readers
  broke *exclusively* on the control (`Factory::build` panics on human
  recordings; `Gbx::parse` needs `lzo_init()` and only human ghosts are
  LZO-compressed). If your control is the only thing your reader cannot read,
  that is the signal.
* **the window was chosen by the candidate** — 284238's contact metric measured
  up to *the candidate's own nearest approach*, so a candidate that missed
  scored 100 %.
* **the survey could not see the thing** — 285885's rotation fan covered ±40 m
  at 5 m and reported the nearest source at 82.6 m; the source that exists is an
  airborne roll inside the window it did not sample.

## Two more shapes worth naming

**A generalisation from n = 1.** `FK_STATE_OFF` was documented as "a fixed
offset from the server base" on one map and is false on the second one tested.
A constant measured once is MEASURED *on that map*; that it ports is INFERRED,
and the inference is usually the weak part.

**A number that survives its own retraction.** This repo's known failure mode is
a headline in a directory contradicted by a newer file with an older-sounding
name — 227654's `RESULT.md` and `RESULTS-entry.md` are the worked example.
**Read a store directory by mtime, never by filename.**

## Not everything needs a tag

Do not qualify what is solid. A true claim with its control cited is the best
outcome on this page and the most common one: most of what is written here is
right. Hedging a sound result is the same failure as asserting an unsound one —
the reader cannot tell which is which either way.
