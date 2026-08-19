# Torment (1-DOWN) — the ending nobody drives

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **20.237** | **−0.318** | **−2.400** |
| Author time (never beaten by a human) | 20.555 | — | −2.082 |
| Human WR — KappaRiley | 22.637 | +2.082 | — |

TMX map [228811](https://trackmania.exchange/maps/228811) · by Bernkastel_. /
Emelius. · **48 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## Every run on the board ends the same way. The author's does not.

All 48 records arrive at the base of the end wall at about 360 km/h, ride *up*
the wall to y ≈ 142, flip 180°, and fly ~314 m back to the line. The wall climb
costs about 1.6 s and the flight 2.6–5.5 s.

**The author never climbs the wall.** He slides along the floor at its base and
hits something that fires him from **323 to 751 km/h in a single contact**, then
glides to the line upside down. That is the entire 2.082 s gap, in one move.

## The floor everyone already drives on is the launcher

At the base of the end wall the floor from x = 32 to x = 128 is boost platform.
**All 48 runs cross onto it and pick up an ordinary turbo there.** Nobody is
missing a hidden object in a corner of the map — everybody drives the length of
this thing, every lap.

Running through the deck at **z ≈ 709**, spanning at least x = 56 to x = 136,
there is a trigger about a metre wide. Cross it correctly and the game fires the
car along its own nose at 700–950 km/h.

It is **not** fussy about where along the line you hit it: **1,343 separate
launches** were produced, spread over 80 m of x. It is extremely fussy about
*how*.

## The condition: you must arrive SIDEWAYS

> **Cross the line going in −z, at floor level, with the car turned across its
> own direction of travel — at least 85 m/s (≈300 km/h) of your speed pointing
> out of the side window.**

| what you do | fires? |
|---|---|
| cross at 360 km/h pointing where you're going — *what everyone does* | **no** |
| slide **along** the line at 102 m/s | **no** |
| cross nose-first at 100 m/s | **no** |
| cross **downwards, body lateral, ≥85 m/s of side speed** | **yes** |

Position and velocity are not the trigger, and this is worth stating precisely
because it is what makes the move invisible: **a tape that reaches the author's
contact point to within 0.3 m, with velocity within 3 m/s, does not fire.**

## And the obstacle is not the launcher — it is the checkpoint at x = 80

Launches upstream of that gate fly beautifully. There are tapes here that pass
**within 0.8 m of the finish** — and validate as DNF, at 5 of 6 checkpoints.

**The launch has to happen in the ten metres *after* the gate**, which is
exactly where the author's is. That single constraint is why the author's route
looks arbitrary, and it is most of why nobody has reproduced it.

## Somebody is already doing this

**Rank 11 (26.715) crosses x = 80 with 87.6 m/s of body-lateral speed at
331 km/h** — the author's own signature, and by a wide margin the largest on the
board. He is still at 81.7 m/s of side speed at x = 65, sliding down toward the
line, and then he puts the car into the end wall at 12 km/h.

He is not near-missing by accident. **He is executing the move and finishing a
couple of metres of x and a few m/s of side speed short of firing it.**

Every other record is under 20 m/s of side speed near the line, and **0 of 48
satisfy the full condition.** So this is not a feature nobody has come near — it
is a feature one person is visibly attempting, which is simultaneously the best
evidence that a human can do it and the best evidence for why the next section
matters.

## The honest difficulty

**Shifting the whole final input stream by ±10 ms does not degrade the launch —
it removes it entirely.**

One caveat, stated because it cuts in our favour and should not be quietly
banked: that number comes from a **697-event TAS line**. The author's own script
is **37 events**, and may well sit in a more forgiving pocket. It cannot be
measured, because his ghost is a state recording rather than an input tape.

## Three objectives that were satisfiable without launching

Getting a search to find this needed **scoring the state, not the time** — and
it took four attempts, because the first three could all be maximised without
firing anything:

| objective | what it did instead |
|---|---|
| −vz alone | ran to the box corner |
| side speed alone | slid along the line |
| progress along the author's line | plateaued at 86.9%, launching at the sky |

Also useless: **peak speed as a launch detector** — the human world record
itself hits 151 m/s. And **a near miss can outscore an arrival**, which inverts
the ranking exactly where it matters.

> **An objective that can be maximised without achieving the goal is not a
> proxy — it is a decoy.**

## Validation

Eighteen tapes across four plain-oracle batches, each carrying the human world
record as a known-answer control; it returned 22.637 every time, and every tape
reproduced its claimed millisecond. **Zero phantoms.**

One defect found and fixed along the way: **the identity control was testing a
moving incumbent rather than the seed.** Its replacement is stronger than what it
displaced — a fork-mode check that read −35.989 against the ghost's own
telemetry of −36.122, validating layout, position, velocity *and* quaternion in
a single number. Written up separately as a reusable control.

## Files

| file | what |
|---|---|
| `replays/TAS_20237.Ghost.Gbx` | the run |
| `notes/TECHNIQUE.md` | **the driver's guide** — the line, the condition, the gate, the tolerance |
| `notes/RESULT.md` | the full write-up, including the three failed objectives |
