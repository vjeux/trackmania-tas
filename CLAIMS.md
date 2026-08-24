# How to read a claim in this repo

Five map investigations lost days this month to statements that looked like
facts and were readings. This file is the convention that came out of that, and
the evidence for why it is worth the trouble.

**If you are about to write a claim**, you need §1 and §2. **If you are about to
trust one**, §3 is the catalogue of ways this project's own instruments have
been confidently wrong. §4 is what good looks like.

---

# 1. The four states

Every load-bearing statement is in one of these, and a reader should be able to
tell which at a glance.

| tag | what it means | what has to be beside it |
|---|---|---|
| **MEASURED** | an instrument was run and produced this number | **the control**, named inline: the thing that would have come out differently if the instrument were lying |
| **INFERRED** | measurement plus an argument | the argument, in one clause — "so", "which means", "at that rate" |
| **UNKNOWN** | nobody has settled it | what would settle it. An open task, not a silence |
| **SUPERSEDED** | a newer result replaces it | a pointer **forward** to the file that replaces it. Never delete the old number — rule 4 of the project |

Untagged prose is narrative, route description and driving advice. Tag the
things a future arm could act on and get hurt by.

## One convention, so it is not re-litigated per page: an exact tie is a beat

**The game awards the author medal at or under the author time, so a run level
with it has taken the medal.** That is the rule on *both* sides of every
comparison in this repo, and stating it once is cheaper than arguing it
twice:

* **A human's tie beats it.** Tannuleet's 8.127 on Great wtf of what #165
  equals the author time to the millisecond, so that map's author time is no
  longer unbeaten by a human, and the front page files it under *beaten*.
* **Our tie beats it.** Our 4.492 on Fall 2025 - 18 CP1 End equals its 4.492,
  and the front page files it under *taken*, not under some lesser heading.

Hedging one of those and not the other is how the same fact gets two verdicts
depending on who set it. **What a tie does not do is imply room**: on
Fall 2025 - 18 the true crossing is 4.49286 and the next millisecond needs
7.8 cm more travel, which is worth saying on the page — the margin is the
detail, the medal is the verdict, and they are different statements.

## The rule that generated this file

**Never report a harness limit as a physics limit.** When our reader does not
find X, the honest sentence is *"we have not found where X lives yet"* — which
is UNKNOWN, and is a task. It is not *"there is no X"*, which is MEASURED about
the world and is usually false.

| what was written | what was true | cost |
|---|---|---|
| 186935: *"no `CSceneVehicleVis` entity at all"* | one entity, 15 533 samples, every position (0,0,0) — a zeroed slot, which `tmtraj check` names in one line | 2 days, file looked unrecoverable |
| 227654: *"the carrier is truncated"* | one car split into **27 entities** at the respawns, tiling 0 → 147.000; every reader takes the largest | 3 attempts read "38 s of the race is unrecordable" |
| 173691: *"the finish is on the upper deck, the lower canopy is sealed"* | ten finish gates spanning y 130…194, **both rows fire**, lowest firing 133.97 | the target was 15.7 m up, not 48 |
| 285885: *"no rotation source within 82.6 m"* | a 797-probe survey whose window could not see the airborne roll that exists | a closed lead reopened |
| 134672: *"two cosmetic wheel channels"* | not cosmetic — another player's run, driving the tyre effects in a published video | shipped |

**This audit made the same mistake and had to eat it.** §E of the ledger said
the unresolved duplicate pairs "cannot be settled from a clean checkout". The
maps were on the shared store and the oracle runs on any box; all 38 were
settled the same evening. *A sentence about what is possible is a claim like any
other.*

---

# 2. What a claim needs before you write it

## An absence needs a positive control

**"N evaluations found nothing" is not a result without a control that fired in
the same batch, on the same budget, through the same code path.** Without one it
is UNKNOWN, however large N is.

A control that is not in the same batch is weaker than it looks: `tmsearch`
shipped with a broken `FINISH_BASE` for weeks while every driver-side control
passed, because the driver and the search were different code
(`tools/LINEAGE.md`).

Three ways a null has been manufactured here, all real:

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
  airborne roll 142 m away, which the same page names two paragraphs earlier.

## Calibrate on a positive control, not on a synthetic one

**A synthetic control shows that a term does what you think it does. Only a
positive control shows that the STATISTIC you compute from it ranks real cases
correctly.**

The first case in this file where **the mechanism is right and the statistic is
wrong**, which is why it gets its own entry rather than a line above.

The mechanism: a car whose wheels have left the ground is a free rigid body, so
its body-frame angular rate is exactly constant. True, and it is the only
readout of wheel load available inside a fork. The search's `domega` term
measures it, and its synthetic control is sound — two fixtures turning equally
hard, one free and one with a wheel biting, which a rate *threshold* cannot
separate and the derivative can.

Then 284238 ran it on a real known-good pair on their map: one human tape that
rides the obstacle, two of their own that launch off it, each window 900 ms from
that tape's own kicker engagement so neither side gets a longer one.

| tape | ticks | mean \|domega\| | max | ticks under 0.5 |
|---|---|---|---|---|
| Yhomas_TM 46.112, **rides** | 81 | 13.30 | 108.6 | **0 (0 %)** |
| cu1best, **launches** | 91 | 24.34 | 589.3 | **64 (71 %)** |
| b2r3, **launches** | 91 | 19.83 | 337.3 | **46 (51 %)** |

The **fraction of ticks below the bar** separates cleanly. **The mean does not,
and it points the wrong way**: the launching tapes average nearly twice the
rider's, because free flight is long exactly-constant stretches *punctuated by
violent impacts*, and impacts dominate an average. Max does not separate either.

So "the rate barely changes when the car is free" is the correct mechanism and
*taking its mean* — the obvious way to write it down — **ranks every launch
above every ride on that map.**

**And the bar itself came from the positive.** The rider's minimum |domega| over
those 900 ms is exactly **0.500**: he grazes the bar and never once goes under
it. A bar at 0.5 is comfortable. A bar at 0.2, which *looks* stricter and safer,
classifies the known-good rider as a launch. Same shape as the two constants
below, both set on the wrong quantity.

The tables are 284238's measurements on their map; the instrument and the
synthetic control are the search's (`SEARCH.md` §5.16).

## A negative result also needs a negative control

If the fix you are considering has a **sign**, the evidence for it must have one
too — and it must be paired with a case that does **not** need the fix, which the
fix should make **worse**.

The worked example is the same correction made twice. `fk regen`'s sample
pairing is one physics tick out on five of thirteen maps.

**Version one, withdrawn.** A `--recshift` flag driven by C11b, which measures
the stale-buffer distance *within* a file. It reported every regenerated file at
a clean `speed × 0.010 m` offset, and **nine files were rebuilt on that
reading**. Then somebody put a **downloaded** human ghost — one the game recorded
itself, which needs no correction — through the same instrument, and it read the
same: 267460's human WR at 0.4538 m / 45.42 m/s = 10.004 ms, 98 % tick-shaped.
**The measurement was right and the conclusion was wrong**, because C11b reports
a *magnitude*, and **a magnitude cannot see which side of a tick a file is on.**

**Version two, accepted.** `ghost phase` decomposes the residual **along** and
**across** the direction of travel — along-track displacement with nothing
across it *is* a time shift, so the quantity is signed. And the row that makes
it a verdict:

```
map      shift 0                      shift -10      shift +10
267859   0.136277 x3                  0.000093 x3    0.272435 x3
274191   0.000482 x3  (already right) 0.920794 x3    0.917557 x3
```

> **A correction that improves every number it touches has not been tested
> against anything.**

## An objective is a claim too

Three distinct ways one has lied here:

* **a decoy** — an objective that can be maximised without achieving the goal.
  Before trusting one, ask what the *laziest* way to maximise it is.
* **a candidate-chosen window** — see above, and here is what it cost. 284238's
  contact metric ran from the obstacle to *the candidate's own nearest approach
  to a station downstream*: a candidate that missed the station got a SHORTER
  window, never reached its freeze inside it, and scored **100 % in contact —
  four launches read as rides and one nearly reached a write-up.** The same map
  supplies the second instance: their do-nothing tape scores **J = −170.4,
  collecting 57 % of the available reward**, because a 120-tick window after the
  kicker reaches all the way to the wall impact at the far end. Narrowing to 40
  ticks is what makes the objective discriminate at all. **On this kind of key
  the window length is not a tuning knob — it is the difference between
  measuring the event and measuring whatever the car eventually hits.**
  The rule: if a candidate can influence the interval it is judged over, it will
  be judged over the interval that flatters it. Fix the window in ticks from the
  event. **A maximum over a superset of ticks is safe in one direction only** —
  aborting can only remove ticks, so it can never inflate a score, which is why
  a max-over-everything-after is sound where a fraction or a dwell measure is
  not.
* **a weighted sum used to test compatibility.** 284238 asked whether the car
  can reach the human's crossing angle *and* his roll; four searches minimising
  a weighted sum of position, velocity and attitude said no. A weighted sum lets
  a candidate **buy attitude with crossing angle**, which is exactly the trade
  the surface forces, so the search settles on the surface's own trade curve and
  reports it back as a frontier. Hard bars instead, and the roll comes down to
  the human's own value on the lane just published as unable to produce it.
  **A weighted sum expresses a preference, not a requirement.**

Related: **apply a bar in the frame the sampler produces.** A `vz` bar that
re-canonicalised an already-canonicalised velocity on a −120° screw map gated on
something that is not a crossing angle at all, and ten restarts chased `vz +55`
as an improvement. One owner per frame conversion.

## Measure the map; do not port the number

A constant measured once is MEASURED *on that map*. That it ports is INFERRED,
and the inference is usually the weak part. Four instances:

* `FK_STATE_OFF` documented as "a fixed offset from the server base" — false on
  the second map tested.
* a winning `FK_ADDR_DELTA` that lands on a `1,1,1` slot on the next map.
* the tick correction above: **eight of thirteen maps measure zero**, so it
  stays a flag and is set by requiring *that map's own* control to return zero.
* gravity, below.

## An exemption must quote the page it exempts

The integrity allowlist excuses files that legitimately carry another driver's
identity, and its own preamble says **"a name is not a contract: the page is the
claim"**. An entry was found breaking that rule:

```
270051  m270051_human_shaped_4831.Ghost.Gbx  human-shaped: built to the human's line
```

The page says that file is *"the author time, with ±10 ms of slack on every
input"*. **"Human-shaped" is a claim about the INPUTS. "His recording" is a claim
about whose run the file is.** Reading the first as the second laundered a real
player's account id and skin locator onto a file the page presents as ours.

The test: **does the page say, in words, that the file IS somebody's
recording?** Of the eleven files the skin census flags, ten pass by quotation —
*"published as his recording, which is what it is"* (227654), *"the author's own
author-time lap"* (228607) — and 270051 was the only failure. Two independent
instruments agree on that one file.

## A caveat that appears later than the number it qualifies will be copied without it

Same failure as the map costs above, at document scale rather than sentence
scale: **the number travels and the qualification does not.**

`SEARCH.md` §4's controls table listed *"the after-key puts him 5 mm from the
finish"* as a control. §5.9, two hundred lines later, explains that the point
being scored **is the author's own last telemetry sample** — he is at it by
definition, so the 5 mm is arithmetic and not evidence. The number had been
lifted into the controls table without the paragraph that empties it.

The strongest form of the evidence: **the author of the caveat is the one who
copied it wrongly, into their own table, having written the disclaimer
themselves.** Nobody misunderstood anything; the distance between the two
passages was enough.

Two fixes, and the structural one matters more than the textual one:

* the controls table no longer claims it;
* **§5.9 was reordered so the disclaimer no longer trails the number it
  disclaims.** A qualification that reads after its number is a qualification
  that will be separated from it.

This sits beside *An exemption must quote the page it exempts* because both are
about a claim surviving the trip from where it was measured to where it is used.

---

# 3. How an instrument lies

Every item here is something that passed, confidently, in this project.

## A two-sided control can pass because it tests the only case that cannot fail

The first entry here where the control itself is the thing that lies, and it
looks *sound*: both halves fired correctly.

285885 needed an airborne detector, so one was built as a gate 8 m above the
reference line's own trajectory. Its control was two-sided and both sides
passed — the reference tape fires **none** of the 8 m rungs and **all** of the
0.4 m ones. Silent when grounded, not silent in general.

Then **26 of 920 candidates fired it, and every one traced as firmly on the
ground**: `u_y 0.982` at 154 km/h, climbing smoothly, no ballistic signature at
all. They were driving further up a ramp whose *surface* is 8 m higher there.
**A gate at a fixed height above one line is a height detector wherever another
line differs — and the reference is the single line it cannot detect.**

> **Any detector calibrated on a reference, then applied to candidates that
> differ from it, has a control of exactly this shape available — and passing it
> means nothing.**

The repair is a detector that needs no reference: `tmtraj airborne` fits `y`
by least squares against the map's own gravity, and its positive control is a
**known-good answer** — the hand-found 35.060–35.400 window, and the fall off
the world at x = 507 — rather than the line it was fitted to. That it then
returns a completely different true answer (17 candidates with real air,
0.10–0.15 s each, all of them **after** the patch rather than before it) is what
confirms the first detector was measuring the ramp.

## It can fail toward *clean*

`tmtraj corpus dup` — the check that catches one run published twice — decided
whether identical positions were *expected* by asking whether the two input
tapes differ, and it asked by shelling out to **`fk tapediff`**. That is not a
command `fk` has, at any build:

```
$ fk tapediff --a A.gtape --b B.gtape
fk: ABORT: unknown command "tapediff"
```

The call failed every time. `.ok()?` swallowed it. `None` from that function
means *the tapes are identical*. **So every pair in the corpus came back
`identical-tapes / EXPECTED-SAME-INPUTS`, and the scan exited 0** — the check
excused every pair it exists to catch, and reported success doing it.

* **`.ok()?` is `2>/dev/null` with a nicer spelling.** That module's own header
  says the shell scripts it replaced were fragile because they "discarded
  stderr". The Rust port reproduced the bug it was written to remove.
* **Failing toward clean is worse than failing toward a null.** A null looks
  like a result and gets argued with; a pass looks like nothing at all.
* **A comparison needs a two-sided control**: a tape must read identical **to
  itself**, and two known-different runs must read **different**. Either half
  alone passes for a broken comparison.

Two more of the same family: `ghost verify` V2 reported *"1 copies, all
36.049"* while two copies of a stranger's 49.958 sat in the replay **header**,
where nothing looked — *a count of a set you cannot see all of is worse than no
count*; and an anchor wrote zeroed wheel channels straight through a passing
gate.

## A corpus can share the defect you are testing against

Four hypotheses were raised for one symptom — regenerated ghosts killing the
game client on import, with every headless check passing on the written file.
Three were killed by their mirror control. The fourth survived it, and it was
a one-line cause that **every test file in the corpus also carried**:
`rebuild_to` ended with `kept.push(car)`, so the vehicle entity came out **last**
where every ghost the game itself writes has it at **index 0**.

Both directions, one map, one session, each behind a `scene ready` control:

| file | car at | imports |
|---|---|---|
| the repo's own `tas_3836` | **0** | yes |
| the same file, car moved to the end, 77 samples unchanged | 2 | **no** |
| our rebuild of it | 2 | **no** |
| that rebuild, car moved back to the front | **0** | yes |

Moving the car breaks a file that works and moving it back fixes one that does
not. Nothing else changes in either direction.

* **The three dead leads were tested on a corpus where every candidate ALSO had
  the car misplaced.** `u01`, the declared checkpoint list and the notice list
  each looked decisive from one side and died from the other — and they had to,
  because the real defect was present in every specimen, on both sides of every
  swap. *A control matched on everything except your variable is not matched if
  the true cause is in the background of both arms.*
* **The one accidental success said so, and was read as evidence for the wrong
  thing.** `graft-scene`'s outputs imported — because grafting appends the
  scene *after* an already-first car, preserving the order by accident. That was
  taken as support for the scene-record hypothesis. A repair that works for a
  reason you have not identified is not a confirmation of the reason you
  assumed.
* **What to do about it:** when several plausible hypotheses all die against
  their mirrors, stop generating hypotheses and **diff a file that works against
  a file that does not, field by field**, including the ones no check reads.
  The separating field here was the ORDER of a list — not a value in it, which
  is why every value-wise comparison called the two files structurally
  indistinguishable.
* Nothing headless could see any of it: the dedicated server re-simulates the
  input chunk and never reads the entity list, so all four files re-simulate to
  their declared times and pass V1–V11.

## Agreement is not confirmation

**Two numbers that agree may be two readings of one quantity.**

* **≈0.5 mm.** Quoted for two years as "the client-vs-server floor", a property
  of two engines. It is **the distance between two copies of the car in the
  server's own memory**, and the pipeline was reading the wrong one: transforming
  from the copy with a live wheel block takes bit-identity from **0 of 455
  samples to 227 of 455**. Three maps agreeing at 0.489 / 0.511 / 0.501 were
  three readings of one thing. The corpus said so independently — 270051 reads
  **0.000000 m** ours-vs-ours where 173691 reads 0.000497 m on the *same*
  comparison. (The orientation half of that fix is open and regressed; the flag
  is default-off.)
* **This audit's own retraction.** It published a defect on 210218 — two files
  differing by 731 input ticks while holding bit-identical positions for
  89.95 s. The engine says both are sound: **0.0001 m over the 1735 samples
  where the records agree.** What misled it was two whole-file *rates* that
  agreed, 93.8 % and 94.1 %, **which say nothing about whether they are the same
  samples.**
* **A reproduction count is a majority.** Five regenerations of one 134672 tape
  produced the car once and four wrong picks, two agreeing to the metre. *A
  majority must never outrank a test that can identify the answer.*

> **An identical number across several independent findings is one artefact.**
> A pair test that failed to use each trace at its own accepted shift produced
> **four "DEFECT" verdicts at 2.140111 m** — one tick at 222 m/s — on five files
> each of which reproduces its own record to 0.007 m. This is a detection rule
> anyone can apply, and it is how the ≈0.5 mm was eventually caught.

## A relocated gate measures a PLANE CROSSING, and a plane crossing is not a route

Two failures of the same instrument, measured on 267460's pit on 2026-08-23.
The instrument is the standard one: move the map's own `Goal` item to a place
you want timed, and read the oracle's finish time as an arrival time. It is a
fine ruler — `tmmaps segments` is built on it — and it has two edges nobody had
written down.

### The car that fires it may have left the map

24 300 explicit programs were scored against a gate placed on the reference
line at the pit's far corner. **Three beat both reference tapes by 1.610 s** —
5.389 against 6.999 for the human world record *and* for our own incumbent, the
same number twice, which reads exactly like a real shortcut.

`fk trace` on one of them, and it is not a shortcut:

```
 6.500 (702.7, 95.5, 704.5) 170.4 km/h
 8.000 (671.6, 34.0, 652.7) 255.2
 8.500 (661.9, 10.1, 636.1) 142.1
19.500 ( 14.8,  8.0, 250.2) 368.2
21.000 (-94.8,  8.0, 189.3) 225.3
```

**y = 8.0 is the plane under the world.** The car fell off the pit, and on its
way down it flew through the gate's plane and fired it. It then slid 800 m west
of the map at 368 km/h. The trace is sound — self-check ok, 1956 rows,
|d(pos)/dt − v| median 0.052 m/s — so this is a real car doing a real thing, and
the thing is falling.

This is the **fourth** time on this map that a search record has been read as a
property of the route, and it is the first time the mechanism has a name:
**an objective placed in open space can be satisfied by a trajectory that has
left the track.** A relocated gate does not ask "did you drive here", it asks
"did you cross this plane", and those differ exactly when the candidate is
airborne — which is the state every interesting candidate on a trial map spends
half its time in.

Two mechanical guards, either of which separates the three fallers from every
real arrival with no judgement required:

* **score the gate together with a containment predicate.** `--pred
  'pit:box:...,ymin=<the surface>'` aborts a candidate that leaves the volume
  the route lives in, so a faller never reaches the gate at all.
* **compare path length against the reference.** The faller's own regeneration
  anchor measured **1189.5 m** against the real run's **817.6 m**. A candidate
  whose path is half as long again as the reference's did not take a shortcut.

Corollary for the other direction, contributed by the regeneration arm the same
night: `ghost regen` refused these tapes with *"the locate found something that
is not the car"*, and sent someone looking for a broken tape for half an hour.
The locate was right; what it found was a faller, and the record it was being
compared against was the **donor container's** — Wirtual's line. **A refusal
that names the wrong cause is worse than a slow answer.**

### And it is a knife-edge: 2.7 cm flips an arrival into a DNF

The same gate, on the same station, differing only in where the anchor was put:

```
i11@705.200,113.300,735.200/0   ->  6.999   (3 of 3 runs)
i11@705.221,113.273,735.215/0   ->  DNF     (3 of 3 runs)
```

The oracle is deterministic; both readings reproduce exactly. The car's own `y`
at that instant is **113.273**, so the first anchor is **2.7 cm above the car**
and the second is level with it. A y-sweep at 0.5 m steps says what the rule is:

| anchor relative to the car | result |
|---|---|
| −6.0 … 0.0 | DNF at every step |
| +0.5 … +6.0 | fires — **at 5.499**, not 6.999 |

So **the trigger volume hangs DOWNWARD from the anchor** (reach measured between
4 and 8 m: at the map's own finish the anchor is 8 m above the grass, and on
this station +8 stops firing), and the car must be *below* it.

The nasty half is the second column. At +0.5 the gate reaches far enough down to
catch an **earlier, different pass** of the same run, and reports 5.499. **A
gate that fires 1.5 s early does not look broken — it looks like a better
time.** Every "improvement" of that shape is a placement artefact.

What follows, and it is checkable rather than remembered:

* **Place the anchor 2–4 m above the height you expect the car at**, and say in
  the page which height that was.
* **A ruler is admissible only if it is INVARIANT.** The pit rulers are not: the
  useful one on this map is at the turbo gate, and it reads human 15.370, our
  incumbent 14.766, the east-flick DNF, **identically at dy = 1, 3 and 5 m**.
  That invariance is what makes it usable; quote it beside the ruler.
* **Never read a DNF from a relocated gate as an absence.** It is "this plane
  was not crossed in the direction and height band this placement can see",
  which is a statement about the placement. Yaw matters as much as height: on
  this map the same station fires for the reference at yaw π/2 and DNFs at yaw
  0, because those are the x- and z-perpendicular planes and the car crosses
  only one of them.

A first negative was drawn from **14 400 programs against a single-orientation
plane** before any of this was measured. It was worth nothing, and the positive
control in the batch did not catch it — the control fired, because the control
is the tape the placement was fitted to. Same shape as §3's *"any detector
calibrated on a reference, then applied to candidates that differ from it, has a
control of exactly this shape available — and passing it means nothing."*

## Test for a time shift, not a distance

A one-tick offset is a **pure time shift**, so it appears as a distance that
scales with speed and looks exactly like a wrong trajectory. Five sound files
read MISMATCH at 0.56–1.54 m at shift 0 and **0.005 m** scanned over ±3 ticks.
A third of this project's clean corpus sits one or two ticks off.

## One reading is not enough

173636 `TAS_22072` read "does NOT match" at fork ticks 400 and 700 (0.30 m) and
**matched at tick 1000 (0.0008 m)**. Same file, same map, same binary. Sweep,
and take the best — and among readings that have found the car, prefer the one
with more **coverage**, not the one that agrees to another decimal.

## Two constants this project keeps getting wrong

**Gravity is ≈24.3–24.6 m/s², not 9.81 and not per-map.** Free fall is linear
drag in vertical speed: `a_y = −g − k·v_y`, g = 24.78 ± 0.10, k = 0.032.

* **Too low:** 153527 computed a slope's gravity as `9.81·sin 26.6° = 4.39` and
  published a car "decelerating at 2.4× the slope's gravity". Measured on that
  map from **335 free-fall stretches**, the median is **−24.314**, against which
  the observation is **0.97× — an ordinary coast.** A whole published mechanism
  rested on a textbook constant.
* **Too specific:** 285885's *"gravity here is per-map"* — both quoted numbers
  are the one law at different `v_y` (−25.20 ⇒ +13 m/s; −24.308 ⇒ −16 m/s).

**Never quote a scalar `g` without the `v_y` it was measured at**, and any
energy, fall or deceleration figure should name the `g` it used. Whether `g` is
*genuinely* per-map is UNKNOWN — the intercept is fitted on one map.

The second constant is the ≈0.5 mm above.

---

## An offset without its anchor is not a measurement

Contributed by the carrier-bytes arm with its own numbers, and it is the fourth
instance of *a check that is precise, confident and blind* (§3).

**The definition.** *"car" = the address of the f32 position triple of the copy
of the vehicle state whose slots at `car+92 / +136 / +180 / +224` hold **live
floats**.* The qualifier is load-bearing because the engine keeps several copies
of the car, they hold the same position, and **they all pass every structural
test there is** — velocity 12 bytes on equals the position's own derivative, the
four floats 16 bytes back are a unit quaternion. Only one has the *fields* around
it. One 1.25 MB window, one server process, ten copies:

```
+1045916   4 of 4 wheel slots live   0.000001 m from the game's own recorded path
+1045052   4 of 4 live               0.000001 m
+1048564   1 of 4 live               0.000486 m
+1043772   1 of 4 live               0.000486 m
```

**The cost.** Anchoring on `Layout::pos` — the address the locator returns, and
the natural thing to anchor on — **wrote zeroed wheel rotations and zeroed gear
into a file that passed the entire `ghost verify` gate**: V1 codec identity, V6
tape agreement at kappa 1.000, V7 the plain oracle re-simulating the written file
to its declared 22.730. Every check passed **because none of those bytes affects
the simulation**, and a provenance check cannot catch it either — the bytes are
not a donor's, they are zeros. The guard that does catch it needs no answer key:
**are the four wheel slots holding floats that move?** Four against zero, nothing
in between.

### And the criterion does not transport — which is the entry

Two arms measured this and their results disagree. **Both are internally
consistent and exactly verified**, so the disagreement is the finding:

| | carrier-bytes arm | video-reconstruction arm |
|---|---|---|
| gear | `car+340`, **100.00 % exact** on 8 recordings | `car+748`, **99.43 % exact** |
| wheels / wetness | 4 rotations at +92/136/180/224, 99.25–100 % | wetness at `car+180`, **95.4–96.0 %** on two ghosts |
| the other's liveness test, ported | — | 4 dead slots, one value, exactly 0.0, over 814 ticks |

The gear relation confirms from both sides (748 − 408 = 340). **The wheel
relation does not**: probing wheel rotation directly from the second anchor
implies **1196** where gear implies **408**, so the two anchors are not one
constant apart and the wheel block may not sit at a fixed offset from the
position at all. So the second arm's anchor reads as "a bare position copy" under
the first arm's criterion while reproducing the recording at 95–99 % — **dead
memory does not do that.**

> **The liveness criterion is sound as a CHOOSER WITHIN THE FRAME IT WAS
> VALIDATED IN, and is not a transportable test.** A rule with a stated domain of
> validity is more useful than a rule that quietly fails elsewhere.

The test that *does* survive transport is the other arm's — *does a named channel
reproduce the recording?* — which needs an answer key and so is not always
available, which is why the liveness rule exists at all.

> **AN OFFSET WITHOUT ITS ANCHOR IS NOT A MEASUREMENT.** Every offset this
> project has published is relative to an anchor nobody was naming, and 408 bytes
> is exactly the size of a mistake nobody catches by eye. Publish the anchor with
> the offset, and publish the frame the anchor was validated in.

Same shape as *measure the map, do not port the number*, one level down: **do not
port the offset either.**

## Four encoding assumptions that each cost a channel

Invisible until the assumption was removed, and any of them may be in other
tools:

* **range** — filtering f32 candidates to 0..1, so a wheel rotation running to
  1607 is never seen;
* **rounding** — round-to-nearest against truncation, worth **17 points** on one
  channel (83 % → 96 %);
* **quantisation** — testing exactness against a u8 quantisation for a channel
  the record does not quantise;
* **the small-integer-lookup trap** — an integer read as an f32 is a denormal, so
  a fitter returns `k = 2.85e45` at a flawless 100 %. Byte 89 was offered that
  way at `car+58` on five keys; scored as a raw byte on eight keys with no refit
  it is **0.00 %**.

And one that is not an encoding but belongs beside them: **a median of a bimodal
population reports whichever mode it lands in.** A quaternion candidate was
reported "exact, 0.00000 rad" from a median while about half the instants matched
and half did not; the honest number is **75.0 % exact, p90 0.00042 rad**.

## Absent is not zero

Dirt (sample bytes 93/95/97/99) is **ABSENT** from regenerated files, not zero —
pre-registered across all eight remaining slots of the wheel record and refuted,
best worst-key lift **−7.35 points**, below a constant. *A page must say absent;
a zero read as a measurement is how a published clip came to run on dirt tyres.*
Ice, by contrast, ships: **100.00 % exact** on two independent recordings on two
maps (462 and 1370 samples, against 71.9 % and 79.0 % constants) — and was
deliberately **not** shipped during the hours it was a one-key result.

# 4. What good looks like

**Do not qualify what is solid.** A true claim with its control cited is the best
outcome and the most common one: most of what is written here is right. Hedging
a sound result is the same failure as asserting an unsound one — the reader
cannot tell which is which either way.

Four passages are the model, all written before this convention existed:

* **`203330`'s authority map.** It separates two statements people conflate —
  *"the car does not respond"* (true from 2.270 to 3.650) and *"the input has no
  authority"* (true only to 2.970) — from 561 single-tick overwrites, then says
  outright that the *mechanism* behind the four bands is **"measured but not yet
  attributed"** and that the obvious explanation fails on the fourth band.
* **`285268`'s check on its own check.** *"`nearident` returned `overlap=0` with
  a mean of 1.8e308 — it compared **nothing**… its `INDEPENDENT` verdict on this
  pairing is vacuous."*
* **`270051`'s false positive.** It reads COPY on an honest pairing; rather than
  overriding the tool, the page measures the tool's own floor, gives the
  human-versus-human control (3, 4 and 6 samples against our 41), and correctly
  attributes the offset to us — two days before anyone could say what caused it.
* **`tools/LINEAGE.md`.** *"Check the constant, not the version."* It splits what
  a defect invalidates into three parts and puts the uncomfortable one in the
  middle.

## Check the record before you write the correction

The audit's third wrong claim, and the cheapest one to have avoided.

`tmtraj corpus claims` flagged that 146612's `TAS_39183` and `KEYBOARD_39706`
each declare **39.555** in their header instead of the time in their name. That
is a true fact about two files. The audit turned it into *"neither figure is
backed by the file that bears its name"* and rewrote two pages around it.

Then the oracle was asked:

```
TAS_39183.Ghost.Gbx        PASS V7   oracle re-simulated the written file: 39.183
KEYBOARD_39706.Ghost.Gbx   PASS V7   oracle re-simulated the written file: 39.706
```

**The names were right and the headers were stale** — a searched tape is built
inside a carrier and inherits its declared time; `ghost declare --from-oracle`
fixes it and changes no physics. All eight publishable files there re-simulate
to exactly the time in their name, and the ninth returns DNF at cps 5 as its own
name says, which is the negative control.

**And `tools/LINEAGE.md` already said so**, in a row written before the audit
started: *"146612 · 9 · 8 + the file named `SEGMENT_cp5_…_DO_NOT_PUBLISH`, which
returns DNF cp5 as its name says."*

> **Before writing that something is unsupported, search the repo for the arm
> that already measured it.** A header is not the authority on what a tape does;
> the oracle is. A flag from a consistency check is a **question**, and if you
> cannot answer it, the honest output is an open question — not the answer the
> check's own framing suggests.

## A settled question beats a standing suspicion

The 46 `corpus dup` refusals were adjudicated rather than left as caveats:
14 fall inside 203330's *measured* per-tick inert window, 3 are at separation
exactly 0.000000 m (two of which the 227654 page already documents by hand as
one trajectory), 5 are documented 286279 provenance, and the remaining 24 — 38
pairs at the finer verdict — were settled by re-simulating both tapes and asking
how far apart the engine puts the two cars **on the samples where the records
agree**: **35 measured innocent, 1 inconclusive at 0.001 m, 2 untested, zero
defects**, over 143 traces.

**The 2 untested are a finding, not a gap.** No file locates on 238835 or
267859 at any of 14 fork points — both turtle maps, where the car is inverted at
walking pace. An independent arm reached the same conclusion from the opposite
direction with a mechanism: the locate demands `d(pos)/dt` agree with stored
velocity to 15 % of speed, and the real car scores **1.41 m/s against a bar of
1.14**. Two instruments agreeing on which maps are unreadable, for a stated
reason, is a result.

## Run these before editing a page

```
tmtraj corpus claims --root .   # does a page agree with the files in its own directory?
tmtraj corpus dup    --root .   # two published files of one map carrying one recording
tmtraj adjudicate ...           # settle a dup verdict against the engine
```

## Scratch named after the input is single-instance per input

**A tool whose scratch path comes from its input is single-instance per input,
and nothing says so.** `ghost roundtrip` named its working files after the
SUBJECT (`<subject>.roundtrip.Ghost.Gbx`, `<subject>.grid.Ghost.Gbx`), so two
runs comparing two locate settings on ONE subject — which is what comparing
settings *means* — shared a grid file and read each other half-written. For
forty minutes the logs showed only `falling back to the anchor search`, which is
what a genuinely hard locate looks like, and one of the runs also reported
`no oracle available` because its input was being rewritten underneath it. Two
plausible physics stories, one filesystem collision.

Several tools here take a subject and write scratch beside it, so this is a
class rather than an incident. The fix is a process id in the name and an
explicit `--out`; the rule is that **a parallel run is the normal way to
compare two settings, and a tool that cannot survive one should say so or stop
being one.**
