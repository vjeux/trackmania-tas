# The ghosts in this repository, audited

**Three defective files, on two maps, out of 174. Twenty-five more cannot be
tested at all, for four distinct reasons — and none of those reads as clean.**

On 2026-08-20 we found that some of the replay files published here carried
another driver's telemetry. This page is the full accounting, including the
corrections we had to make to our own instruments along the way.

## What was wrong, and why it could happen at all

A TrackMania ghost declares the map it was recorded on, so a synthesised run
needs a container bound to that map — and the only native containers for map X
are ghosts **humans recorded on X**. So every file here began as a downloaded
human ghost with our input tape grafted in. The input archive is replaced; the
*telemetry* record starts out as theirs, and a regeneration pass from live
engine memory is the only thing that makes it ours.

That pass had a bug. Its write loop skipped any sample with no engine instant —
`missing.push(ms); continue;` — leaving the donor's bytes in place at that index
and reporting success. A file could be perfect for six seconds of a ten-second
run and nobody would see it, because the seam is invisible unless you compare
against the human's own recording.

**The oracle reads the input archive, and the input archive was never spliced.
The times are unaffected. What was contaminated is the telemetry a viewer sees.**

## The verdict

| map | file | finding |
|---|---|---|
| **227654** | `TAS_57493`, `_57498`, `_57573`, `_59912_watchable` | rank 1's telemetry, 365 of 365 samples in race |
| **227654** | `TAS_57503` | 364 of 365 in race |
| **286279** | `AUTHORMIN_831ev_354781` | 707 m of shared path in race |
| **279218** | `TAS_5345_starttrick` | the human r001 5.355, 112 of 112 |

`227654/HUMAN_WR_retries_cut_64871` is rank 1's run **by intent** — it is
published as the human's lap, and matching it is correct.

**Untestable, for four different reasons — recorded as untested, never as clean:**

| category | files | where |
|---|---|---|
| no car in the file at all | 9 | 165922 |
| the only reference **is** the donor | 5 | 238835 |
| no human recording held | 2 | 134672 |
| the map has **no human records at all** | 2 | 276874, 276877 |

- **238835** — five files are rank 1's telemetry, and the map's only human
  recording *is* the donor. There is nothing independent to grade a repair
  against, so the map is refused rather than fixed.
- **227654** — same shape: all 43 of our files on that map inherit rank 1, and
  the only other record is an eleven-minute respawn run that cannot carry a
  57-second tape. It needs a third recording from any source.
- **165922** — nine files contain no `CSceneVehicleVis` entity at all: every one
  exactly 5019 bytes, nine distinct md5s, nine distinct input tapes, and no
  telemetry section whatsoever. **They cannot be contaminated — there is no
  carrier telemetry in them to be wrong.** They are also the cheapest repair in
  the corpus: nothing to preserve, nothing to compare against, no risk of the
  chooser picking a donor.
- **134672** — no downloaded human recording is held.
- **276874, 276877** — zero-record maps. No human has ever driven them, so no
  reference recording can *ever* exist and these files can never be certified by
  comparison. Their evidence is their provenance manifest and nothing else.
  **Both of these are pages that carry a video.**

**143 files are clean against a human recording**, and three more are a human's
run *by intent* — `227654/HUMAN_WR_retries_cut`, `228607/AUTHOR_LAP`, and
`186935/ONE_ATTEMPT_DELETED`, which is the record holder's own run with a fall
removed. Matching the human is correct for those three.

228607's ten files, which an earlier pass could not compare at all, are **clean** —
tested alignment-free against the author's own validation lap, with not one
bit-identical position.

## The corrections we made to our own instruments

Every one of these was a check that was wrong about data that was right.

**A shared prefix proves nothing.** Two runs of one map with the same opening
inputs produce identical positions under a deterministic simulation — our own
sibling tapes are 67 % identical to each other. The proof of a splice is
**re-convergence**: once two runs are 147 m apart they are different physical
states, and no input sequence returns them to *exactly* zero.

**Bound the comparison to the race.** Three files flagged as contaminated were
tail overlap — two independent runs that share a carrier agree exactly in the
post-finish tail, because neither is driving there. In race they had zero
identical samples.

**A teleport is not a distance, it is a speed the car did not have.** Our jump
detector measured metres and called an 805 km/h Kacky run a teleport. Against the
car's own recorded speedometer, **every one of the corpus's 293 flagged jumps
reclassifies: 296 respawns, one origin placeholder, and zero splices.**

**Separating a respawn from a splice took four attempts, and the failures are
the interesting part.** "What the car does in the second after the jump" does not
work: on a Trial map a respawn lands and drives away at 9.4 m/s, and so does the
far side of a splice. Three tests each catch part of the class — the speedometer
reading zero (8 cases), arriving at a standstill (41), returning to a point the
car already occupied (244) — and none catches all of it. What was left sat at
233.7 m/s, inside the ordinary respawn band, and refused a genuine human
recording. The rule that works is contextual: **a run that demonstrably respawns
has its other jumps of the same size calibrated as respawns.** That is
deliberately conservative and can miss a splice disguised as a respawn on a
respawning map — accepted, because contamination has two other instruments and a
teleport has only this one.

**Absence of signal is not evidence of correctness — and not evidence of a fault
either.** Six times in one evening a check reported nothing and was read as a
verdict: a crash that printed no failures read as a pass; an empty result set read
as zero failures; "cannot test" read as fine; a comparison that stopped at sample
zero read as clean; a tool that wrote no file because there was nothing to do read
as a failure; and a wheel test that only examined ground-borne samples declared a
descending car's wheels dead — in Nadeo's own downloaded recording.

## What is in place now

- The regeneration writer **refuses a partial write** instead of silently
  inheriting donor bytes, and reports per-file coverage.
- Every ghost carries a **provenance manifest**: source inputs, container donor
  and its md5, the engine run, which fields are regenerated and which inherited,
  declared and oracle-validated times. A file that cannot be certified says
  `UNCERTIFIED` in a mandatory block — never `CLEAN` — with the reason printed as
  loudly as a failure.
- The publish gate runs **before** publication, and includes the contamination
  test, the corrected jump detector, and a re-simulation of the written tape.
- Videos are filmed **only from regenerated ghosts**. One clip was published from
  a raw tape that passed every check we owned and was still footage of the
  carrier's run, 17 m from where the car actually goes; it was withdrawn.
- The audit's controls are 20 checks with eight negative controls, including:
  fast driving is not a teleport, a genuine respawning human run passes, no splice
  is claimed on a respawning Trial run, and tail-only overlap is not
  contamination.

## One known defect that is not fixed

The ground-contact byte in a regenerated ghost is still the carrier's. Three
independent attempts to recover it failed, and the third explained the first two:
**it is a two-bit field, not a flag.** Bit 0 is contact; bit 1 is live on a third
of samples and has never been attacked; a scalar fit is fitting the sum of two
independent bits and cannot succeed in principle. So a regenerated ghost may show
ground effects in mid-air. It is named per file rather than guessed at, because a
car with a known wrong flag is better than one with an invented one.

## A note on how the jump detector was calibrated

It needed four attempts, and the discarded ones are worth recording because two
of them refused the *same* genuine human recording by different routes.

- A **distance** threshold called an 805 km/h Kacky run a teleport.
- A **200 m/s** bar sat inside the ordinary respawn band (153–213 m/s) and
  refused `286279/HUMANCUT_236972`, a real human's run.
- "What the car does in the second after the jump" does not separate the
  classes: on a Trial map a respawn lands and drives away at 9.4 m/s, and so
  does the far side of a splice.
- Three narrower tests each catch part of the class and none catches all —
  the speedometer reading zero (8 cases), arriving at a standstill (41),
  returning to a point the car already occupied (244). The residue sat at
  233.7 m/s and refused `HUMANCUT_236972` again.

The rule finally adopted is contextual: **a run that demonstrably respawns has
its other jumps of the same size calibrated as respawns.** It also needs an
absolute distance floor, because a 0.20 m shuffle at 0.8 m/s trips a pure ratio
test and reads as a splice.

That rule is deliberately conservative — it can miss a splice disguised as a
respawn on a respawning map. The reason for erring that way is worth stating:
**contamination has two other instruments, and a teleport has only this one.**

The mechanism behind it, found while calibrating: a respawn **restores a saved
checkpoint state**, so the speedometer that looked "frozen" on one file is not
frozen but *restored*. That gives a physical statement rather than a threshold —
*a respawn returns the car to a place it has already been* — and the distance
from each landing to the nearest earlier sample of the same run separates the
classes by three orders of magnitude.

**And one honest limit on all of it.** After the reclassification the corpus
contains **zero splices**, which means the detector's negative control was
mislabeled: the file we had been treating as the known splice, `227654/TAS_57503`,
turns out to open with an origin placeholder rather than a jump. So the jump
exemptions are **unfalsified, not validated** — we checked that the rule refused
things, and not that what it refused was the thing we named. Anyone extending
this should treat a genuine splice as untested territory and re-derive the
control when a real one appears.
