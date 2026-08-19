# finish is on the roof to your right — the endgame is closed, and the map is now an ordinary search problem

**Author time 43.079 · human world record 61.229 · best validated run 50.229 —
11.000 s faster than any human, 7.150 s short of the author time.**

> **Status: the hard half is provably closed and the easy half is untouched.**
> The finish trigger is now closed by **arithmetic** rather than by exhaustion —
> one inequality, one calibrated constant, and a budget that fails term by term
> — with the single assumption that would reopen it named below. Meanwhile
> rank 1's inverted flip is a human-demonstrated, validated way to finish, it
> costs 11.2 s, and the approach it needs has **never been searched**.
> Measurement says ~6 s is sitting unclaimed in one fourteen-second stretch. Two
> arms are on it now. Jump to
> [the endgame closure](#the-endgame-is-now-closed-by-measurement-not-by-exhaustion),
> [the trigger equation](#closed-by-arithmetic--the-triggers-own-equation-calibrated)
> and [what replaced it](#which-turns-this-into-an-ordinary-search-problem).

| tape | validated | vs human WR | vs AT |
|---|---|---|---|
| [`TAS_50229`](replays/TAS_50229.Ghost.Gbx) | **50.229** | **−11.000** | +7.150 |
| [`TRIGGERPOKE_50469`](replays/TRIGGERPOKE_50469.Ghost.Gbx) | 50.469 | −10.760 | +7.390 |
| [`POKE_1input_50659`](replays/POKE_1input_50659.Ghost.Gbx) | 50.659 | −10.570 | +7.580 |
| author time | 43.079 | — | — |
| human WR *(control)* | 61.229 | — | +18.150 |

TMX map [285885](https://trackmania.exchange/maps/285885) · **3 recorded runs**,
all three re-simulate exactly · field reproduction **3/3**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

This map is published as a **negative** — but a precise one. The 18-second gap
turns out not to be what it looks like, and the thing blocking a sub-author-time
run is measured to the millimetre.

---

## The map's joke, and it is a good one

**The finish gate is an item sunk about a metre below the roof the car sits on.**
A car resting on the roof is above the trigger, and driving over the finish does
nothing at all.

*(The trigger's exact condition is **not** the obvious one. For most of this
map's investigation it was modelled as "the car's origin drops to or below
`gate_y + 1.25 m`", which fits every measurement made on an inverted car and is
wrong. The real condition is a point fixed in the car's **body** — see
"the tested point is on the roof of the car", below. Nothing in this section
depends on which model you use.)*

**All three humans drive over it repeatedly without finishing.** The world
record's lowest point inside the gate's horizontal footprint is

```
y = 145.2506      against a ceiling of      y = 145.2500
```

**It misses by under five millimetres.** Rank 2 misses by ~15 mm and rank 3 by
~20 mm. That near-miss costs rank 1 **10.6 s** and rank 2 **37.2 s** of driving
past, turning round and coming back.

Proof by map surgery — raise the gate 1 m and all three finish about ten seconds
earlier, with the control at the original position reproducing the untouched map
exactly:

| gate y | rank 1 | rank 2 | rank 3 |
|---|---|---|---|
| 144.000 (true) | 61.229 | 88.209 | 97.769 |
| **144.005** | **51.059** | — | — |
| 144.150+ | 50.639 | 51.009 | 56.309 |

**Five millimetres of gate height is worth 10.17 seconds.**

## So the real gap is ~7.4 s, not 18.2 s

And the 18.150 s figure was misleading twice over: the "human world record" is
the author's own alt account, 18.150 s slower than the author time they
themselves set.

## What a human should take from this

**One steering input at the right moment is worth 10.5 seconds.** Applied to the
world record's own tape, a single change at race ~50.9 s takes 61.229 → 50.659.

And it is **extremely forgiving**. Of twenty-three held values tried at that
instant, **sixteen fire the gate ten seconds early — including neutral, i.e.
doing nothing at all.** Only hard left (≤ −30) loses the run. The timing window
is ~550 ms and hold durations from 0.08 to 0.5 s all work.

So the instruction is not a frame-perfect input. It is:

> **Arrive, and stop turning right.**

One of the few values that does *not* fire the gate is exactly what the world
record does.

*(Precision note: the published poke tape is a rectangular override of the
steering channel over 25 ticks, not a single key press. The durable claim is the
tolerance result above, not that tape's literal contents.)*

## The tested point is on the roof of the car, not at its origin

*(This supersedes the earlier reading of this page. The "70 mm of clearance" was
real as a measurement and wrong as an explanation.)*

**The trigger does not test the car's origin. It tests a point fixed in the
car's body, about 0.84 m above the origin — roughly the roof.**

The control is model-free: one ghost, one gate, two crossings of the same
footprint 20 mm apart in gate height.

| crossing | gate y | fires at | **origin y at the fire** | attitude |
|---|---|---|---|---|
| rank 2 | 144.34 | 68.608 | **145.521** | upside down |
| rank 2 | 144.36 | 51.019 | **143.937** | upright |

**The crossing that fires with its origin 1.585 m HIGHER is the inverted one.**
No function of the origin can produce that. The `car_y ≤ gate_y + 1.25` model
held for hours because it was fitted entirely on inverted tapes.

Turning the car over moves the tested point **1.7 m down**. So the achievable
envelope, measured rather than assumed:

| term | contribution |
|---|---|
| **attitude** | **±1.7 m** — by far the largest, and nobody had it |
| suspension | **≤ 5 mm** — no grounded sample in any human run is more than 2/255 below the resting damper value |
| steering | **< 1° of body roll** — full lock for 0.8 s at 190 km/h leaves the car *flatter* than unperturbed |
| going airborne | **impossible here** — the ramp's radius is ~1500 m, so leaving it needs 440 km/h; the route crosses at 190 |

The roof under the finish is one clean plane (11.4°, rms 38 mm over 263 human
samples), so on that plane the car's attitude *is* the plane's attitude. There is
no other term. That is why 1.6 M evaluations of steering, pedal and pulse
perturbations found no gradient: **on the current route, the fast tape is already
at the optimum for a car sitting flat on that plane.** All three humans finish
upside down, and on this map that is a necessity rather than a fumble.

What the map actually asks for is **~26° of body tilt inside the footprint** —
0.26 s of the 1.7 rad/s tumble that leaving any ramp edge produces. The open
question is a **rotation source within about 2 s of the finish**, which is a
different and much better-posed problem than the one this page described before.

## Correction: the 70 mm is not locked. It is a cost curve.

*Added after a later session; this supersedes the "the crossing geometry is
locked" reading above, which was written when the only evidence was a 10 mm
rung.*

Two agents established 144.070 as immovable across ~57 000 evaluations. **They
were measuring with a 10 mm rung — and 10 mm is five to twenty-five rungs of the
gradient that actually exists.** A search of the same class, scored on a **1 mm**
height ladder, moved it:

| clearance | gate y | evaluations to reach it | fire time |
|---|---|---|---|
| the two agents' wall | 144.070 | 0 finishers in ~57 000 | 41.074 |
| ladder rung 1 | **144.068** | 384 | 41.084 |
| ladder rung 2 | **144.067** | 2 304 | 41.059 |
| ladder rung 3 | **144.066** | 5 376 | 41.059 |
| ladder rung 4 | **144.064** | 12 672 | **41.069** |
| — | 144.063 | not reached in 35 712 | — |

Six millimetres, and **the depth was free in time** — the deepest tape fires
*earlier* than the incumbent. The instrument carried nine controls, including a
dead-window control (896/896 evaluations return the seed's exact score over a
window before the race starts, zero "new best" events) and an independent
re-measurement of the deepest tape by the other agent's separate binary.

**The honest extrapolation is bad news and should be read as such.** Cost per
millimetre roughly doubles: 384 → 1 920 → 3 072 → 7 296 evaluations for
successive millimetres. At that rate the next 6 mm costs 10⁵–10⁶ evaluations and
64 mm is out of reach *by this route*.

The value of the result is that it converts a wall into a measured cost curve. It
removes the reason to keep grinding local mutations at 10 mm granularity, and it
says what a winning lever has to look like: **something that buys tens of
millimetres at once** — which is exactly the shape of the tilt attack above. This
ladder is the instrument that will grade it, at 1 mm.

**Best validated time on the untouched map is unchanged at 50.229.** The fast
route still DNFs, as it must at 64 mm of remaining clearance.

> **The general lesson, which is not about this map: a negative from a rung the
> population cannot reach in one mutation says nothing about the rungs in
> between. Suspect the enumeration before the hypothesis — including the
> enumeration hidden in your rung spacing.**

## The six probes that found nothing, and why

A search seeded from the world record found a genuinely faster route — full
throttle up the roofs at 150–190 km/h where the field lifts to 30–100 —
**reaching the finish patch at ~41.04 s, 9.6 s earlier than any human.** The
first 35 s is the world record's line within a metre; everything after the
landing is new. **That would be ~2 s under the author time.**

It misses the trigger by **70 mm of height** — established by 1 mm bisection
(clears at gate y 144.070, refuses at 144.069) — or equivalently **0.42 m along
the (+x, −z) diagonal**.

What has been thrown at that 70 mm:

- ~50,000 designed tapes: steer and pedal overrides, pulses, neutral holds,
  single-tick nudges across ticks 3000–4170
- ~1.5 M shaped search evaluations
- a **height ladder**: a single 10 mm step below the clearance, seeded from the
  fast route, 168 workers — **50,430 evaluations, zero finishers**
- a five-rung **lateral ladder** that stalls exactly on the measured footprint
  boundary

The crossing geometry of this route is **expensive rather than locked** — see the
1 mm ladder correction above — and every one of these probes was an attempt to
arrive *lower*, which is the wrong currency. What the trigger wants is tilt.

## The endgame is now closed by measurement, not by exhaustion

*(This section supersedes "The one idea nobody has run" and the speculative parts
of the section below it. Both are kept because the reasoning is still worth
reading, but the questions they pose have since been answered — and the answers
are all negative.)*

Every route through this trigger now has a **price in seconds**, against a budget
of `43.079 − arrival` = **2.042 s** at the current arrival:

| what the trigger needs | why it is unavailable |
|---|---|
| arrive lower | the deficit is **144.092 ± 0.024 for every fast line whatever it does** — 7 tapes, 3 families, 0.913 m of spread, slope +0.003 m/m |
| tilt ≥ 26° on the approach | the car takes the surface normal; full lock *flattens* it; suspension gives ≤ 5 mm |
| leave the surface and rotate | the ramp radius is ~1500 m, so leaving it needs 440 km/h against 190 — and a launched car is 1.23 m *under* the rising roof after 0.5 s |
| clip an edge or a step | the roof is flat to **0.026 m/m** over x ∈ [405, 428] on three lines, and the corridor above the finish is the **same plane**, extrapolated and confirmed (mean −0.123 m, sd 0.160, n = 12) |
| flip early and drive on | **11.2 s** — rank 1's own route; inverted, the car crosses at 20–45 km/h against 190 |
| flip on the wall | **19.4 s**, and the wall blocks the only usable approach |

The last row is the one that closed it. The 28.3° "bank" that looked like a
tiltable surface is **the face of the wall that stops you**:

| t | position | speed |
|---|---|---|
| 60.500 | (404.20, 148.095, 1666.50) | **273.7 km/h** |
| **60.550** | (405.14, 148.432, 1667.40) | **76.0 km/h** |
| 60.600 | (405.85, 149.030, 1667.56) | 60.8 km/h |

**274 → 76 km/h in one 50 ms tick.** The 28.3° reading is the car's attitude
*during* the impact, not the slope of anything it was driving on. It does flip
the car fast — 3.8 rad/s, twice the ramp-edge rate — but the tilt and the speed
loss are the same event, and the wall sits square across the mouth of the one
corridor whose geometry would otherwise work.

> **285885 is characterised, not merely unbeaten.** The finish tests a point
> 0.84 m above the origin in the car's body frame. Every upright crossing of the
> footprint, by any line, misses by 70–140 mm. Closing it needs ~26° of body
> tilt, the map contains exactly three sources of tilt, and the cheapest costs
> 5.5× the entire time budget.

## Closed by arithmetic — the trigger's own equation, calibrated

The strongest form of this result is not "we could not find it". It is that the
finish condition has been reduced to one inequality with one constant, and the
budget fails term by term. The trigger fires iff

```
q = (y − plane(x, z)) + 0.84·u_y  ≤  C          one constant — no x, no z
```

`C` was calibrated on a **1 mm ladder**, on its own build and server tree, with a
return-to-origin control passing on 7 ghosts and every bracket re-run singly:
**C = 142.988 ± 0.003**. So the real gate, at 144.000, fires iff `q ≤ 1.012`.

**The equation has no x and no z in it, and that is the whole explanation for a
night of stalled probes.** The trigger's top face is parallel to the roof, so
*where* you are inside the footprint is inert by construction — two independent
ladders stalled on exactly the measured geometry because they were walking along
a direction the trigger cannot see. A probe that moves only in x and z is not a
weak probe here; it is a probe of nothing.

With the equation in hand the budget closes term by term:

| term | full observed range | can it pay? |
|---|---|---|
| `gap` — body height above the roof | **31.0 mm** | no |
| `0.84·u_y` — attitude | **5.3 mm** | no |
| **required** | **71.6 mm** | — |

* **`gap` cannot pay.** Body compression was measured across the whole map in 13
  speed bins: **3–8 mm mean, and flat in speed.** A car at 627 km/h compresses
  its body the same as one at 120 km/h, so **there is no downforce lever** — the
  intuition that going faster presses the car down is measured false here.
* **Attitude cannot pay.** The same equation asks for **tilt ≥ 26.6°**, which
  independently re-derives an earlier arm's 26° from a different direction, and
  the cheapest source of that tilt on this map costs **11.2 s against a 2.042 s
  budget**.

**Two instruments with nothing in common agree on the deficit to 1.6 mm** — 70.0
mm measured directly from crossings, 71.6 mm derived from the decomposition. And
the fine ladder puts the incumbent tape's true threshold at **144.070**, with
every tape silent from 144.000 all the way to 144.069: the gate is not nearly
firing.

### The one assumption whose failure reopens the map

An arithmetic close that hides its assumption is worse than an honest negative,
so this is stated in the arm's own terms:

> The close assumes the tested point is a **single** point on the body's up-axis.
> That model is *known* to break between `u_y` 0.98 and 0.56 — 1.33 m upright,
> 0.125 m at the loop apex. **If the trigger is instead a hull with several
> tested points, a partial 10–15° tilt could be worth far more than `−L·sin θ`
> predicts.**

That is a **trigger-geometry measurement, not a search** — which makes it cheap,
well-posed, and the first thing anyone returning to this map should do.

## …which turns this into an ordinary search problem

Here is the part that matters, and it is optimistic rather than the reverse.

**Rank 1's flip is a human-demonstrated, fully validated way to finish this map.**
It costs 11.2 s and needs nothing new — no undiscovered physics, no new
technique, no gate surgery. It just needs the approach driven in **31.9 s instead
of 41.0**.

And the approach has never been searched. An earlier note on this page said the
first 35 s is "within a metre of the world record's line — the world record
drives that part essentially optimally". **The first clause is a measurement; the
second does not follow from it**, and nobody had tested it. Matching a human's
line is evidence about the line, not about whether the line is fast.

Measured properly — for each sample of the fast route, the nearest point of the
human's whole path and the human's time there, giving the TAS's *lead along the
route*:

| phase | what the TAS is doing | lead gained |
|---|---|---|
| 0–14 s, the highway | on the human's line within 0–11 m | **+0.36 s in fourteen seconds** |
| 14–20 s, the loop | a genuinely different line, 55–96 m off | **+3.4 s** |
| **20–34 s, the westbound run** | **back on the human's line, 0.6–1.9 m away** | **flat at +3.74 → +3.87 — nothing** |

That third row is fourteen seconds and ~1075 m at an average of **276 km/h**, on
a car that reaches 639 km/h on the highway. **43 % of every sample before 36 s is
below half of peak speed.** The section is acceleration-limited rather than
skill-limited, so the lever is not the straight itself but the speed it is
*entered* with — the loop spits the car out at 144 km/h. Carrying 400 km/h in
instead would cover those 1075 m in roughly 8 s rather than 14.

**That is ~6 s available in the one part of the run nobody has searched, next
door to the loop, which is the only place a TAS has ever beaten this human.** The
flip route needs 9.1 s.

So the recommendation on this map has inverted: **reopen it on the approach, not
the finish.** Everything difficult is in the last 40 m and the last 40 m is now
known to be impossible; the time is in the first 35 s, and the first 35 s is
untouched. It is a normal arrival-time search with an objective that already
works, no trigger subtleties, no attitude, and no gate surgery. Two arms are on
it now.

## Where the remaining time is — the earlier station analysis

An arrival-time ladder — 16 stations along the route, with a control that aborts
unless returning the gate to its origin reproduces the untouched map — gives:

| station | rank1 | rank2 | rank3 | our route |
|---|---|---|---|---|
| 0 — landing (343, 106, 1835) | **36.049** | – | – | 36.049 |
| 4 — hairpin apex (301, 123, 1763) | **40.074** | – | – | 40.074 |
| 8 — (343, 132, 1741) | **44.789** | – | 48.488 | 44.789 |
| 13 — (409, 144, 1710) | 49.569 | 50.809 | 55.189 | 49.549 |
| 14 — (415, 145, 1705) | 50.579 | 51.009 | 56.299 | 50.439 |
| 15 — the real gate | 61.229 | 88.209 | 97.769 | 50.509 |

**The hairpin is the entire deficit, and it is larger than the deficit.** The
record lands at station 0 at 36.049, drives *away* from the finish — west and
downhill — to the apex at 40.074, turns round, and does not regain the same x
(25 m higher, station 8) until 44.789. **That U-turn costs 8.74 s. The map needs
7.4 s.**

So this is a **landing problem, not a crawl problem**: land on station 5 at
~36 s instead of 41.709 and it is worth 5.7 s; land on station 8 and it is worth
8.7 s — the whole map — without touching the rooftop crawl at all.

**And rank 2 is doing something nobody has looked at.** It never enters stations
0–10, reaches station 13 at 50.809 and station 14 at 51.009 — within 0.4 s of the
world record — and only passes stations 11 and 12 at 70.289 / 71.338, twenty
seconds later, coming back down. It reaches the patch by a completely different
approach, almost as fast, while being 27 s slower overall. **If that approach
skips the hairpin, it may already be most of the answer.**

## Two instrument findings from this map that outlived it

**A gate probe is only an instrument if it can return to its origin.** The
standard gate mover swaps the item model before moving it, changing the trigger
volume — on this map it *enlarges* it, so everything fires early including the
null case. Caught because putting the gate back at its original position
returned 50.589 instead of the true 61.229. A broken search fails to find
something; a broken gate probe **fabricates a discovery**.

**A synthesised tape carries its template's telemetry, not its own.** Decoding a
candidate here returned byte-identical data to its seed — reporting the world
record parked at a hairpin 120 m from where the candidate actually was. An agent
read a climb rate of +9.2 m/s off it when the truth was a *descent* of 1.1 m/s,
and nearly redirected the search on that basis. Both findings are written up in
this repo's [`FINDINGS.md`](../FINDINGS.md).

**A units mismatch in a tool flag manufactures a perfect-looking null.**
`m8force --from` takes **ticks, not milliseconds**. Asking it for "30 s" as
`--from 30000` lands past the end of the tape, so the forced edit does nothing
and the run returns the container's own time — a clean, plausible, completely
empty result. It was caught only because the output file's md5 changed while the
answer did not.

**And this map established its failure anchors *before* it produced any
negative**, which is why the negatives above can be read at all. Three
distinguishable signatures, each demonstrated on purpose:

| what happened | what the oracle prints |
|---|---|
| a guaranteed-simulated driving failure | `DNF cps=0` |
| a file the server refused | **no result block at all** |
| a tape belonging to a different map | `DNF cps=-` |

Knowing those three apart is what separates "the car tried and failed" from "the
harness never ran", and it has to be established while you still expect success.

## Validation

Field reproduction 3/3 exact (61.229 / 88.209 / 97.769). Encoder identity
control passes. Every number here came from the plain oracle on the untouched
map with a human ghost as a known-answer control in the same batch. **No
phantoms**; one wrong model published during the work (a "floor" on the trigger)
was retracted within minutes of being measured against.

There is **no author ghost** in this map file — verified on the LZO-decompressed
body with another map's author lap decoding in the same tool as a positive
control.

## Files

| file | what |
|---|---|
| `replays/TAS_50229.Ghost.Gbx` | the fastest validated run — 11.0 s faster than any human |
| `replays/POKE_1input_50659.Ghost.Gbx` | the world record with one steering change: 61.229 → 50.659 |
| `replays/TRIGGERPOKE_50469.Ghost.Gbx` | the same idea, searched |
| `notes/TRIGGER.md` | the sunken gate, the 5 mm human miss, the tolerance sweep |
| `notes/LADDER.md` | the arrival-time ladder and the hairpin finding |
| `notes/CLEARANCE.md` | the 70 mm / 0.42 m clearance and its trade curve |
| `notes/FINDINGS.md` | the route half: search, negatives, telemetry trap |
| `notes/TRIGGER_IS_A_BODY_POINT.md` | the corrected trigger model: the tested point is on the car's roof |
| `notes/TILT_SOURCES.md` | where tilt can and cannot be obtained, measured |
| `notes/HEIGHT_LADDER.md` | the 1 mm ladder that turned the 70 mm wall into a cost curve |
| `notes/ENDGAME_CLOSED.md` | **the closure** — the wall face, the corridor plane, every route priced |
| `notes/NEXT_LEVER_THE_APPROACH.md` | **what to do instead** — the fourteen seconds nobody has searched |
| `notes/INVERTED_ARRIVAL.md` | the flip's value and its 11.2 s cost |
| `notes/DISPLACEMENT_IS_INERT.md` | a lever that was retired by measurement |
