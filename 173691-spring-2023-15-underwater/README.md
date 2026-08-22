# Spring 2023 - 15 (Underwater) — the jump lands

**GothMommyTM's underwater jump was measured at 4.1 m short of the stadium's
lower canopy. It isn't. A search over his own flight inputs closes the gap: the
car crosses the deck plane at z = 448.2 and comes to rest on the deck at
y = 114.0, stationary, 0.6 km/h.**

TMX map [173691](https://trackmania.exchange/maps/173691) · 3 checkpoints ·
author time **2672.290**. **The map is not beaten and this does not beat it** —
see "the platform is sealed" below.

**Underwater jump — the car lands on the second platform (36.049)**

https://github.com/user-attachments/assets/26276c41-85ba-4bc2-a6db-9b3836edbafa

Two cars, one scene, camera on ours: the TAS car and GothMommyTM's own
demonstration run, which leaves the same lip at the same speed and sinks past
the deck into the water.

## What the jump is

The route is GothMommyTM's and the credit for it is his. The map is driven
underwater; at race **25.80** the car leaves the end of a banked curve at
(1311.8, 138.2, 386.4) doing 101 km/h and glides. Water drag takes the speed
within about four seconds and after that the car only sinks, at a terminal
2.7 m/s, to the stadium floor at y = 79.

Ahead of that lip, the first solid thing is the stadium's **lower canopy**: a
flat deck at **y = 114.16**, whose solid cells begin at **z = 448, x ≥ 1312**
(`CanopyCenterFlatBase`). The ring of `CanopyCenterFlatHFC` one cell nearer is
**not solid** — his flight passes straight through it.

So the whole jump is one number: **the z the car has reached at the moment it
falls through deck height.** His demonstration reaches **444.41** and the
landing threshold is **448.5 ± 0.4**.

## What changed

| | z at deck height | outcome |
|---|---|---|
| GothMommyTM's demonstration | 444.41 | sinks to the floor |
| 126 one-move perturbations of it (earlier pass) | 442.9 best | all worse |
| **this run** | **448.2** | **lands, and stops** |

The earlier pass searched single moves around his keyboard line — one-tick
boundary shifts, curve-exit steering biases, in-flight wiggles — and found his
line to be a local optimum under every one of them. What closes the 4.1 m is
not a cleverer move but **rewriting the whole input stream from the lip
onward**: sixteen hill-climbers, ~40k evaluations, scored first on how far the
crossing point misses the deck footprint and then on **contact time** — how long
the car goes without descending. Contact time is what makes the last two metres
a hill to climb: the first candidates to touch the deck landed on its very lip
and slid back off after 0.6 s, which a yes/no landing test scores exactly like a
clean miss.

The exit is untouched. Every input up to the last ground contact at tick 2575 is
GothMommyTM's own, so the car leaves the lip in **his** state, bit for bit; only
the flight is ours.

Touchdown, from the re-simulated file:

```
 t=36.10   y 114.39   vy -2.67     still falling
 t=36.20   y 114.13   vy -2.60
 t=36.30   y 113.95   vy -0.28     <- contact
 t=36.40   y 114.01   vy +0.25
 t=36.60   y 114.06   vy -0.13     at rest, 0.6 km/h
```

## The platform is sealed — this is a landing, not a win

The map's finish is on the **upper** deck at y ≈ 163–169. From the lower canopy
there is no way up: an earlier pass fuzzed 2 400 tapes from two spawns on this
deck and got **0 finishes**, against 515 of 2 400 from one storey up, and no
tape that lands at 114.16 ever regains height. Structurally there is nothing
between y = 122 and the stand fronts at y = 162.

Reaching this deck is the end of this route, not the start of a lap.

## Reading the clip honestly

* The replay is filmed on **GothMommyTM's own copy of the map** — the one his
  recording embeds. He added a finish gate so the game would let him save a
  replay, and that gate fires at **36.049**, which is why the on-screen timer
  stops there, **0.15 s before the car reaches the deck**. The touchdown is the
  last 0.75 s of the recording, after that gate.
* The canopy and its pillars are **byte-identical** between his copy and the
  untouched map, so the surface the car lands on is the real one.
* The clip was filmed on a named gate exception (`C4,C6,C10`). C4 is the
  post-finish tail described above — cutting at the finish would cut the landing
  out of the clip. C6 and C10 are the ground-contact byte, which reads as
  "another run's" on every ghost from this map: the check assumes a car with the
  contact flag off is in free fall, and underwater it is not.

## Method notes worth keeping

**Distance travelled is not progress.** The first objective maximised how far
the car got from the launch point, and the fleet promptly learned to fly it 166 m
sideways, still sinking, nowhere near the platform. An objective has to name the
place you want, not the amount of movement.

**The block census names the deck but not what is solid.** Two block families
sit at y = 114 one cell apart; one holds a car and one does not, and nothing in
their names says which. The engine settles it in one run.

**A fork-server score is not a result.** Every number above is read off a
written `.Ghost.Gbx` re-simulated by the plain oracle. The search's own best
score has been wrong by 12 m on this map before.
