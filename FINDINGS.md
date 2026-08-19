# What transfers between maps

Things that turned out to be true on more than one map, and are therefore about
the game rather than about one leaderboard. Each one is something a driver can
act on.

## Check which road you are on before you polish the one you are on

The most actionable result in this project cost nothing to find: take every
recording on the geometry, measure one *geometric* property per run — path
length, height, which side of an obstacle — and see whether the field is one
population or two.

On [199100](199100-spring-2023-24-2up), between two checkpoints 241 m apart,
splitting 88 recordings by whether they climb over a tower separates them
cleanly:

| line | n | mean sector time | best | mean path length |
|---|---|---|---|---|
| over the top | 27 | 5.664 | 5.516 | **710 m** |
| **low and short** | **61** | **4.721** | **3.901** | **306 m** |

Every human on that map's own leaderboard was on the 27-run branch, and so was
every tape we produced. **The long line is not badly driven — it is the wrong
road.** It never drops below 350 km/h and averages 430; the winning line
averages 276 km/h, brakes to 150, and beats it by nearly two seconds over 400
fewer metres.

This is the wide-line trap in physical form: **a wider line reads faster and is
slower**, because every glance at the speedo flatters it and nothing charges it
for distance.

## The flashiest difference between runs is usually not the one that orders them

On [249521](249521-impossible-at-for-ssano) every human wags the car's nose
across the strip for 11.4 of the record's 15.0 seconds. Watch the fast runs and
the obvious lesson is *swing harder*. Across all 147 records, peak swing speed
correlates **0.02** with finishing order — nothing at all.

What actually separates them is invisible on video: the car's **attitude at the
moment of the gas lift**. The field lifts at 80–85° of heading, before the nose
is square, and the boost pads give them 0–6 km/h; the fast line lifts at
90–105°, past square, and the same pads give 25–35 km/h.

Two more maps tell the same story: on
[279197](279197-fall-2025-01-reverse-cp1-end) the dramatic closing sweeper costs
every run in the field the same 1.100–1.110, and on
[270051](270051-fall-2025-16-cp1-end) the big closing jump spreads 0.005 across
the field and correlates 0.07 with finishing order, while the quiet stretch at
2.4–3.7 s correlates 0.43.

**Before grinding the scary part, find out whether the field actually loses time
there.**

## Keep the car flat

On [227969](227969-great-wtf-of-what-165), all 42 recorded runs hold full lock
through the final wall-ride. That rolls the car onto its side and pitches the
nose up ~57°, and the kicker then eats a third of their speed (73.6 → 61.3 m/s
for the world record). Staying flat (roll under 5°) and square (sideways speed
0.35 m/s) costs 3 m/s instead: **0.199**.

On [203330](203330-get-in-the-hole-impossible), the finish is decided at the
platform lip: every fast candidate crosses it level, every human run crosses it
heavily rolled, and speed at the lip orders the field perfectly. On
[267859](267859-bald-turtle-35), with fifteen inverted landings, attitude at the
surface orders the top ten as well.

It is not universal — on a map whose runs each lose a chunk somewhere different,
no single per-feature statistic orders anything — but where one section
dominates, "the car should be flat here and the whole field lets it roll" is
something you can act on with no tooling at all, and see in any replay.

A related mechanism on reactor maps: **a firing reactor pushes about 21–22 m/s²
along the car's own "up" axis.** Inverted it cancels gravity, upright it doubles
it, on edge it is horizontal thrust. **Your attitude aims it** — so there,
steering in the air is not steering, it is pointing the thrust.

## The most useful artefact is a human's own run plus two presses

The fastest tape is rarely the useful one. On
[252289](252289-surely-my-least-cooked-at) our best is 3.836 and our keyboard
tape 3.844, but both differ from the world record in nine ticks, and nobody
practises nine changes. What a person can use is: **take the world record's run
and add two keyboard actions** — a one-tick right tap at 2.63 s and a 50 ms
throttle lift at 2.89 s. **3.848 validated, 0.003 under the author time.**

And the pair is irreducible, which is why nobody found it:

| change, on the world record's own tape | time |
|---|---|
| the lift alone | 3.858 |
| **the tap alone** | **4.189 — catastrophic** |
| both | **3.848** |

A field grinding one variable at a time will never find a pair where one half is
a disaster on its own — and on a hunted map, that is exactly what is left after
the route has been ground flat.

## Fragility of a recording is not difficulty for a driver

Shift one input on a fast tape by a single tick and the run usually detonates.
That sounds like "no human could do this", and it is not: **the same test kills
the human world record's own tape** on every map where it has been tried. On
[279197](279197-fall-2025-01-reverse-cp1-end) both quantisation and
sample-and-hold DNF the record holder's run, mid-route.

A replayed tape is open loop; a driver is a closed loop and re-aims every frame.
The honest measurement is *recoverable* tolerance — mistime one input, then
drive the rest — and by that measure these runs are comfortable: on
[252289](252289-surely-my-least-cooked-at), **40 of 40 mistimings recover to
exactly the same time**.

Two refinements worth having:

**Read tolerance per region, not per run.** One map's whole-run figure is 94.2 %,
which sounds forgiving — until you split it: 0 % in the first 2.96 s, 30 % in the
next second, 100 % after 4.96 s. "Precision-bound in one two-second window and
free everywhere else" is something you can hand a player; "94 % tolerant" is not.

**Report the direction, not just the size.** On one map, slipping the steering
10 ms *early* through the decisive window survives and is 0.011 faster, while
10 ms *late* loses the run outright — and the budget is exactly one tick (−2 and
beyond lose it too). A driver cannot use a survival rate. They can use "release a
touch early rather than a touch late."

## A TAS can be the *more* forgiving object

The instinct is that a machine tape is a house of cards and a human's own run is
robust, because a person had to repeat it. Measured, with the record's own tape
put through the identical test:

| map | our tape | the human world record |
|---|---|---|
| [249521](249521-impossible-at-for-ssano) | **41 %** survive | 18 % |
| [267859](267859-bald-turtle-35) | **76.1 %** survive | 24.3 % |

Two things make it possible: a search can afford to score candidates under
perturbation, and a human's record is optimised for *the one time they got it*,
which selects for luck as much as for robustness.

So a run that is both faster and more forgiving than the incumbent is not a
curiosity to watch — it is a better thing to practise.

## Optimise for what you can hold, and the teachable input appears

On [270051](270051-fall-2025-16-cp1-end) a pure speed search found the last
millisecond as a one-tick 75 %-lock stab — an unteachable lottery ticket.
Scoring candidates by their *worst* time over a ±1–2 tick window found the
**same physical effect** as a three-tick, 7 %-of-lock brush with a 30 ms window,
and matched the author time with ±10 ms of slack on every input.

The lottery ticket and the teachable input were the same discovery. Only the
objective decided which one came out.

## Fewer inputs is not automatically easier

Measured on six maps now, input count predicts nothing about how much timing
error a tape forgives — and on
[274191](274191-u10s-32-yeet-max-up) it is exactly backwards: the fastest,
least-constrained tape survives **70.5 %** of one-tick displacements, the
world record's own tape 71.9 %, and the tape with the fewest inputs **17.8 %**.
Stripping a tape to its minimum deletes precisely the inputs that were absorbing
error, so what is left has no slack anywhere. On [249521](249521-impossible-at-for-ssano), thinning the
keyboard tape from 54 events to 30 made it **slower and less forgiving** —
survival fell from 41 % to 10 %, because each remaining input had to do more
work. On [267859](267859-bald-turtle-35) the keyboard version loses on both axes
at once: 0.029 slower than the analog record and half as tolerant. There, the
right thing to hand a person is the record itself.

What you actually pay for is the **alphabet**, not the event count. From the
ladder on [279197](279197-fall-2025-01-reverse-cp1-end): going from 66 keyboard
events down to 35 costs 0.010, while going from 30 distinct steering values down
to 3 costs 0.033. Once the alphabet is fixed, events are close to free.

And a restricted alphabet sometimes *finds* time rather than costing it: on
[165922](165922-idm-ruinin-ur-day-460) the keyboard-constrained run is the
**fastest run on the map**, ahead of the analog one. A smaller vocabulary is a
coarser, better-shaped set of choices on a map whose payoff comes from a few
large decisions.

## On a trial map, the clock runs through your failures

The recorded time is **clean driving plus every failed attempt**, because you
respawn at the last checkpoint and the clock keeps running. So you do not have to
drive better than the record — **you have to fail less.**

On [Angustus](238835-turtle-trial-angustus) the only human record contains 198
respawns and the author's own author-time lap contains 20, of which roughly 160
of its 463 seconds are fourteen failed attempts at one obstacle. On
[Leto](286279-turtle-trial-leto) the first run under the author time was **100 %
the world record holder's own inputs**, in his own order, with ten of his failed
attempts deleted — and the map's own author failed nine times in the same sector.

Three practical facts:

- **One press restores the state you crossed the checkpoint in** — position,
  speed *and attitude*, which on a turtle map hands you back a car doing 62 km/h
  upside down. **Two presses (or the direct respawn key) is a standing respawn**:
  the checkpoint block's own transform, upright and square, at the price of the
  car being frozen for 800–850 ms. Top players use one deliberately at fast
  checkpoints, because braking to a controlled standstill costs more and the
  reset hands you a known attitude.
- **The saved state is live on the very next tick after the checkpoint fires**,
  so you can take the checkpoint however you can and immediately respawn to start
  the hard part clean.
- **Check the checkpoint count first.** On
  [Impossible Mini Trial 2](267460-impossible-mini-trial-2) the only waypoints
  are the spawn and the goal, so a respawn sends you back to the start and is
  never a recovery. "Trial" there is a building style, not a timing mechanic.

## A map author who reuses modules has published an answer key

[YOU LOVE WATER](284238-you-love-water) is one 40-block module placed four
times, with a single human record 8.7× the author time, so there was no way to
tell whether the author time's pace was achievable at all.

Its author has 486 maps on trackmania.exchange and reuses the module
byte-identically — one sibling map shares 167 of 186 block records, same
positions, same angles. **On that sibling, a human holds a time under its author
time, in a clean single-life run, driving the same obstacle.** That one ghost
answered the question: it clears the critical gap four times out of four at
300–323 km/h, and the pace is a thing a human does repeatedly on this geometry.
It also showed the launch fails on **sideways velocity, not speed**, and
retired a published claim that said otherwise.

**Before concluding an obstacle cannot be driven a certain way, look for a
sibling map where somebody drives it well.**

## Some air phases are not live

On [U10S_32 MAX-UP](274191-u10s-32-yeet-max-up) there is a **1.2-second stretch
of the fall with no air control at all** — replace the steering with any
constant anywhere inside it and the game returns the identical millisecond.

It is easy to watch a replay and describe what the fast run is "doing" in the
air. There, the engine was ignoring the input entirely, and the real technique
was one beat earlier, on the ground.
