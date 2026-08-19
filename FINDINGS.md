# Results

One block per map. Times are validated through the plain oracle
(`TrackmaniaServer /nodaemon /validatepath=`) against the untouched map file,
with a known-answer human ghost as an identity control in every batch.

---

## 227969 — Great wtf of what #165 (uid `LtSUTxJ71u7ayvLj57wUdVPyH2h`)

AT **8.127** · human WR **8.197** (Titoch_tm) · 42 recorded runs · gap 0.070

**Author time beaten. Full write-up and driving guide: `227969/RESULT.md`.**

| tape | time | vs AT | steer events | distinct steer values | device |
|---|---|---|---|---|---|
| keyboard, 14 inputs | **8.075** | **−0.052** | 12 | 3 | keyboard |
| keyboard | 8.058 | −0.069 | 23 | 3 | keyboard |
| action keys, 8 detents | 8.050 | −0.077 | 54 | 15 | pad |
| analog, event-thinned | 8.021 | −0.106 | 62 | 50 | TAS |
| analog, unconstrained | **7.998** | **−0.129** | 185 | 111 | TAS |
| (human WR, for scale) | 8.197 | +0.070 | 11 | 3 | keyboard |

**Headline:** the author time falls **on a keyboard**, with the same three
steering values and essentially the same number of key presses the human world
record uses (12 steer events vs 11), 0.122 s faster.

**Where the time is:** nothing over the first 6.5 s (we are +0.010 s down there);
all 0.187–0.199 s in the last 1.4 s.

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
  from the keyboard WR converged near 8.140; the arm seeded from the rank-2 *pad*
  run (0.031 s slower as a human run) produced everything below 8.130. A 3-value
  tape has almost no local neighbourhood for the operators to work in.
* **Quantising an optimised analog tape does not work at any resolution.** Even
  a 64-level ladder (max change ±1/127 per tick) makes the 7.998 tape DNF. Low-
  input tapes must be *searched for* under the constraint. Seeding the
  constrained search from a human keyboard run — already legal in every ladder —
  reached 8.102 in 80 s.
* **Establish the input alphabet from the data.** The human WR's own tape
  contains exactly `{-127, 0, +127}`; that is ground truth for "keyboard", not
  an assumption.
* **Greedy event deletion is cheap and effective within an alphabet**: 20 → 14
  events for zero cost on the keyboard tape, 185 → 62 steer events for 0.023 s on
  the analog one.
* **Post-finish ticks are inert** and deleting them is free — worth doing first,
  it removes a third of the events on a tape.
* **INCIDENT — the sub-tick plane surrogate requires the finish to be crossed
  with a repeatable attitude.** On this map (airborne finish, roll varying over
  1.5 rad across the field) the trigger is body-based and a fixed plane is wrong
  by up to 1.30 m ≈ 0.019 s. It produced a self-consistent 7.990705 that the plain
  oracle calls 8.004. Per-seed calibration was exact and the whole-tick guard
  passed, so nothing internal catches it. Specimen in
  `tm-loop/phantoms/m165-subtick-plane-20260818-1752/`. Detail in
  `227969/RESULT.md` §7.
* `p37` on this leaderboard (8.610) re-simulates to **8.477**. Flagged, unused.

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

## Single-window findings do not compose — and neither do single-window searches

Three maps have now hit the first half of this, and one map measured the second.

**Findings.** On [203330](203330-get-in-the-hole-impossible) the brake is
deletable over *every individual sub-window* at no cost, and removing it
everywhere at once does not finish the map — it tolerates ~70 ms of slack, so
each local deletion is absorbed by its neighbour. On
[286279](286279-turtle-trial-leto) blanking one tick range is free, blanking the
next is free, and blanking the **union** DNFs.

**Searches.** The same map ran the two directions side by side for 49 minutes on
one incumbent:

| arm | candidates | gain |
|---|---|---|
| single operator, 20 µs resolution | 1,054,194 | **1 microsecond** |
| up to four operators, 6 ms | 739,558 | **33 microseconds** |

**A million single moves at fifteen times finer resolution bought one
microsecond.** The only direction still producing anything was combinations.

So when a map plateaus, the answer is not a finer single-move scan — it is pairs
and triples of the operators that matter. (Honest footnote from the same
measurement: the multi-move arm is not a rich seam either. 33 µs in its first 23
minutes, 2 µs in the next 25, decelerating.)

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

A second map found the same inversion from the other side, without moving
anything: **the validator reports `ceil(t_true)`, so it is a 1 ms ruler — but if
you know the incumbent's true crossing sits 0.304 ms above the integer boundary,
then any candidate better by 0.304 ms reports one millisecond lower and
everything else does not.** *A coarse ruler is precise when you know where on it
you are standing.* That turned 590,370 perturbations into a floor argument
adjudicated entirely by the plain oracle, with no surrogate in the acceptance
path. **Precondition, and it is the part that will get dropped first: the
crossing must be plane-verified, or there is no claim** — on a map that fails the
crossing-spread test the diagnostic is uninformative in both directions.

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

## What a minimiser can and cannot tell you

Two maps were minimised with the same tool, and the contrast corrects a claim
that was about to travel as a general law.

**The claim.** On [Leto](286279-turtle-trial-leto), the 218.812 tape reduces from
885 input events to 832 and no further — 1-minimal, every remaining single
deletion tried and failing. Of 177 single-deletion positions, **171 killed the
run and *zero* merely cost time**. That reads as "on a TAS tape a missing input
does not cost you time, it kills the car".

**The correction.** The same exhaustive test on
[203330](203330-get-in-the-hole-impossible), a short map with 8 ms of headroom
rather than 136 seconds:

| tape | deletions that merely COST TIME | that KILL | in budget |
|---|---|---|---|
| Leto, 218.812 (136 s headroom) | **0 of 177** | 171 | 6 |
| 203330, 12-input (8 ms headroom) | **2 of 11** | 9 | 0 |
| 203330, 31-input | 3 of 30 | 9 | 18 |

So lethal degradation is **not** a property of TAS tapes. It was a property of a
turtle map whose headroom had already been harvested out of it by the
author-cut. 203330 degrades lethally in its spine — seven inputs before 6.5 s
that no version of the line can do without, the same nine fatal ticks appearing
in two independent lineages — and **gracefully in its endgame**.

**Two independent lineages converge on the same floor.** Minimising 203330's
12-input tape and its 31-input optimum (a different line, 19 more inputs, one of
them 2 ms faster) both terminate at **twelve events**. That upgrades the driver's
document from "this is the tape we happened to thin" to "the fast line on this
map costs twelve inputs whichever end you approach it from".

**But check provenance before calling two starting points independent.** The same
claim was made for Leto — 832 events from the optimised run, 831 from the
author's raw lap — and retracted within the hour: the optimised run *is* the
author's lap, cut and then searched, so both minimisations were consuming the
same inputs. The tell was there to be read: identical steer counts, identical
brake counts, two apart on accel, and event sequences agreeing everywhere except
about twelve events out of 862. **Two genuinely independent minimisations of a
220-second map do not agree that closely.** Near-identity means shared ancestry,
not corroboration — so diff the surviving event sequences and compare event-class
counts before publishing a convergent-floor claim.

**And a minimiser measures MARGINAL freedom, not joint freedom.** Blanking ticks
25290–25399 of Leto's author lap is free; blanking 25400–25834 is free; blanking
**the union DNFs**. So "6 of 177 single deletions are in budget" says nothing
about whether those six are jointly removable — and by symmetry, a position that
looks dead may be alive in company. (Delta debugging is safe here because it
evaluates cumulative prefixes and a bad combination simply fails to be accepted;
a one-at-a-time probe is not.) The nearest available bound: 119 adjacent pairs on
the 1-minimal tape, all 119 dead.

**Three rules follow.**

- **1-minimality is per lineage.** "No input of this run is removable" is not "no
  simpler run beats the author time". A deliberately slow, simple line is ruled
  out by nothing — testing it needs a search under an event-count penalty from a
  *slow* seed, not a minimiser on a fast one.
- **Report the failure mode, not just the count.** `reached_cps` on every death
  is what distinguishes "the car needs that input" from "the deletion derailed
  something downstream" — every dead run above reached exactly the checkpoints
  before its deletion point and never one more.
- **Prefer delta debugging to greedy thinning.** O(n log n) against O(n) passes
  of O(n) evaluations, and it terminates 1-minimal — "no 11-input version of this
  line exists within the budget" is a materially stronger published claim than
  "greedy stopped at 12".

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

## Most of our replays were showing the wrong run — a fleet-wide survey

A `.Ghost.Gbx` carries *two* things: the input stream the validator re-simulates,
and recorded telemetry samples the game plays back. Every file this project
synthesises rewrites the inputs and copies the samples verbatim, so **the replay
you watch is the template's run, not the one the file validates as.**

A survey of **194 tapes across the collection found 48 on 9 maps showing the host
container's run rather than their own** — 286279 ×11, 238835 ×9, 199100 ×7,
197047 ×6, 126859 ×6, 285885 ×5, 227654 ×2. It is not a mistake anyone made; it
is what transplanting a tape into a host container does.

**Eight files were repaired** by cutting the sample data to match the input cuts,
and now play the run they claim (validated three times each, controls in every
batch): both Leto author-cuts (220.391, 220.821), both Leto human-cuts (236.972,
237.122), Angustus's author-cut (246.602) and its two earlier no-retry cuts
(347.003, 407.463), and 227654's cut of rank 1 (59.912).

**Three classes cannot be repaired, and the boundary is honest:**

- **Searched tapes.** A mutation search changes steer values, so the tape is not
  a subsequence of any recorded run and no samples of it exist anywhere. Leto's
  218.812 and 218.877 stay validator-only.
- **Cut + injected respawn.** Substituting a respawn press mid-tape sends the car
  down a path nothing recorded. Angustus's 262.907 and 239.133 are in this class.
- **Unmodified tape in a foreign container.** The tell is a cut spec with *zero*
  drops — the aligner reports a "pure cut" that removes nothing.

Making a searched tape watchable would need a simulator that emits telemetry,
which is a different project.

**A ghost's vehicle track is not always one entity.** 227654 records it as **27
type-2 entities**, one per respawn segment. A scanner that takes the first
matching entity reported that map's *healthy* files as running 145 seconds past
their declared finish — wrong output from a new instrument on its first
fleet-wide run. Merge all vehicle entities in time order.

## Which recording in a multi-node ghost is the author's lap?

A ghost can hold many recordings, and index 0 is not reliably the one you want.
The rule, in order:

1. Prefer a whole `CGameCtnGhost` blob if the file has one.
2. Else match a recording's end time to the author time, or to **AT + ~2.96 s**
   — the countdown lead-in, and the only two values observed across five maps.
3. Else, **if the nodes start at different positions, they are not laps.**

146612 has 25 nodes — 13 distinct recordings, the first twelve duplicated —
starting at thirteen different places in six clusters, several mid-map, ending
nowhere in common, and no end time matching either target. It has **no author
lap**, and a file previously published for it on the index-0 assumption was
withdrawn by its own author when the rule caught it.

## Reward shaping is inert by construction — a proof, not a measurement

The obvious cure for a stalled search on a long map is **reward shaping**: score
partial progress through relocated intermediate gates, so a candidate that gets
further scores better even if it does not finish. It was about to be prescribed
to three maps. It is dead weight in every search this project has ever run, and
the reason is arithmetic:

```
score_finish(t) = FINISH_BASE − t   = 1e8 − 14600  ≈ 9.9986e7
score_dnf(k,t)  = k·SEG_UNIT − t    ≤ 4e7           (4 stations)
```

With a **finishing** incumbent, the acceptance delta for the best possible
shaped non-finisher is −5.998e7; at T=25 that is `exp(−2.4e6)`, zero in double
precision. Confirmed empirically — every accepted state in the shaped arm scored
≥ 9.99e7, all finishers, no exceptions.

The finisher-always-outranks-non-finisher invariant is *correct*: it is what
stops a ladder optimising "fast to CP3" into a run that finishes a second
slower. But it means **once the incumbent finishes, every non-finisher is
strictly worse however far it got** — and **we seed every map from a downloaded
human run, so the incumbent finishes from the first evaluation onward.**

The A/B, since a proof still deserves a measurement (concurrent, same box, same
seed, distinct staging roots, 20 min, 40 workers each):

| arm | best | evaluations | rate |
|---|---|---|---|
| sparse control | **14.589** | 350,130 | 290/s |
| dense, 4 stations | 14.669 | 186,066 | 152/s |

**1.88× more expensive and 80 ms worse** — and it is cost, not quality: at equal
evaluation counts the trajectories coincide.

**Where shaping WOULD pay**, stated and explicitly not tested: only where the
incumbent itself is a non-finisher — a first finish from a broken state, or a
route change that must cross a DNF valley.

**Two variants that would not be inert.** First, and worth trying on any map
that plateaus: **stations as a tie-break among finishers**, scoring
`(finish_ms, station_time)` lexicographically. It only ever compares finishers,
so it can never be eaten by the invariant, and it supplies a gradient on a
plateau of equal milliseconds — the same shape as the vernier that broke two
CP1-End maps open. Second: a separate explorer island scored only on stations,
migrating into the main population when it produces a finisher.

**A general lesson about scoring functions.** This defect is invisible in a log:
the shaped arm runs, accepts states, and improves — it just improves for reasons
that have nothing to do with the shaping. Before adding a term to an objective,
check it can actually change an acceptance decision in the regime you will run
it in. That is the same discipline as the instrument controls above, applied to
the objective rather than the apparatus.

**A ladder with too few rungs is indistinguishable from a flat landscape.** The
first pass of a height-ladder objective used four rungs and collapsed every
perturbation into "fires nothing below the top rung" — no gradient, and the
agent nearly concluded the neighbourhood was a cliff. Five extra rungs spread the
same population across six levels. The failure presents as *"no gradient
exists"*, not as *"my instrument is too coarse"*. Pair it with the vernier
finding on another map: the same sweep gave 0 improving candidates at 28 µs
rungs and 1,964 at 1.9 µs. **Resolution decides whether a gradient exists at
all.**

**And validate an objective against known artefacts before validating the
search.** A proposed objective for another map — maximise the dwell time inside
a trigger volume — was caught before implementation because a candidate that
sinks *faster* reaches the reference height *sooner*, so the metric moves
backwards exactly when the run gets better.

It was then measured, on 13 perturbations of the fast tape plus the human world
record, and the metric turned out to carry **no** information at all: at
identical clearance, dwell ranged from 6 to 33, and the two tapes sharing the
best rung had dwells of 20 and 37. Twenty minutes of checking a proposed metric
against tapes whose answers are already known is cheap against a multi-hour
search pointed the wrong way.

## A control validates the property it exercises, and nothing else

The origin round-trip — move a gate back to where it started, confirm the
untouched map reproduces to the millisecond — was adopted as *the* standard for
trusting a gate probe, after a model-swapping mover fabricated a ten-second
discovery on one map and deleted the finish on another.

It is necessary. It is **not sufficient**, and here is the case that proves it.

A 14.7-minute search against a segment map reported **−13.975 s**. The winning
tape collects only CP1 and CP2 on the untouched map. It had not found a route —
it had found the promoted gate's **enlarged trigger volume**.

**The origin control passed the entire time.** The gate's *position* was restored
correctly, so the round-trip reproduced exactly. The defect was in the *volume*,
and a control that exercises position cannot see volume.

| control | catches | misses |
|---|---|---|
| origin round-trip | a mover that shifts, rotates or deletes the gate | **a gate whose trigger volume differs from the one it replaces** |

This is the seventh instrument failure catalogued on this project and **the first
to defeat a countermeasure built for exactly its family.** It also produces the
most attractive kind of wrong answer: not a subtle error, a *fourteen-second
improvement*.

> **When you adopt a control, write down what it cannot see — and check whether
> the thing you are actually worried about is on that list.**

The fix is to make the substitution incapable of the defect rather than to add
another check: a position-only mover that never touches the model, or a finish
placed beyond a real checkpoint so the trigger the candidate must satisfy is the
map's own. And, as always, re-validate every improvement on the untouched map
before believing it — that is what exposed the 14 seconds as two checkpoints out
of four.

## Fewer inputs is not automatically more drivable

The low-input work in this repo rests on an assumption worth testing: that
reducing input count makes a tape easier for a human. On
[249521](249521-impossible-at-for-ssano) it was tested, and the assumption
failed.

Thinning that map's keyboard tape from 54 events to 30 made it **slower *and*
less forgiving** — survival under ±1–4 tick mistiming fell from **41% to 10%**.
The remaining inputs each had to do more work, so each one mattered more.

A related bad trade from the same map: a robustness re-placement pass bought
**2 percentage points of survival for 130 ms**.

**So measure drivability, do not infer it from event count.** The right test is
the one used there: perturb each input by ±1–4 ticks, let the rest re-time, and
count what fraction still finishes — **with the human world record's own tape put
through the identical test as the control.** On that map our 41% against the
human's 18% is the statement worth publishing; "54 events" on its own says
nothing.

## Check what actually orders the field before naming a technique

The visible difference between fast and slow runs on
[249521](249521-impossible-at-for-ssano) is how hard the car is swung. Peak
swing speed correlates **0.02** with finishing order across all 147 runs. "Swing
harder" would have been a confident, plausible, useless piece of published
advice.

What does order the field is the car's *attitude at the moment of the throttle
lift*: the field releases at 80–85° of heading, before the nose is square, and
the boost pads give them 0–6 km/h; the fast line releases at 90–105°, past
square, and the same pads give 25–35 km/h.

This is the third map where the flashy quantity is not the causal one — the
others being a closing corner that turned out to cost every run the same 1.1 s,
and a lock-percentage correlation that inverted once the confound (slower
drivers steer more because they are correcting) was removed.

**Before publishing a technique, regress the candidate quantity against
finishing order across the whole field.** If it does not order the field, it is
not the technique, however large the difference looks.

## A map declared unusable was beaten by 694 ms — check the verdict, not just the map

[203072](203072-yeet-fall-2024-04) was named in this project's own notes as *the*
canonical unfalsifiable map: human ghosts would not re-simulate, so no search was
ever run on it. That verdict was wrong in three independent ways, and finding
that out was most of the work:

- **The oracle is faithful there — 1.7 mm** position RMS against ghosts' own
  telemetry, across three game builds.
- **Nadeo's own map file is sha256-identical to ours**, killing the "edited in
  place" theory.
- **The failures are a bounded window of game builds.** Outside the window,
  **80/80 = 100%**. Inside, 92/190.

**The original reading came from a 34-ghost sample in which the post-window
builds had n = 1.** A full-field check (270 of 272) inverted the conclusion
entirely.

Two general rules from that. **A §8 field-reproduction failure has at least three
distinct causes** — an old build, a *windowed* build range, and a genuinely
unfalsifiable map — and they call for opposite responses, so identify which
before condemning anything. And **sample sizes in a field check are not
optional**: a category with n = 1 will happily support the wrong story.

Worth knowing what it cost: this map's author time then fell by 694 ms, and its
keyboard flight by 591 ms, on a map nobody had searched at all.

## The winning move is often a combination that looks wrong in every direction

On [203072](203072-yeet-fall-2024-04) the final tape differs from its predecessor
in three places: a 40 ms throttle lift, a 10 ms throttle lift, and one wheel left
un-unwound into the launch. A full 2³ factorial:

> **Every proper subset DNFs or is slower. All three together are worth 566 ms.**

The gradient points *away* from the answer in all three directions, so no
incremental refinement finds it — human or machine.

This is now the third instance. On [145875](145875-unlucke-get-jiggy-with-it) the
whole margin is a non-separable pair of analog details, one of which is worth
+27 ms *alone*. On [252289](252289-surely-my-least-cooked-at) the two-press
solution has a half that is catastrophic on its own (4.189 against 3.867).

**So on a hunted map, after single-move search converges, run a small factorial
over the candidate moves rather than more single moves.** A field grinding one
variable at a time cannot find a pair or triple where the parts are individually
bad — which is precisely what is left once everything separable has been found.

## An objective that can be maximised without achieving the goal is a decoy

On [Torment (1-DOWN)](228811-torment-1-down) the search had to find a launcher
that fires only on a specific *attitude*. Scoring the state rather than the time
was the right idea, and it took **four objectives**, because the first three
could each be maximised without firing anything:

| objective | what the search did instead |
|---|---|
| downward velocity alone | ran to the corner of the box |
| body-lateral speed alone | slid **along** the trigger line |
| progress along the author's line | plateaued at 86.9%, launching at the sky |

Two related traps from the same map: **peak speed is useless as a launch
detector** — the human world record itself hits 151 m/s without launching — and
**a near miss can outscore an arrival**, which inverts the ranking precisely
where it matters.

This is the instrument disease applied to the *objective*. The defence is the
same shape as everywhere else in this file: **before running a search, ask what
the best possible score looks like for a candidate that does not do the thing.**
If that score is competitive, the objective is a decoy.

A second instance from the same session: a proposed objective of "maximise the
dwell time inside a trigger volume" was caught before implementation, because a
candidate that sinks *faster* leaves sooner — the metric moves backwards exactly
when the run improves. Measured afterwards, dwell ranged 6 to 33 at identical
clearance: no information at all.

## Bank the control ghosts, not just the tapes

Twice tonight a map's result arrived with every claim validated and **no human
ghosts in the durable directory** — the controls had been used from `/tmp` and
lost with the node.

The distinction is load-bearing. Re-simulating a banked tape proves *a filename
matches a simulation*. It does not prove **the oracle was answering correctly on
that map at that moment**, which is what a known-answer control is for, and it is
the difference between a result and a plausible number.

The standard, from the map that did it properly: **30 human ghosts and all 15
tapes in one batch, 45/45 exact**, from rank 1 to rank 470, reproducible by
anyone with the directory in a single command. Bank the ghosts with the first
write-up, not after somebody asks.

Related, and cheap: **check downloaded ghosts are complete before use.** A
truncated download validates as `DNF cps=1` and reads as a genuine field-check
failure — which is how a healthy map gets condemned.

## You pay for the alphabet, not the input count

The clearest measurement anyone has produced of what a low-input tape actually
costs, from the ladder on
[279197](279197-fall-2025-01-reverse-cp1-end):

- **Within a rung, fewer events is nearly free.** 66 → 57 keyboard events costs
  4 ms; going all the way down to 35 events costs 10 ms.
- **Across rungs, the value alphabet is expensive.** 30 distinct steer values →
  3 costs **33 ms**.

**If you want a tape someone can hold in their head, the price is in the
alphabet.** Event count is close to free once the alphabet is fixed.

**And the seed matters more than the constraint.** Same alphabet, different
starting tape: 5 detents from the analog champion is 10.702; from a human
action-key run it is 10.643. **A 59 ms swing with the constraint held fixed** —
larger than most of the margins in this repo. Seed a constrained search from a
run that already uses that alphabet, not from your fastest analog tape.

Which is the same finding as "conversion does not work", stated positively: the
analog champion is not merely a bad seed for a keyboard search, it is 59 ms
worse than the obvious alternative even when the search is done correctly.

## Verifying a low-input claim: do not use the trajectory decoder

A trap that will catch the next person who checks one of these claims.
`tmtraj decode` on a synthesised candidate reads the **template's** stale
telemetry, so decoding a quantised tape reports the *seed's* steering alphabet —
in one case 86 analog values on a tape that genuinely contained 3. The agent
nearly reported their own working ladder as broken.

**Read the input archive, not the telemetry.** `tmsimp` does this and confirmed
17 values for the 8-detent arms and 3 for keyboard. This is the same
synthesised-tape telemetry trap documented above, in the place where somebody
auditing a published number is most likely to hit it.

## Bank the map and the controls, not just the tapes

Three times in one session a map's result arrived complete, careful and
cold-validated — and impossible for anyone else to reproduce, because the map
file and the control ghosts had only ever existed in scratch space on the
agent's node.

On the largest result in this repo, **the map was one node reclamation away from
turning a −1746.748 s finding into a claim with no evidence.**

**The map is not input. It is half the evidence.** A time is a claim about a
tape *and* a map, and re-simulating a tape against a map nobody else has is not
a validation anyone can repeat.

The standard, set by the map that did it properly: **30 human ghosts and all 15
tapes validated in one batch, 45/45 exact**, reproducible from the directory in
a single command. And the check that actually proves it — **re-validate with
everything read from the durable store, with scratch space excluded**, and bank
that transcript.

> **Could a stranger reproduce every number in your write-up from this directory
> alone, with your machine destroyed?** If not, it is not banked yet.

## A rule that turns out to be a property of one tool

The project believed that cutting a tape to a standing respawn works at **one
exact, non-periodic phase** — a striking claim, measured on real data, and it
shaped three agents' work.

It is a property of **the tool that was used to measure it**. That tool splices
through a respawn-blind code path; a packet-level cutter carries the respawn bit
with the packet, and the same sweep then gives **27 consecutive survivors, all
arithmetic-exact**.

Neither measurement was wrong. The generalisation was — and it is a subtle
failure, because the phenomenon was real, reproducible, and had a plausible
physical story attached.

> **Before elevating a measured behaviour to a rule about the game, reproduce it
> with a second implementation.** Especially when the behaviour is surprising:
> surprise is exactly what a tool artefact looks like from the inside.

## A diagnostic invented on one map can be a false negative on another

The project needed to know whether a map that had never produced a single
validated time was broken or merely badly seeded. The test: **relocate the finish
onto the spawn and re-run the tape.** Still fails → the ghost is broken. Returns
a time → the map is fine and the tape diverges later.

Run on a **known-good** map as a control, minutes apart on the same machine:

| | result |
|---|---|
| control map, untouched | `Time 886277`, `IsValid true` |
| control map, **Goal only** moved next to the spawn | **`null`, `wrong simu`** |

**A map that validates perfectly reproduces the failing map's exact signature
under the test.** The reason is simple once seen: **a finish only counts once
every checkpoint has been collected.** The recipe was invented on a map with *no
checkpoints*; the maps it was being applied to have eleven and twelve.

The repair is to relocate the checkpoints too — stacking eleven waypoint blocks
in one cell is legal, and on the control map K relocated checkpoints report
exactly K.

Run correctly, the answer **inverted**: the map validates, and its tape
simulates faithfully for at least 96 seconds — past two real checkpoints and its
own first respawn — where the bare failure message had implied it died inside the
first.

**The lesson is not about gates.** It is that a diagnostic carries the
assumptions of the map it was invented on, and those assumptions are usually
invisible in the recipe. **Run a new diagnostic on a known-good case first** —
the control here cost one validation and saved a wrong conclusion about a map
three agents had already worked.

And the asymmetry worth carrying: **a firing rung is proof; a silent rung is not
evidence.** Six of twelve rungs returned nothing on a map that provably
validates, and a gate one cell along from a firing one may simply not fire.

## A respawn is an input — and whether its state is canonical is a property of the map

A respawn is not a break in a recording. It is a **packet in the input tape**,
carried in **bit 31 of the packet's 34-bit state literal** (literal
`0x80000002`). That matters twice over.

**It is invisible to the obvious tool.** The ghost factory does not surface it,
and counting discontinuities in the *telemetry* is not the same measurement: one
record with 941 respawn presses shows 914 telemetry jumps. **Enumerate the
packets, not the jumps.**

**It is editable.** Which means a tape can be *constructed* around one, and that
is what broke open a map where the only human record was a 2 h 26 m retry grind
with no finish in it:

```
[ any prefix reaching race t = 1.670 s ] ++ [ respawn packet ] ++ [ the winning attempt ]
```

On that map the construction finishes at exactly `(K + L)·10 − 1540` ms for every
prefix length K, swept across 4 700 ticks — perfectly linear, including from
mid-flight at the speed cap. Mutate the prefix 3 000 times and 140 finish, **every
one at the same millisecond**. The 1 885 ticks after the respawn replay
identically regardless of what the car was doing before it. That produced the
first finishing tape anyone had ever had on the map, which is what made a real
search possible.

**But that property does not travel.** On another map the same week, grafting a
record's own last respawn and winning tail onto its own prefix works perfectly
and lands exactly on the arithmetic — while grafting the *same tail* onto a
searched line finishes **0 of 31 times**, and 0 of 124 over a (cut, shift) sweep.

The difference is what "respawn" restores. On the first map it restores a fixed
spawn state. On the second it restores **the run's own checkpoint crossing
state**, which a different line does not have.

> **Check which one you have before you plan around it.** One validation each
> way: sweep the cut point and see whether the finish time is linear in it. If it
> is, the state is canonical and transplants are free. If it is not, they are not
> available at all.

And a third thing follows: a respawn route can have a **hard floor**. If the
respawn cannot be armed before race *t*, every tape built on one is bounded below
by *t* plus the tail — on that first map, 16.1 s against an author time of
15.643. **The construction was the instrument, not the deliverable.** It is worth
saying because it is easy to mistake a legal finishing tape for progress toward
the target when it is actually a scaffold you will throw away.

## The oracle is ~1000× slower than it needs to be on a tape cut from a long recording

Which is most tapes, on most maps. Measured on a 2 206-tick tape cut from an
8 790 769 ms record:

| | before | after |
|---|---|---|
| a finishing candidate | 2.7 s | **0.03 s** |
| a DNF candidate | 32 s | **0.34 s** |
| throughput, 150 workers | 14.5 cand/s | **~500 cand/s** |

Three independent causes. All three are invisible: nothing warns you, the numbers
are simply correct and slow.

**1. The "strip the telemetry" flag strips nothing, and never could.** The
recorded telemetry (`CPlugEntRecordData`, `0x0911F000`) is **not** a top-level
skippable chunk. It is written inline inside the ghost chunk `0x03092000` as
`id | version | uncompSize | compSize | zlib`. A top-level chunk walk that filters
on class-id top bytes `{0x03, 0x0B, 0x24, 0x2E, 0x30}` can never match `0x09`, so
the flag is a silent no-op. The blob inflates to 24 MB *per candidate* — memory
bandwidth, so it gets *worse* with more workers. Check a "stripped" template's
size before believing it.

The fix is to re-encode the record with the same header, descriptors and notices
but **zero samples**, then shrink the enclosing chunk header. 1 914 181 → 5 425
bytes, same millisecond.

**Do not just empty the blob.** Setting the uncompressed size to 0 with an empty
zlib stream produces a file the server **refuses to load, silently**: the ghost
vanishes from the batch, there is no diagnostic, and the caller reads no time at
all — indistinguishable from a DNF.

**2. A DNF is simulated all the way to the DECLARED race time.** A tape cut out
of a 2.4-hour record still *declares* 2.4 hours, and a run that never crosses the
line is simulated to that clock — **independent of the tape's own length** (a
300-packet DNF cost 22.5 s; a 1985-packet DNF 17.7 s). The declared time lives in
**four** places, and the race-time chunk everybody knows about is not the one
that governs:

```
0x03092005              drives the walltime -- changing ONLY this leaves the DNF cost intact
0x0309200B  +12
0x0309201B  +10
0x0309202B  +4 and +32  (the splits chunk)
```

Rewrite all four and a DNF goes 17.7 s → 0.34 s, finishers unchanged. Bonus:
declare just above the incumbent and everything slower comes back DNF, which is
free pruning for a minimising search.

**3. The batch scheduler silently caps the worker count** at
`min(workers, ceil(n / batch))`. With a default batch of 600, a 1 500-candidate
round runs on **three** workers however many jobs you asked for. Symptom: 100
jobs and a load average of 3.

> **Time one candidate through the plain oracle — as a finisher and as a DNF —
> before you size a search.** A short template that is 1.9 MB, or that declares a
> race time from the recording it was cut out of, is costing you two to three
> orders of magnitude.

## A relocated gate is a PLANE, and the plane's axis is a byte

This repairs the "relocating a gate is the safest map surgery available" finding
above, and it explains a mystery this project had lived with for a while: *why do
roughly a third of well-chosen probe placements simply not fire?*

A goal gate relocated to cell `(cx, cy, cz)` does not fire when the car is *in*
the cell. It fires when the car crosses **one plane** through the cell centre,
and the block's `dir` byte — immediately before the three cell bytes in the same
record — chooses which:

| `dir` | trigger plane | fires at |
|---|---|---|
| 0, 2 | z-plane | `z = 32·cz + 16` |
| 1, 3 | x-plane | `x = 32·cx + 16` |

Predicted from a world record's own trajectory *before* the maps were built, then
measured: −23 ms, −24 ms, −67 ms. The lead is the car's nose — 23 ms at 81 m/s is
1.9 m — and it is consistent every time.

So the silent rungs were never unlucky placements. **They were gates whose plane
the car was running parallel to.** A silent rung now has a first hypothesis you
can test in one run: flip `dir` and try again before you conclude anything about
the tape.

Two corollaries:

**You may not need a model swap at all.** The volume question that makes gate
surgery dangerous only arises if you *promote* a checkpoint into a finish. Look
first for gates that are already goal-model: on one map a naive listing showed
five waypoint blocks and the un-baked listing showed **nine**, four of them
already tagged Goal and sitting on top of the finish road blocks. Relocating one
of those is position-only — overwrite three cell bytes — with no promotion, no
volume question, and no hole left in the track.

**A wide rung is a decoy generator.** A 4-cell curtain produced a march winner
316 ms ahead of the best known tape. It was the car **off the right-hand side of
the road, airborne**, having left the track 40 m earlier. Nothing in the ladder
output distinguished it; every internal control passed. Make the rung as narrow
as the road, and **decode every march winner's own trajectory and check it
against the road cell's x-span before you believe its number.** Two commands.

> A gate ladder measures "did the car cross this plane". That is not the same
> question as "did the car drive this track", and the gap between those two
> questions is where a decoy lives.

## A greedy per-station crawl locks in its own accidents

A ladder turns "distance along a sector" into a millisecond, which makes an
unsearchable plateau searchable: on one map, **0 finishers in 207 000
evaluations** with only the finish as an objective, and **13 of 22 stations
climbed** once each station became its own objective. The obvious way to use it
is a crawl — optimise arrival at station *k*, take the winner, seed station
*k+1*.

Delta to the human world record, per station:

| st02 | st03 | **st04** | st06 | st08 | st10 | st12 | st14 |
|---|---|---|---|---|---|---|---|
| −0.501 | −0.231 | **+1.232** | +1.416 | +1.728 | +1.891 | +2.161 | +2.601 |

**The entire run is decided at one station.** st03 → st04 is 1.813 s for 28 m — a
wall contact that dropped the car from 74.5 to 22.6 m/s. Every station after it
inherits a dead run, and the crawl spends the rest of its budget nursing one. The
tape that comes out at st14 is 2.601 s behind a run that was 0.501 s *ahead*
twelve stations earlier.

Nothing in the crawl notices. Each station reported an improvement over its own
seed, every result validated, no phantom, no error. The greedy accept is doing
exactly what it was told.

Three fixes, cheapest first:

1. **Watch the delta, not the absolute.** A jump in arrival-minus-reference is
   the signal. Re-run any station whose delta jumps before continuing past it.
2. **Keep the best *k* per station, not the best one.** A beam of 3–4 costs 3–4×
   and is the difference between finding a line and polishing a crash.
3. **Score arrival at station *k+2..3*, not at *k*.** This addresses the cause.
   "Fastest to station *k*" is satisfiable by a tape that arrives fast and
   pointing at the outside wall; "fastest to station *k+3*" is not, because a bad
   exit cannot get there.

The general form of (3) is worth stating on its own: **optimise arrival PAST a
checkpoint, never at it.** On that map "fastest to CP5" bought a state 1.128 s
ahead of the world record that could not use its own speed — it overshot the road
entirely and came down on the outside wall. A ladder makes the better objective
cost exactly the same to evaluate as the worse one.

## Suspect the rung spacing before you believe the wall

Two agents established a 70 mm clearance as immovable across ~57 000 evaluations,
with every control passing. They were measuring against a **10 mm rung**.

A search of the same class, scored on a **1 mm** ladder, moved it in 35 712
evaluations:

| clearance | evaluations to reach it |
|---|---|
| the wall | 0 finishers in ~57 000 |
| −2 mm | 384 |
| −3 mm | 2 304 |
| −4 mm | 5 376 |
| −6 mm | 12 672 |

10 mm was five to twenty-five rungs of the gradient that actually existed. **A
negative from a rung the population cannot reach in one mutation says nothing
about the rungs in between.**

This is the "suspect the enumeration before the hypothesis" rule one level down —
the enumeration in question was hidden inside the *instrument's resolution*, not
in the candidate set.

It did not save the map: cost per millimetre roughly doubles, so the remaining
64 mm is out of reach by that route. But converting a wall into a measured cost
curve is itself the result. It says what a winning lever must look like —
something that buys tens of millimetres at once — and it stops anyone else
grinding local mutations at the wrong granularity.

## A negative needs a detector proven able to say YES

Sharper than "a negative result requires a positive control", and it caught a
live error: the *instrument* has to be shown to fire on a tape that
demonstrably does the thing.

A slope-route negative reported "0 of 5 940 launch-sweep tapes reach any gate on
the finish platform". The cheapest possible check — does the map's own **finishing
tape** fire those gates?

```
finishing tape vs (1005,50,665) -> DNF     (1012,50,660) -> DNF
               vs (1000,52,668) -> DNF     ( 996,56,690) -> DNF
```

A y-sweep explained it: the car crosses x = 1005 at y ∈ (50, 52), and only a gate
at y = 54 brackets it. The two gates at y = 50 and y = 52 sat **on either side of
a 6 m window without containing it** — about four metres out, which on that
trigger is the whole window. A gate is a small asymmetric box, so one four metres
off is **silent, not approximate**.

Re-run with detectors that fire on the known-good tape, the negative *survived* —
0 arrivals earlier than the incumbent's own out of 5 672 hits. But it became a
negative about *perturbations of one line* rather than the sweeping claim that
nothing can reach the platform. That is the difference the control buys even when
the answer does not change.

## "Trial" is sometimes a building style, not a respawn mechanic

Several of the biggest margins in this repository come from deleting retries out
of a recorded time, because on a Trial-family map the clock runs through
respawns. It is tempting to reach for that on anything tagged Trial.

One map tagged *Mini Trial* reports:

```
NbRespawns 0     NbCheckpoints 1
```

**One checkpoint** means the only waypoints are the spawn and the goal. There is
nowhere to respawn *to* except the start, with the clock running. There are no
retries to delete and the entire family of techniques is inapplicable — the tag
described how the map was built, not how it is timed.

> **Check `NbCheckpoints` before assuming a trial map's time is mostly retries.**

`NbRespawns` is a first-class field in both the declared and validated result
blocks, so a zero there is a fact about the run, not a limitation of the
validator.

## A map author who reuses modules has published an answer key

One map here is a single 40-block module placed four times, with exactly one
human record — 8.7× the author time, mostly retries — and no way to tell whether
the author time's implied pace was achievable at all.

Its author has 486 maps on TMX and **reuses the module byte-identically**. A
sibling map turned out to share **167 of 186 block records** — same block, same
absolute position, same angles — with its four checkpoint gates at the *same
world coordinates*. And on that sibling, a human holds a time **under its author
time**, in a clean single-life run, driving our obstacle.

That ghost answered every question the map had been stuck on: it clears the
critical gap four times out of four at 300–323 km/h; its checkpoint crossings are
65–69 m/s where our record's decay 53 → 36; and its cycle times bracket the pace
the author time needs. **The pace is a thing a human does, repeatedly, on this
geometry.**

It also produced the diagnosis. Our field's launch fails on **sideways velocity,
not speed** — all three measurable launches hit the kicker at 91–99 m/s, and what
separates the two that work from the one that does not is vz (−17.9 and −25.1
versus −3.2). A previously published claim that the map's extra boost pads force
too much speed into the catch was **withdrawn on that evidence**.

The method is cheap and general:

1. author's map list from the TMX API, paginated;
2. download at ~1 request / 1.5 s with an honest User-Agent;
3. **fingerprint by block census** — the count of each block model per file;
4. confirm by sorting `name,x,y,z,pitch,yaw,roll` and diffing: identity of block
   *records*, not just of counts.

> **Before concluding an obstacle cannot be driven a certain way, look for a
> sibling map where somebody drives it well.** A clean single-life run on
> identical geometry is worth more than any amount of search on a map whose only
> record is a retry grind.

Two cautions from doing it. A relocated finish used as a probe is **a doorway,
not a sphere** — 32 m wide and thin along its normal — so its yaw must be the
travel direction or it is edge-on and never fires; and a probe placed *before* a
kept checkpoint voids the run instead of timing it.

## Measure a map's Lyapunov time before choosing a method

On one map, changing **one steer unit on one 10 ms tick**:

| gate | reference | +1 unit at 2.0 s |
|---|---|---|
| 1.9 s | 1.916 | 1.916 |
| 2.9 s | 2.927 | **2.927 — exact** |
| 8.0 s | 7.973 | 8.037 |
| 9.6 s | 9.634 | **15.716 — the run is gone** |

Errors e-fold every 0.6–0.8 s. Everything else about that map follows from this
one number: a 40-second spread across 15 records, **0 of 319 input events
deletable** at a 40 ms budget over 83 319 evaluations, and the fact that ten of
its fifteen ghosts fail to re-simulate.

It costs five perturbed candidates and one gate ladder, and it tells you up front
whether splicing, event thinning and cross-run transplant are available to you at
all. On a map like this they are not, and any plan built on them is dead before
it starts.

## A build-correlated reproduction failure is not a broken oracle

On the same map, 10 of 15 leaderboard ghosts fail to re-simulate — a figure that
elsewhere in this project was grounds for abandoning a map.

**All ten are from one 2022 build.** All **5 of 5** from three different
2025–2026 builds reproduce exactly, including a 101.259 run, and the state
locator tracks a ghost's own telemetry to rms 0.008 m over 68 s.

On a map with a sub-second Lyapunov time, any physics-build difference is fatal
to a replay. That is a property of the map, not evidence against the instrument —
and it is a completely different animal from the failure mode that *does* condemn
a map, which is ghosts returning **wrong times** rather than not finishing.
Zero wrong-time divergences with a cluster of DNFs is the healthy pattern.

> **Check the `git=` build string on the failing ghosts before condemning a
> map.**

## Our tape's fragility is not the map's fragility — measure the human's too

Perturbation tolerance is normally reported as one number per tape: shift every
input-change boundary by one tick, re-simulate, count survivors. That number is
close to useless on its own, in two separate ways.

**It has to be read per region.** On one map the whole-run figure is **94.2 %**
(1 261 of 1 338 shifts survive), which reads as a comfortably forgiving run. By
window:

| window | shifts | survive |
|---|---|---|
| race 0.00-2.96 s | 52 | **0 %** |
| race 2.96-3.96 s | 30 | 30 % |
| race 3.96-4.96 s | 54 | 93 % |
| after race 4.96 s | 1 202 | **100 %** |

The same shape on all four tapes of that map, and inside the opening window
**0 of 1 300 two-boundary pairs** survive either. So the honest description is
"precision-bound in one two-second window and free everywhere else" — which is
a thing you can hand a player, where "94 % tolerant" is not.

**And it is a fact about the tape, not about the map.** Run the identical
instrument on the human record's own winning attempt and **17 of 42 boundary
shifts survive — 40.5 %** (three of them faster). A launch program with real
one-tick tolerance exists on that map. Every tape our search produced has none,
because a search minimising finish time has no reason to keep any.

Tolerance is also partly purchasable, and the curve can be measured. Coarsening
the launch to a keyboard alphabet gives a third point:

| tape | time | launch tolerance |
|---|---|---|
| our fastest | 15.2 | 0 % |
| coarse keyboard launch | 16.3 | 10 % |
| the human's attempt | 18.8 | 40.5 % |

> **The fast program and the forgiving program are different programs, and a
> driven author time usually sits between them.** Publishing only the fast one
> and calling the map "frame-perfect" describes our search, not the map.

The follow-up that this implies, and which is still open: seed a tolerance
search from **the human's** inputs rather than from any tape of ours, and score
the perturbation a player actually makes — a correlated timing error across the
whole launch — rather than one boundary at a time. Scoring pass/fail directly
does not work: everything scores 0 and there is no gradient. Grade the failures
(finished / reached the pad / cleared the chute / died) and evaluate every shift
deterministically, or a hill climber freezes on the noise.

## A constraint can find time rather than cost it

The standing rule in this project is that converting a finished analog tape to a
restricted alphabet does not work — five maps and counting — and that low-input
runs must be *searched for* under the constraint. True, and there is a second
half to it that is easy to miss.

On one map the keyboard-constrained search did not merely produce a drivable
tape at some cost. It produced **the fastest tape on the map**:

| keyboard steering from | result |
|---|---|
| race 13.56 s | 15.224 (free) |
| race 6.56 s | 15.220 |
| **race 4.56 s** | **15.217 — best of the session** |
| race 3.56 s | 15.292 |
| race 2.56 s | 15.285 (70 input events in the whole run) |
| race 1.56 s | 16.276 |

against a pure-analog champion of 15.224. A restricted alphabet is a smaller
search space with a coarser neighbourhood, and on a map whose payoff is
dominated by a few large decisions that is an advantage, not a handicap.

Two mechanics that made it work, both reusable:

**Grow the constrained window backward from the finish.** The fragile end of a
run is usually the start, so extending the constraint from the finish backwards
keeps the incumbent a *finisher* at every rung. A ladder that starts at the
fragile end has a DNF seed and therefore no gradient at all.

**Apply the alphabet only inside the window.** The naive quantiser projects the
whole steer array, which destroys the prefix the search was told not to touch —
and then the identity control dies for a reason that has nothing to do with the
constraint under test. Two separate tools in this project had that bug.

And the resourcing note: **a rung that reports "no finisher" is not a negative
until it is resourced.** The 1.56 s rung above returned nothing at 2 minutes on
60 workers and produced a finisher at 8 minutes on 90.

## An instrument that reuses a worker directory silently validates the wrong map

Worth recording as a specimen, because the symptom was a confident, plausible,
completely wrong physical conclusion — and because the failure lives in the
harness, not in the physics.

A tolerance sweep validated one tape against several relocated-gate maps under
one root, naming its oracle workers `w{:03}`. The worker's setup creates the
`UserData/Maps` symlink **only if it is missing**. So every map after the first
was silently validated **against the first map**.

What it produced: "0 of 52 one-tick-shifted variants even get down the start
chute" — a clean, dramatic result that was published in two write-ups. The
corrected reading is **52 of 52 clear the chute**, and they crash later, on the
booster ramp. Same data, opposite localisation, and the corrected version made
the human story better rather than worse.

> **If a sweep reuses a working directory across configurations, put the
> configuration in the directory name.** And when a sweep returns a suspiciously
> uniform answer across configurations that ought to differ, check that the
> configurations reached the oracle at all.

## Closing the hard half can turn a map into an ordinary search problem

A map can be worth reopening precisely *because* a long investigation ended in a
negative — if the negative is sharp enough to redirect the effort.

On one map, three agents spent a night on a finish trigger that no upright car
can fire. The endgame is now closed by measurement rather than by exhaustion:
every route to the trigger has a price, the cheapest is 5.5× the time budget, and
the "bank" that looked like a tiltable surface turned out to be the face of the
wall that stops you (274 -> 76 km/h in one 50 ms tick — the tilt and the speed
loss are the same event).

That sounds like the end of the map. It is the opposite, because one of those
priced routes is **a human-demonstrated, fully validated finish** — rank 1's own
flip, costing 11.2 s — which needs no new physics at all if the approach can be
driven 9.1 s faster.

And the approach had never been searched, because of a sentence in an earlier
write-up: *"the first 35 s is within a metre of the world record's line — the
world record drives that part essentially optimally."* The first clause is a
measurement. **The second does not follow from it.** Matching a human's line is
evidence about the line, not about whether the line is fast.

Measured as *lead along the route* — for each sample, the nearest point of the
human's whole path and the human's time there, which is immune to the two runs
being in different places at the same instant:

| phase | how close to the human's line | lead gained |
|---|---|---|
| the highway | 0-11 m | +0.36 s in fourteen seconds |
| the loop | 55-96 m — a different line | **+3.4 s** |
| **the long westbound run** | **0.6-1.9 m** | **flat — nothing, over fourteen seconds** |

That third phase is 1075 m at an average of 276 km/h on a car that reaches
639 km/h, with 43 % of samples below half peak speed. It is acceleration-limited,
so the lever is the speed it is *entered* with, and a crude bound puts ~6 s in
it.

> **When a map's hard half closes, re-price the whole run before abandoning it.**
> The budget is `author_time − arrival`, so every second saved upstream is a
> second of slack downstream — and the part of a run that matches a human's line
> is the part most likely never to have been searched.
