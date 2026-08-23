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

## Two constants this project keeps getting wrong

Both are the same mistake — a number assumed instead of measured — pointing in
opposite directions, and both have cost a published mechanism.

**Gravity is ≈24.3–24.6 m/s², not 9.81 and not per-map.** Free fall in this
engine is linear drag in vertical speed:

    a_y = −g − k·v_y        g = 24.78 ± 0.10 m/s²,  k = 0.032 ± 0.002 /s

* **Too low:** 153527's `ADDENDUM_v2` computed a slope's gravity as
  `9.81·sin 26.6° = 4.39` and reported a car "decelerating at 2.4× the slope's
  gravity". Measured on that map from **335 free-fall stretches of the driver's
  own recording**, the median is **−24.314 m/s²** — against which the observed
  figure is **0.97×, an ordinary coast**. A whole published mechanism rested on
  a physics-textbook constant.
* **Too specific:** 285885 reported *"free fall on this map is −24.308, not the
  −25.20 measured elsewhere. Gravity here is per-map."* Both numbers are the one
  law read at different `v_y` (−25.20 ⇒ v_y +13; −24.308 ⇒ v_y −16). Withdrawn.

Three maps agree: **153527 −24.314** (335 stretches), **285885 −24.308**,
**134672 24.62 ± 0.54** — the last with its own caveat printed, that its flights
span only v_y ∈ [−18, +6] so k is unidentifiable there. **Never quote a scalar g
without the `v_y` it was measured at**, and any energy, fall or deceleration
figure should name the g it used.

**≈0.5 mm was never "the client-vs-server floor".** It is the distance between
two copies of the car in the server's own memory, and the pipeline was reading
the wrong one — MEASURED on the position half 2026-08-22, where transforming
from the right copy takes bit-identity from **0 of 455 samples to 227 of 455**.
The three maps that "agreed" at 0.489 / 0.511 / 0.501 were three readings of one
quantity. The orientation half is **open and got worse** under the same change,
so the flag is default-off and the publish path is unchanged. Full table in
`tools/README.md`.

## An exemption must quote the page it exempts

The integrity allowlist (`_integrity/intg_reference_files.tsv`) excuses files
that legitimately carry another driver's identity, and its own preamble already
says **"a name is not a contract: the page is the claim"** — it was written
after a first encoding matched `HUMAN*`/`AUTHOR*` on the filename and missed a
file. On 2026-08-22 an entry was found breaking that rule in the other
direction:

```
270051  m270051_human_shaped_4831.Ghost.Gbx  human-shaped: built to the human's line
```

That row excused the file from the identity check, and the file was carrying a
real player's account id and their personal-skin locator URL. But the page says:

> `replays/m270051_human_shaped_4831.Ghost.Gbx` — **the author time, with ±10 ms
> of slack on every input**

**"Human-shaped" is a claim about the INPUTS. "His recording" is a claim about
whose run the file is.** Reading the first as the second launders an identity
onto a file the page presents as ours.

The test, and it is cheap: **does the page say, in words, that the file IS
somebody's recording?** Applied to all 11 files the skin census flags, ten pass
by quotation — *"published as his recording, which is what it is"* (227654),
*"the author's own author-time lap"* (228607), *"Ssnake01's 49.491"* (285268) —
and 270051 is the only failure. Two independent instruments agree on that one
file: `ghost identity show` keys on the account id, `tmtraj corpus qc` on the
skin path and its storage-object URL.

## A magnitude cannot settle a question about direction

The best worked example this project has, because it is the **same correction
made twice** — withdrawn the first time, accepted the second — and the
difference is entirely in the epistemics.

`fk regen` pairs each recorded sample with an engine instant, and on five of
thirteen maps that pairing is one physics tick out. Correcting it means shifting
the pairing by a tick.

**Version one, withdrawn.** A `--recshift` flag driven by C11b, which measures
the stale-buffer distance *within* a file. It reported every regenerated file at
a clean `speed × 0.010 m` offset, and **nine files were rebuilt on that
reading**. Then somebody put a **downloaded** human ghost — one the game
recorded itself, which needs no correction — through the same instrument, and it
read the same: 267460's human WR at 0.4538 m / 45.42 m/s = 10.004 ms, 98 %
tick-shaped. **The measurement was right and the conclusion was wrong**, because
C11b reports a *magnitude*, and a magnitude cannot see which **side** of a tick
a file is on.

**Version two, accepted.** Same correction, different quantity: `ghost phase`
decomposes the residual **along** and **across** the direction of travel.

```
267859  +0.1357 m along, 0.0067 across  =  +9.83 ms
279209  +0.3603 m along, 0.0066 across  =  +9.72 ms
285268  +0.5876 m along, 0.0115 across  =  +9.96 ms
228607  -0.0000 m along, 0.0004 across  =  -0.00 ms   (a clean map)
```

Along-track displacement with nothing across it **is** a time shift; a physics
difference has both. The quantity is *signed*, so it says **late** rather than
merely **far**.

**And what makes it a verdict rather than a better guess is the negative
control** — applying the fix to a map that does not need it, and requiring it to
make that map worse:

```
map      shift 0                      shift -10      shift +10
267859   0.136277 x3                  0.000093 x3    0.272435 x3
274191   0.000482 x3  (already right) 0.920794 x3    0.917557 x3
```

> **If the fix you are considering has a sign, the evidence for it must have one
> too — and it must be paired with a case that does NOT need the fix, which the
> fix should make worse. A correction that improves every number it touches has
> not been tested against anything.**

**Corollary, and it is the n = 1 rule again:** it stays a **flag**, never a
constant. Eight of those thirteen maps measure zero. The only honest way to set
it is to require the correction to return **that map's own** control to zero.
Three instances of the same shape now: `FK_STATE_OFF` generalised from one map
and false on the second, a winning `FK_ADDR_DELTA` that landed on a `1,1,1` slot
elsewhere, and this. **A per-instance measurement promoted to a constant.**

**And it explains a "noise" nobody had explained**: 279209 lands correct one run
in three at shift 0, and 0.000541 on all three at −10. The pairing was quantising
to either side; the bias measurement was not wandering. **Anywhere a measurement
in this project is "usually right", that is now the first hypothesis to test.**

## A weighted sum cannot express a requirement

From 284238, and it cost four searches. The question was whether two demands are
compatible — can the car reach the human's crossing angle **and** his roll? All
four searches minimised a **weighted sum** of position, velocity and attitude,
and 36 restarts came back saying no, with a real measured coupling behind it
(−0.78 °/m of roll per metre across the lane, against 0.09 on the flat deck the
human uses).

**A weighted sum lets a candidate buy attitude with crossing angle — which is
exactly the trade the surface forces.** So the search settles on the surface's
own trade curve and reports it back as a frontier. Put the crossing angle and
the speed on **hard bars** — satisfied, or the candidate ranks below everything
that satisfies them — and the roll comes down to the human's own value on the
lane just published as unable to produce it.

> **A weighted sum expresses a preference, not a requirement. When the question
> is whether X and Y can be had together, a preference measures the COUPLING
> instead of the COMPATIBILITY. Make X a bar and Y the objective; never both in
> one sum.**

The tell is available before the search runs: **ask what the objective can buy
with what.** That is the same question as *"what is the laziest way to maximise
this?"*, which this project already asks of state objectives, and it is the
third distinct way an objective has lied here — after the decoy that can be
maximised without achieving the goal, and the window whose end the candidate
chooses.

Same family, smaller: **apply a bar in the frame the sampler produces.** A `vz`
bar that re-canonicalised an already-canonicalised velocity, on a −120° screw
map, gated on a quantity that is not a crossing angle at all, and ten restarts
chased `vz +55` and `+71` as improvements. **One owner per frame conversion.**

And the shape of the retraction is worth as much as the rule: **it overturned a
sufficiency claim, not a measurement.** Every number in the original stands. The
finding is now that reaching the human's state at the kicker is *possible and
not sufficient* — the two cars part 17 m later, where his wheels stay loaded and
ours goes rigid. **The stronger negative was hiding behind the weaker one.**

## What the repaired check found, and how it was adjudicated

46 refusals, sorted — because 46 unadjudicated flags become next month's
"everyone knows those are false positives":

| n | class | how it was settled |
|---|---|---|
| **14** | **innocent, MEASURED** (203330) | every one of the 227 differing ticks falls inside that map's **measured per-tick inert window** (race 0.000–2.970, established by overwriting one tick at a time); **zero** in either live window. The one map that can prove it, proving it |
| **3** | **same recorded motion**, separation exactly 0.000000 m | 227654 ×2 and 186935 ×1. **Positive control: the 227654 page already says by hand that those files are "one trajectory, not two runs"** — the repaired check rediscovers a defect the corpus had documented independently, and adds `TAS_57518` to the set |
| **5** | documented provenance (286279) | all against the author's ghost extracted from the map, which that page says every run there was built from. `corpus splice` independently calls them CONTAMINATED. Not news, and not a new defect |
| **24** | **then UNRESOLVED — now settled against the engine** | see below |

**Those 24 (38 pairs at the finer verdict) are no longer open.** Re-simulating
both tapes and asking how far apart the engine puts the two cars **on the
samples where the two records agree bit for bit** returns **35
INNOCENT-INERT-INPUTS, 1 inconclusive at 0.001 m, 2 untested, and zero
defects** — `tmtraj adjudicate`. The records agree because the car really is in
the same place: those inputs had no authority where they differed.

The two untested are 238835 and 267859, the two **turtle** maps, where no file
locates at any of 14 fork points — the car is inverted at walking pace, the
locate's velocity-consistency test has nothing to bite on, and an independent
arm diagnosed the same thing from the other direction. **That is a statement
about the instrument, not the files**, which is the whole point of this page.

**And the first version of that adjudication was wrong.** This audit published a
defect on 210218 — two files differing by 731 input ticks while holding
bit-identical positions for 89.95 s — and the engine says both are sound
(0.0001 m over 1735 samples). The reading that misled me was two whole-file
*rates* that agreed, 93.8 % and 94.1 %, which do not say whether they are the
same samples. Four further instrument errors were caught by controls before the
number above was trusted; they are in `CLAIMS-LEDGER.md` §I and each one is a
rule already on this page being relearned the hard way.

## Not everything needs a tag

Do not qualify what is solid. A true claim with its control cited is the best
outcome on this page and the most common one: most of what is written here is
right. Hedging a sound result is the same failure as asserting an unsound one —
the reader cannot tell which is which either way.

**Four passages already do this as well as it can be done.** They are the model,
and they were all written before this convention existed:

* **`203330`'s authority map.** It separates two statements people conflate —
  *"the car does not respond"* (true from 2.270 to 3.650) and *"the input has no
  authority"* (true only to 2.970) — from 561 single-tick overwrites, then says
  outright that the *mechanism* behind the four bands is **"measured but not yet
  attributed"** and that the obvious explanation fails on the fourth band.
  Measurement, inference and open question, all three labelled, in one section.
* **`285268`'s check on its own check.** *"`nearident` returned `overlap=0` with
  a mean of 1.8e308 — it compared **nothing**… its `INDEPENDENT` verdict on this
  pairing is vacuous."* A tool reporting a clean verdict having measured nothing,
  caught and named rather than quoted.
* **`270051`'s false positive.** It reads COPY on an honest pairing, and instead
  of overriding the tool the page measures the tool's own floor, gives the
  human-versus-human control (3, 4 and 6 samples against our 41), and correctly
  attributes the offset to us — two days before anyone could say what caused it.
* **`tools/LINEAGE.md`.** *"Check the constant, not the version."* It splits what
  a defect invalidates into three parts and puts the uncomfortable one in the
  middle, with the measurement that bounds each.

If a passage you are writing could be summarised as one of those, it does not
need a tag. It already is one.
