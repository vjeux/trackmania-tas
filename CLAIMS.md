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

## The worked example: a check that was precise, confident, and blind

Everything above is about documents. This one is about an instrument, and it is
the purest instance of the failure this file exists for, so it is worth reading
before you trust any tool in this repo — including the ones this file tells you
to run.

`tmtraj corpus dup` answers "do two published files of one map carry the same
recorded motion?" — the defect that had two of our own tapes rendering as a
single car. It decides whether identical positions are *expected* by first
asking whether the two input tapes differ, and it asked by shelling out to
**`fk tapediff`**.

`fk tapediff` is not a command `fk` has. At any build.

```
$ fk tapediff --a A.gtape --b B.gtape
fk: ABORT: unknown command "tapediff"
```

The call failed every time. `.ok()?` swallowed the failure. And `None` from that
function means *the tapes are identical*, which makes identical positions
expected. **So every pair in the corpus came back
`identical-tapes / EXPECTED-SAME-INPUTS`, and the scan exited 0.** The check
that exists to catch one run published twice excused every pair it exists to
catch, and reported success while doing it.

Three things worth taking from it:

* **`.ok()?` is `2>/dev/null` with a nicer spelling.** The module's own header
  says the shell scripts it replaced were fragile because "every one of them
  piped a tool's stdout through awk and discarded its stderr". The Rust port
  reproduced the bug it was written to remove. Porting a pipeline does not port
  its discipline.
* **It failed toward CLEAN, and that is worse than failing toward a null.** A
  null looks like a result and gets argued with. A pass looks like nothing at
  all. This is the second time `fk` being unreachable produced a silent wrong
  answer (`tools/search/SEARCH.md` has the first, where 24 attempts "failed to
  find the car"); that one at least produced a suspicious zero.
* **A comparison needs a two-sided control.** The fix has one as a unit test: a
  tape must read identical **to itself**, and two known-different runs must read
  **different**. Either half alone passes for a broken comparison — a blind one
  satisfies the first, a noisy one satisfies the second.

**And when an instrument is repaired, its first output is not a result either.**
This one produced three wrong readings on the way to a right one, all mine, all
caught by a control:

1. Its first repaired run returned **35 refusals keyed at `diverge@-1.52 s`** —
   the countdown, before the car can act on anything. Pre-race ticks are now
   excluded.
2. Its second returned **777 refusals**, because the new "the trajectories
   separate, so it is not one run twice" arm was placed ahead of the ordinary
   shared-prefix case and swallowed 607 legitimate pairs.
3. Reading `ghost tape diff` to check a pair by hand gave "zero differences
   after the countdown" — because **`ghost tape diff` prints at most 80 rows**
   and then stops. The pair has 1041. `tmtraj tapediff` exists because of that
   and does not truncate.

## What the repaired check found, and how it was adjudicated

46 refusals, sorted — because 46 unadjudicated flags become next month's
"everyone knows those are false positives":

| n | class | how it was settled |
|---|---|---|
| **14** | **innocent, MEASURED** (203330) | every one of the 227 differing ticks falls inside that map's **measured per-tick inert window** (race 0.000–2.970, established by overwriting one tick at a time); **zero** in either live window. The one map that can prove it, proving it |
| **3** | **same recorded motion**, separation exactly 0.000000 m | 227654 ×2 and 186935 ×1. **Positive control: the 227654 page already says by hand that those files are "one trajectory, not two runs"** — the repaired check rediscovers a defect the corpus had documented independently, and adds `TAS_57518` to the set |
| **5** | documented provenance (286279) | all against the author's ghost extracted from the map, which that page says every run there was built from. `corpus splice` independently calls them CONTAMINATED. Not news, and not a new defect |
| **24** | **UNRESOLVED — inert inputs or a splice** | reported with the count of differing ticks *inside* the identical stretch, and the test that would settle it named |

The 24 are not a shrug: **one of them is a real defect and is now written up.**
210218's two published files differ by **731 input ticks** spread across the
whole run and hold **bit-identical positions for 89.95 s** — `tmtraj diff`
returns `IS-THE-REFERENCE` on the pair. At least one of those two records is not
its own tape's run.

Settling the rest needs the map, to re-simulate each tape and ask whether the
engine reproduces that file's own record. This repo does not redistribute maps,
so it cannot be done from a clean checkout — which is why they are an open task
with a named test, and not a verdict in either direction.

## Not everything needs a tag

Do not qualify what is solid. A true claim with its control cited is the best
outcome on this page and the most common one: most of what is written here is
right. Hedging a sound result is the same failure as asserting an unsound one —
the reader cannot tell which is which either way.
