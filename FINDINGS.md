# Results

One block per map. Times are validated through the plain oracle
(`TrackmaniaServer /nodaemon /validatepath=`) against the untouched map file,
with a known-answer human ghost as an identity control in every batch.

---

## 227969 — Great wtf of what #165 (uid `LtSUTxJ71u7ayvLj57wUdVPyH2h`)

AT **8127** · human WR **8197** (Titoch_tm) · 42 recorded runs · gap 70 ms

**Author time beaten. Full write-up and driving guide: `227969/RESULT.md`.**

| tape | time | vs AT | steer events | distinct steer values | device |
|---|---|---|---|---|---|
| keyboard, 14 inputs | **8075** | **−52** | 12 | 3 | keyboard |
| keyboard | 8058 | −69 | 23 | 3 | keyboard |
| action keys, 8 detents | 8050 | −77 | 54 | 15 | pad |
| analog, event-thinned | 8021 | −106 | 62 | 50 | TAS |
| analog, unconstrained | **7998** | **−129** | 185 | 111 | TAS |
| (human WR, for scale) | 8197 | +70 | 11 | 3 | keyboard |

**Headline:** the author time falls **on a keyboard**, with the same three
steering values and essentially the same number of key presses the human world
record uses (12 steer events vs 11), 122 ms faster.

**Where the time is:** nothing over the first 6.5 s (we are +10 ms down there);
all 187–199 ms in the last 1.4 s.

**The technique — verdict UNDISCOVERED:** the map ends with the car being thrown
off a wall at 420 km/h, arcing ~270° along a curved wall, and being kicked into
a ballistic flight through the finish gate. **All 42 humans hold full lock
through that wall.** That rolls the car onto its side (roll 0.9–1.5 rad) and
pitches the nose up ~57°, and the kicker then eats a third of their speed
(73.6 → 61.3 m/s for the WR). Releasing and **pumping the lock in three taps
(220 / 80 / 40 ms)** keeps the car flat (roll < 5°) and square (sideways
velocity 0.35 m/s), so the kicker costs only 3 m/s: **vz into the finish plane
69.2 m/s against the WR's 57.3 and the field's best 59.8.** Also commit to the
kicker at 7.39 s, not the WR's 7.69 s.

**Integrity:** legitimate. Max 2.57 m from the human WR's own trajectory over
the whole run; state at the decisive point inside the 42-run field on every
axis, with two humans passing it faster; the map's single collision event is
taken identically by every human run; no respawn, no skipped geometry.
164/164 tapes written this session re-validate exactly; headline tapes
re-validated cold against a re-downloaded byte-identical map.

**Tolerance:** the two mid-pump releases have ±20–30 ms of recoverable slack;
the three commits (into the throw, into tap 3, into the kicker) are 10 ms-tight,
the kicker commit worst (10 ms early = +90 ms, 10 ms late = DNF).

### Transferable findings

* **A pad seed beats a keyboard seed for an unconstrained search.** Arms seeded
  from the keyboard WR converged near 8.14; the arm seeded from the rank-2 *pad*
  run (31 ms slower as a human run) produced everything below 8.13. A 3-value
  tape has almost no local neighbourhood for the operators to work in.
* **Quantising an optimised analog tape does not work at any resolution.** Even
  a 64-level ladder (max change ±1/127 per tick) makes the 7998 tape DNF. Low-
  input tapes must be *searched for* under the constraint. Seeding the
  constrained search from a human keyboard run — already legal in every ladder —
  reached 8102 in 80 s.
* **Establish the input alphabet from the data.** The human WR's own tape
  contains exactly `{-127, 0, +127}`; that is ground truth for "keyboard", not
  an assumption.
* **Greedy event deletion is cheap and effective within an alphabet**: 20 → 14
  events for zero cost on the keyboard tape, 185 → 62 steer events for 23 ms on
  the analog one.
* **Post-finish ticks are inert** and deleting them is free — worth doing first,
  it removes a third of the events on a tape.
* **INCIDENT — the sub-tick plane surrogate requires the finish to be crossed
  with a repeatable attitude.** On this map (airborne finish, roll varying over
  1.5 rad across the field) the trigger is body-based and a fixed plane is wrong
  by up to 1.30 m ≈ 19 ms. It produced a self-consistent 7990.705 that the plain
  oracle calls 8004. Per-seed calibration was exact and the whole-tick guard
  passed, so nothing internal catches it. Specimen in
  `tm-loop/phantoms/m165-subtick-plane-20260818-1752/`. Detail in
  `227969/RESULT.md` §7.
* `p37` on this leaderboard (8610) re-simulates to **8477**. Flagged, unused.

### Tooling added (Rust, in `tmtas-rs2`)

* `tmsimp` — quantise a tape onto an input ladder, greedily delete change
  events, snap events to a coarser tick grid, constrained polish, and measure
  per-input **recoverable** tolerance (mistime one input, re-time only the later
  ones). Everything evaluated against the real oracle.
* `tmsearch --qlevels N` — low-input mode for BOTH search paths: every candidate
  is snapped onto a ladder of N levels per side after mutation, so the search
  only ever emits tapes a human's input device could produce.
* `pred_core.rs` — the sub-tick plane accepts a negative value meaning "z-plane
  at |v|, crossed with z increasing", for maps whose run axis is z.

---

# Cross-map patterns

Findings that showed up on more than one map, and are therefore about the game
rather than about one leaderboard.

## Attitude: where the field loses time, and where the rule stops working

Two unrelated maps, same story.

**227969** — every one of the 42 human runs holds full lock through the final
wall-ride. That rolls the car onto its side (roll 0.9–1.5 rad) and pitches the
nose up ~57°, and the kicker at the end then eats a third of their speed
(73.6 → 61.3 m/s for the world record). Staying flat (roll < 5°) and square
(sideways velocity 0.35 m/s) costs 3 m/s instead: **199 ms**.

**203330** — the endgame ordering at the finish-platform lip is a roll ordering.
Every fast candidate is at roll −0.02 to −0.01; every human run is at −1.25 to
−2.82. Speed at the lip orders the field perfectly.

Neither map's advantage is precision. Both are "the car should be flat here and
the whole field lets it roll" — which is something a driver can act on with no
tooling at all, and which is visible in any replay.

### The series, including its failures

The rule was then tested prospectively, with predictions and bars frozen before
any correlation was computed.

| map | decisive feature | surface? | result |
|---|---|---|---|
| 227969 | wallride into a kicker | yes | holds — the whole 199 ms margin |
| 203330 | platform lip at 860 km/h | yes | holds — orders the field perfectly |
| 203072 | 5.5 s ballistic flight | no | **correctly null** (r = +0.14), pre-registered |
| 267859 | 15 inverted landings | yes | **holds**, pre-registered: top-10 r = +0.75…+0.91, strengthening under the confounder check |
| 228607 | reactor map, mixed features | yes | **FAILS**, pre-registered: r = +0.327 top-10 but **−0.131** over the full field, signs flipping between populations at every contact |

**So the rule is not universal, and the missing condition is about the FIELD,
not the map.** On 228607 the last sector carries the variance (sd 936 ms against
44–417 ms elsewhere) but **does not order the finish** — the run with the
fastest last sector in the entire field finishes 11th, having dropped 2.5 s
earlier. That field is separated by *where each run loses a chunk*, not by one
technique done better or worse, so no per-feature statistic could have ordered
it.

The selection criterion that falls out, written down for the next test:
**the decisive sector must dominate the variance AND its time must order the
finish.** Only then is a per-feature attitude statistic meaningful.

Also worth recording as method: a null control arm is only evidence when the
treatment arm fires. 228607's ballistic phases came back null exactly as
predicted (0 of 5), and that bought nothing, because the surface arm did not
fire either.

### What the same maps did establish, firmly

Two agents independently measured the reactor block's force on two variants of
the same map. One predicted the second variant would mirror the first at +21;
it measured **−22.33 ± 1.99** — same sign, same magnitude. The prediction was
refuted, and that is what makes the measurement useful: a flip would have made
it a property of the map variant, while identical values across two
independently measured variants make it a property of the block.

> **A firing reactor applies ≈21–22 m/s² along the car's own −up axis.**
> Inverted, it cancels gravity. Upright, it doubles it. On edge, it is
> horizontal thrust. **Orientation aims it.**

The free-ballistic control in the same runs, same statistic: **−1.37 ± 0.25**,
three orders of magnitude away.

That also forces a rename in waiting. If attitude matters under a firing reactor
in mid-air, then the rule is not about *surfaces* at all — it is about **any
force whose direction is fixed in the car's frame**. A surface normal is simply
the commonest example.

## Is a modelled finish plane trustworthy on this map? One command

Searching against a modelled finish plane (interpolating the crossing inside the
tick, so scores are microseconds instead of integer milliseconds) breaks the
millisecond plateau — and on some maps it silently lies, because the real
trigger is **body-based**, not a plane through the car's centre. A differently
oriented car presents a different leading point.

The test, which needs no theory:

Validated time is `ceil(t_true)`, so a tape's crossing coordinate at its own
validated millisecond carries `[0, v × 1 ms)` of spread **by construction**.
Measure the actual spread across several tapes and compare it to that budget.

| map | measured spread | budget | verdict |
|---|---|---|---|
| 227969 (airborne finish, roll varies 1.5 rad) | 1.30 m | 0.067 m | **19× excess — the plane lies by ~19 ms** |
| 203330 (grounded, repeatable +0.30 rad pitch) | 0.233 m | 0.238 m | no excess — exact to ±0.02 ms |

On 227969 the plane produced a self-consistent "7990.705" that the plain oracle
calls **8004** — six milliseconds *worse* than the seed it came from. Per-seed
calibration was exact to 0.002 ms, so nothing inside the search could catch it.

**The rule: the plane is a gradient, not a score.** Use it to escape a plateau,
never bank on it, and only promote an incumbent when the plain oracle improves.

## Seeds: one basin or many is a property of the map

On **270051**, a full greedy from rank 5 converged 3 ms behind the same
treatment applied to rank 1 — the basins do not merge, and the choice of seed
decides the answer.

On **203330**, all five records converge to 13986 in 2.5 minutes — *including* a
wall-clipping run 7 s slower and an overshoot that never triggers the finish at
all. One basin, and the seed does not matter.

So "prefer this seed" never generalises on its own. It applies only where the
seed's steering shape survives into the decisive section of the map. Test it per
map; it costs minutes.

## Optimise for robustness and the teachable input appears

On **270051** a speed-first search found the last millisecond as a one-tick
75 %-lock stab — an unteachable lottery ticket. Scoring candidates by their
**worst** time over a ±1–2 tick placement window found the *same physical
effect* as a three-tick, 7 %-of-lock brush with a 30 ms window, and matched the
author time with **±10 ms of slack on every input**.

The lottery ticket and the teachable input were the same discovery. Only the
objective decided which one came out.

## What a "can a human do this" test must not be

Perturb-and-replay — quantise the tape, or mistime one input, then replay it
blind — kills runs that humans demonstrably drive. On **279197** both
quantisation to a steering step of 2 and sample-and-hold at 2 ticks DNF the
**human world record's own tape**, mid-route.

That measures the open-loop fragility of a recording, not human skill. A driver
is a closed loop: they see the car drift and correct on the next frame. The
honest measurement is *recoverable* tolerance — mistime one input and let the
later inputs re-time, which is what a person actually does.

And on these maps the author time is a **driven validation lap**. Someone hit
it. "No human can do this" is never the answer.

## Trial maps: the clock runs through your failures

On a trial map the recorded time is **clean driving plus every failed attempt**,
because the player respawns at the last checkpoint and the clock keeps running.
So a TAS does not have to drive better than a human — **it has to not fail**.

On [Angustus](238835-turtle-trial-angustus) the only human record contains
**198 respawns** and the author's own author-time lap contains **20**, of which
roughly **160 of its 463 seconds are fourteen failed attempts at one obstacle**.
Cutting the failures out took it from 1 964 933 to 265 159 — **43 % under the
author time, with no driving search at all.**

**What a respawn is.** It lives in the input bitstream as a bit in each packet's
`word0`, and the standard decoders are blind to it — which is why every run this
project had inspected reported `NbRespawns: 0`. That was never a rule: the
validator accepts and exactly re-simulates a run containing 198 of them.
One press of `0x22` is a **soft** respawn (restores position, speed *and*
attitude — on a turtle map, a car doing 62 km/h upside down); two presses within
~100–640 ms, or one press of `0x1002`, is a **hard** respawn (standstill,
upright, square).

**Why the method is self-validating.** Respawn state is deterministic and
history-independent, so a failed attempt splices out and
**`finish = base − deleted`, exactly**. If the arithmetic is not exact, the
splice is wrong — every cut checks itself.

**But check `NbCheckpoints` first.** On
[Impossible Mini Trial 2](https://trackmania.exchange/maps/267460) the only
waypoints are the spawn and the goal, so a respawn returns the car to the start
and is never a recovery: "Trial" there is a *building style*, not a checkpoint
mechanic, and the gap is a genuine route gap.

**And the same method took [Leto](286279-turtle-trial-leto) 33.7% under its
author time**, where the result is even starker: the first sub-AT tape was
**100% the world record holder's own inputs**, in his own order, at his own
resolution, with ten of his failed attempts deleted — 236.972 against a 355.181
author time. Not one tick of TAS mutation. The map's own author failed nine
times in the same sector, at the same obstacle everybody else fails at, and 135
of the author time's 355 seconds are those retries.

Leto also pins down exactly what a respawn restores: **the state the car had
when it crossed the checkpoint** — position, velocity *and* attitude, each run
returning to its own crossing state (measured at one checkpoint across five
runs: 26.7 / 22.2 / 16.8 / 23.4 m/s). A *standing* respawn is different: a
canonical reset to the checkpoint block's own spawn transform, bit-identical
across runs on all 29 telemetry columns, at the price of the car being **frozen
for 800–850 ms**. Top players use one deliberately at fast checkpoints, because
braking to a controlled standstill costs more and the reset hands you a
perfectly known attitude for a section where attitude is everything.

**The corollary that bites: a cut is safe, an optimisation upstream is not.**
Deleting ticks that lie entirely after a crossing changes nothing the respawn
depends on, which is why `finish = base − deleted` holds exactly. Change
anything *before* the checkpoint and the crossing state moves with it, and every
input after the respawn was tuned for the old one. Respawn-anchored sectors are
therefore **not independent** — they cannot be optimised in parallel and
recombined. Measured twice, independently: on
[284238](https://trackmania.exchange/maps/284238) a sector optimised to cross
CP4 95 ms earlier made the unchanged tail DNF outright.

## Input tapes ARE portable between ghost containers — but only with two chunks

A `.Ghost.Gbx` holds its per-tick inputs in an archive chunk (`0x0309201D`).
Moving that archive alone into a *different* ghost file DNFs at checkpoint 1,
every time, while moving it into its own container reproduces its time exactly.
That looked conclusive, and it was published here as "cross-run splicing is
impossible". **It was wrong.** Bisection found the carrier — a minimal pair:

| transplanted | result |
|---|---|
| the donor archive alone | DNF at CP1 |
| + `0x0309202E` *(the obvious suspect: 4 bytes in one file, 69 in another)* | DNF at CP1 |
| + `0x03092000` *(the recorded telemetry)* | DNF at CP1 |
| + `0x0309202D` | DNF at CP2 — progress |
| **+ `0x0309202D` and `0x0309202B`** | **exact** |

Carry that pair and **every** foreign tape re-simulates exactly in another run's
container. Cross-run splicing is available, and so is re-simulating an author
ghost extracted from a `.Map.Gbx`.

**It paid immediately.** Leto's published run went from 235.625 to
[**220.391**](286279-turtle-trial-leto) — the map author's own embedded
author-time ghost, which an hour earlier could not be re-simulated at all, with
its nine failed attempts cut out. 134.790 s inside the author time.

Two things about how the wrong answer survived as long as it did:

- **A negative from a hand-enumerated list is worth nothing.** Fourteen chunks
  were swept by hand and the finding was written up as a hard limit; the answer
  was the two chunks that were never on the list. Bisection settled it in three
  validations. When a sweep comes back empty, suspect the enumeration before the
  hypothesis.
- `0x0309202D` **is the same size in every ghost**, which is exactly why a
  size-diff sweep skipped it. "Looks identical across files" is not evidence of
  "carries nothing".

## The DNF at CP1 was never about CP1

Three maps independently recorded the same unexplained failure — a synthesised
respawn press splices in exactly at the later checkpoints and DNFs at CP1 — and
all three filed it as respawn lore. The rule:

> **You can cut TO a soft respawn but not to a hard one. The trap is the respawn
> KIND, not the checkpoint index.**

On Leto the CP3 trim that works lands on a soft respawn; the CP1 trim that never
worked lands on a standing one. Nothing about CP1 is special.

A cut to a hard respawn *does* work — but only at one exact phase, and **the
phase is not periodic**: ticks 12376–12398 DNF, 12399 works, 12400 works,
12401–12499 DNF, 12500 works. So **sweep the prefix, not the tail.** Sliding the
tail finds nothing and invites an "impossible" conclusion; sliding the cut point
at a fixed graft finds the phase. That is what `tmsimp --mode cutsweep` does,
reporting each survivor's expected `base − 10·deleted` and whether the
arithmetic is exact.

## Before you describe what a driver is doing in the air, check the air is live

On [U10S_32 MAX-UP](274191-u10s-32-yeet-max-up) there is a **1.2-second dead
zone with no air control at all**: replace the steering with *any* constant
anywhere inside it and the game returns the identical millisecond.

A complete and plausible write-up had already been drafted — *the field pins full
lock through the fall and we let go* — and it was wrong, because the engine
ignores that input entirely. The real technique was one beat earlier, on the
ground.

**Sweep a constant through every air phase before writing a word about
technique.** It costs a minute.

## The oracle is not a physics engine you can assume

Two instrument bugs found only because a new map entered a new regime:

- the trajectory reader searched a fixed 16 KB *below* the vehicle state for the
  frame clock — correct on one map, and **319 KB wrong** on another;
- its self-consistency check rejected trajectories when `|d(pos)/dt − v|`
  exceeded a **fixed 2.0 m/s**, calibrated on a car topping out near 90 m/s. At
  215 m/s a one-tick central difference legitimately disagrees by about 1 % of
  speed, so **good** trajectories were being discarded with an alarming error.

Both had been invisible for as long as every map looked like the first one. The
habit that catches this class: **when you are the first into a regime — a faster
car, a deeper map, a new surface — check the instrument before you trust the
measurement.**

## A constraint that silently does not bite

Every low-input result in this repo depends on an alphabet constraint — "steer
may only take these values" — actually being enforced during the search. Twice
now the constraint was wired in wrongly and the search happily produced a
"keyboard" tape with 150+ distinct steer values. The tape was *valid*; it was
just not keyboard. This is the phantom problem inverted: an instrument quietly
reporting a success it has not earned.

The two ways it went wrong are both worth knowing. The constraint was applied in
one of two near-identical `mutate → apply` sequences and not the other, so it
did nothing for 140,000 candidates. And even in the right place, snapping only
the mutated window **leaks**, because retime/shift operators move values from
outside the window into it — the ladder has to be applied to the whole steer
array after mutation.

**The 90-second control that catches both: search with a one-level ladder.**
Every steer value becomes zero, the car drives straight, and a healthy
constraint must report `finish 0%` and no improvement. Before the fix this
printed `finish 64%` and a new best. Paired with an ordinary identity run (no
ladder → the template's own time) it pins the instrument from both sides: it can
say yes, and the constraint really bites.

## Do not convert an analog tape to keyboard — search under the constraint

The obvious way to produce a keyboard run is to take the fast analog tape and
round it. **It does not work anywhere**, and it has now been measured
independently on four maps: replacing each analog sweep with the instantaneous
step a keyboard physically produces (82 placements across four sweeps on
[145875](145875-unlucke-get-jiggy-with-it)) produced **not one finishing run**;
quantising a finished tape DNFs at every resolution tried, down to a 64-level
ladder.

The pad and keyboard lines are **different basins**. A keyboard strat has to be
searched as one, from a keyboard seed, under the constraint. When that is done
properly the result is often startling: on 145875 the keyboard tape reaches
6.323 against an unconstrained floor of 6.322 — **one millisecond** — and on
[Leto](286279-turtle-trial-leto) keyboard costs 314 ms out of 236 seconds, 0.13%.

This matters because "there is no low-input family on this map" is a conclusion
several searches have reached *by conversion*, which is exactly the method that
cannot find one.

## A negative result requires a positive control

The most expensive class of bug on this project is not a search that reports a
time it did not earn. It is an **evaluator that can only ever say no** — because
nothing it produces is ever banked, so every re-validation guard is blind to it.

Two instances, both caught only by deliberately making the instrument say yes:

**A foreign evaluation template.** To avoid the 20-second DNF cost of an
879,231-tick tape, an agent built a short template by patching the map's uid over
a *different* map's ghost header. It loaded, it simulated, it ran 130× faster —
and it could never have returned a finish. **~144,000 evaluations of "DNF" read
as "the target is a needle."** Caught by relocating the finish gates onto the
spawn: on that map the map's own ghost finishes in 2.024 s, while the template
still reported `wrong simu`. The root cause was a `recon` mode added so an
unknown map's base tape was *allowed* to DNF — deliberately removing the identity
control at exactly the point it mattered most.

**An alphabet constraint that silently did not bite** (above): the search
reported healthy progress under a "keyboard" constraint while producing tapes
with 150+ distinct steer values.

The rule, and it is cheap: **before you believe an instrument saying no, prove it
can say yes.** If the base tape cannot finish, build something that *must* —
relocating the gates onto the spawn is one command. Pair it with the identity
control (unconstrained → the template's own time) and the instrument is pinned
from both sides.

## An hours-long record is not evidence that a map is a joke

`idm ruinin ur day #460` has a 2.44-hour world record, which reads as a troll
map. It is not. The validator reports **`NbRespawns: 929`**, the telemetry has
914 position discontinuities back to spawn, and there is a **50-minute
motionless stretch** where the player was AFK. It is one session of 930 attempts
with the clock never reset — the map's only waypoints are a spawn and a goal, so
nothing ever resets it. **The final successful attempt lasted 18.819 s**, against
an author time of 15.643.

So the map is a ~16-second flight and its author time is an ordinary good run.
**Compare against the author time, not the record, and check `NbRespawns` first.**

The same map supplies the cheapest possible dead-ghost check, from a different
direction: `NbRespawns: 4294967295` is `(u32)−1`, a field that was never written,
and it reliably marks a ghost recorded on a build the current oracle cannot
re-simulate. Two seconds to check, before spending a field download.

## Relocating a gate is the safest map surgery available

A **free block's position is not in its block record** — it lives in chunk
`0x0304305F`, six f32 per free block (yaw, pitch, roll, x, y, z) in block order.
That is why a block lister prints `pos=None` for a free waypoint: the position is
knowable, just somewhere else.

Rewriting those floats relocates a gate **with no lookback-table involvement, no
size change and no re-encoding** — which matters because the Id table is what
makes every other kind of map surgery dangerous. Measured: a relocated gate is
caught at 16 m spacing and missed at 32 m, so 16 m is the ceiling for a shaping
ladder.

This is what makes intermediate-gate shaping practical on maps that have no
checkpoints to score against — and gate relocation is also how you prove an
evaluator can say yes (above).

## The cheapest deliverable is the human world record plus two presses

The fastest tape is rarely the useful one. On
[252289](252289-surely-my-least-cooked-at) our best is 3.836 and our keyboard
tape is 3.844, but both differ from the world record in nine ticks — and nobody
practises nine changes.

What a person can actually use is this: **take the world record's run and add
two keyboard actions.**

1. At 2.63 s, a one-tick tap of RIGHT — 66 km/h, pointed across the track, a
   right tap 50 ms before you turn left. It sets the attitude for the corner.
2. At 2.89 s, lift the throttle for 50 ms, then full throttle again at 2.94 s —
   73 km/h, full left lock, with the seam between the two lanes passing under
   the car at 2.97 s.

**3.848 validated, three milliseconds under the author time.**

And the pair is irreducible, which is why nobody found it:

| change, on the WR's own tape | time |
|---|---|
| the lift alone | 3.858 (−9 ms) |
| **the tap alone** | **4.189 — catastrophic** |
| both | **3.848** |

The tap only makes sense because the lift follows it. A field grinding one
variable at a time will never find a pair where one half is a disaster on its
own — and on a hunted map, that is precisely what is left after the route has
been ground flat.

**So the last step of every map should be: what is the smallest edit to a run
the field already drives that gets under the author time?** Ablate down to it
deliberately. It is usually a better artefact than the optimum.

## When a search stalls, suspect the ruler before the road

On [279197](279197-fall-2025-01-reverse-cp1-end) a search seeded with the human
world record reached the author time in **28 seconds**, reached 10.596 in seven
minutes, and then stopped dead for **1.7 million evaluations**.

It had not run out of road. It had run out of *resolution*. The car crosses the
line at its terminal 94.9167 m/s, so one reported millisecond is **9.49 cm** —
and the oracle's integer answer is quantised into uneven bins up to **15 cm**
wide, with 10.599 unreachable entirely. Any true gain smaller than the current
bin is simply invisible to the search: it looks like no improvement, forever.

The fix is to make a finer ruler. A "CP1 End" map has exactly one waypoint and
it is a **relocatable item**, so the finish plane is ours to place — and
re-timing through the game's own trigger means no model and no calibration. A
**ratchet** then re-aims the plane a hair past the champion's staircase edge
each cycle, so the smallest true gain reads as a whole millisecond.

The concurrent control is the proof: a real-map arm sat at 10.596 for **41.9
minutes and 1,337,400 evaluations** while the ratchet went 10.596 → 10.595 →
10.594, and the ladder predicted the untouched oracle every single time.

Three preconditions, each of which cost a cycle before it was understood:

- **Auto-calibrate the ladder on the incumbent every round.** A fixed ladder goes
  blind after one edit — measured on another map: 1,724 improving candidates,
  then 0.
- **Make it two-sided.** A one-sided vernier cannot distinguish "worse" from
  "equal", and a beam then sorts by array index straight into full lock.
- **Resolution decides whether a gradient exists at all.** Same sweep: 0
  improving candidates at 28 µs rungs, 1,964 at 1.9 µs.

So "the search has converged" and "the search can no longer see" produce
identical logs. Before concluding a map is at its floor, check what one
millisecond is worth in metres.

## A synthesised tape carries its template's telemetry, not its own

Every ghost file this project makes — a search output, a splice, a transplant, a
poke override — is the **template** with the *input bits* rewritten. The
recorded entity data is copied verbatim and never updated, because nothing
re-simulates the file in order to write it.

So decoding a candidate returns the **seed's** position, velocity, attitude and
ground contact. No error, no warning, entirely plausible numbers. A candidate on
[285885](285885-finish-is-on-the-roof) decodes byte-identical to the human world
record it was seeded from:

```
md5 32165f9fa47f41d4377b022d401545d2   decode(candidate).csv
md5 32165f9fa47f41d4377b022d401545d2   decode(seed).csv
```

**What it cost, in one hour, in one case.** An agent read `vy = +9.2 m/s` off a
candidate, derived a 10.7° climb through the finish trigger, concluded a lateral
fix was a net loss, and was about to redirect the search. The truth was a
**descent of 1.1 m/s** — the opposite sign. The telemetry was describing the
world record parked at a hairpin 120 m away. A second agent hit the same trap
from a different construction path and called it "a confident wrong answer,
silently and self-consistently".

**Why it is easy to fall for:** the decode succeeds, the splits and race time
match the template so nothing looks odd, and it is wrong *only where the
candidate diverges from its seed* — precisely the part under investigation. **The
better the search, the more wrong the telemetry.** Downloaded human ghosts are
fine; this is specific to files we made.

**The rule: any state claim about a synthesised tape must come from a
re-simulation, or from a probe that goes through the oracle.**

**The substitute, which needs no re-simulation and works on a tape that DNFs:**
use a gate as a ruler. Fix the horizontal position, sweep the gate's height, and
read the fire times — they fall as the ceiling rises and then **saturate**.
Saturation is footprint entry; the slope below it is the car's vertical speed.
Two rungs 20 mm apart gave a sink rate to ±0.1 m/s in about 40 seconds of oracle
time.

## One disease, five costumes

Everything above, plus the phantom incidents, is a single failure mode: **an
instrument whose broken state is indistinguishable from its working state unless
you deliberately make it say the other thing.**

1. **A search that reports times it did not earn** — two processes sharing a
   staging root swap candidate files. 7 phantoms in 13 shared runs, 0 in 8 with
   distinct roots.
2. **An evaluator that can only say no** — a foreign-template shortcut that could
   never return a finish. ~144,000 DNFs read as "the target is a needle".
3. **A constraint that silently does not bite** — a "keyboard" search producing
   tapes with 150+ steer values.
4. **A gate probe that fabricates discoveries** — a model swap that quadruples
   the trigger volume, so everything fires early *including the null case*.
5. **A decoder that answers about the wrong run** — the telemetry trap above.

The defence never changes and is always cheap: **make the instrument say the
other thing, on purpose, before you trust it.** A search must reproduce a known
time. A constraint must fail a one-level ladder. A gate probe must return to its
origin and reproduce the untouched map to the millisecond. An evaluator that
cannot finish anything must be shown finishing something. A decode of a
synthesised file must be checked against a re-simulation.

And the corollary that caught two of these: **when a sweep comes back empty,
suspect the enumeration before the hypothesis.** A negative from a hand-listed
set is worth nothing — one such negative was published here as a hard limit, and
the answer was the two items never on the list.

## When a whole sweep comes back empty, suspect the harness before the physics

Twice tonight, on two maps, a sweep returned an unbroken wall of failures that
looked exactly like a law of nature — and both times it was the tooling.

On [Angustus](238835-turtle-trial-angustus) a checkpoint-to-checkpoint sweep
came back empty across **897 candidates in two independent forms**, over the
whole segment. It was one step from being written up as "this attempt is
uncuttable". The cause: the cut tool applies operations in the order given, so a
cut list must be **descending by tick** — a low cut ordered before a high one
makes every later index stale. Reordered, the very first candidate tested was
exact.

On [284238](https://trackmania.exchange/maps/284238) a deletion sweep over a
3-second stall produced **zero finishers in 1,194 candidates**, and was recorded
as a physical stall. That one may be genuine — but it was swept in the direction
that finds nothing (see the trial-cutting rules: sweep the *cut point*, not the
graft point), so the negative does not mean what it appears to.

Both have the same shape as the container-portability false negative, where a
hand-enumerated list of fourteen chunks omitted exactly the two that mattered,
and the result was published as a hard limit.

**A negative that looks like a law of nature is the one to distrust.** Before
filing it: does the harness reproduce a known-good case? Is the sweep in the
direction that can find the answer? Was the enumeration exhaustive or
hand-listed? All three are minutes of work against hours of misdirection.

## The pattern, restated: five instruments and two enumerations

The recurring failure on this project is not a wrong answer. It is **a
confidently right-looking answer from an instrument that could only ever have
given that answer.** Five of them, all found in one night:

| costume | what it looked like | what caught it |
|---|---|---|
| shared staging root | improvements that did not exist | re-validating every banked best |
| foreign evaluation template | "the target is a needle" (~144k DNFs) | relocating the gates onto the spawn — proving it *can* finish |
| alphabet constraint not applied | a healthy "keyboard" search | the zero ladder — demanding `finish 0%` |
| gate probe that swaps the model | a discovery worth 10 seconds | returning the gate to its origin |
| decode of a synthesised tape | plausible telemetry, wrong run | comparing against a re-simulation |

And two enumeration failures with the same signature: a hand-listed chunk sweep
that missed the answer, and an operation-ordering bug that made 897 valid
candidates fail.

**The defence is always the same and always cheap: make the instrument say the
other thing, on purpose, before you trust it.**
