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

## Input tapes are not portable between ghost containers

A `.Ghost.Gbx` holds its per-tick inputs in an archive chunk (`0x0309201D`).
Moving that archive into a *different* ghost file does not work:

| tape | container | result |
|---|---|---|
| rank 2's input archive | rank 2's own ghost file | **977.690 — exact** |
| rank 4's input archive | rank 4's own ghost file | **1371.430 — exact** |
| rank 2's input archive | rank 1's ghost file | **DNF at CP1** |
| rank 4's input archive | rank 1's ghost file | **DNF at CP1** |
| the author's AT archive, from the map file | rank 1's ghost file | **DNF at CP1** |

The two identity rows prove the transplant machinery is correct. Copying the
archive's `start_offset_ms` alignment does not help, and neither does copying
**all fourteen** small `0x03092xxx` chunks from the donor, so the carrier is one
of the large ones.

**Consequence: "best-of-field splice" — composing the best sector from each
human's run — is not available.** Splice within one ghost file only. This is
worth knowing before you spend a day on it: three agents on three different maps
each hit an unexplained DNF-at-CP1 and misdiagnosed it as a respawn-state or
physics problem before this was pinned down.

It is also what currently blocks the biggest known prize on Leto. The author's
own author-time ghost is embedded in the `.Map.Gbx` and decodes fine; with its
nine failed attempts deleted it is worth **220.563 s**, another 15 seconds under
the published run. It will not re-simulate, purely because it is a foreign
container.

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
