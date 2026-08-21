# untitled 01

**Author time 23.839, and nobody has ever set a time on this map. This run does
it in 12.759 — and it drives the map: supported for 80.7 % of its samples,
never rising above its own spawn altitude, using the map's own lift to
reach a finish that sits at the bottom of the lattice.**

| run | time | vs author time |
|---|---|---|
| **TAS** | **12.759** | **−11.080 (−46.5 %)** |
| TAS, first finish | 13.349 | −10.490 |
| Author time | 23.839 | — |
| Human record | *none — nobody has a time on this map* | — |

TMX map [276874](https://trackmania.exchange/mapshow/276874) · author
**DugonGOD** · **0 recorded runs** · map uid `9wv8HirGqNFCJsFeVJg6ErKYH6b`.

**untitled 01** — TAS **12.759** (−11.080) | AT 23.839 | no human has ever recorded a time here

With the inputs overlaid:

https://github.com/user-attachments/assets/d7912e54-4a9a-49be-b3f8-355a9f26eed1

## What kind of result this is

Same author and upload day as [untitled 02](../276877-untitled-02), and the same
lattice — **0 checkpoints, 2 finish gates, nothing enforcing a route.**

But **this run drives the map.** The test is model-free: a car with nothing under
it accelerates downward at 25.20 m/s² in this engine (calibrated on two tapes that
provably fall out of the world). Across the stretch that looks like a glide, the
car drops **18.4 m in 2.71 s** where free fall would drop 92.5 m — it is supported
the whole way.

Measured per sample:

| | supported | airborne |
|---|---|---|
| inside an occupied block cell | 58.1 % | 14.8 % |
| outside every cell | 22.5 % | 4.5 % |

**80.7 % of the run is supported**, total airborne time is 2.43 s with the
longest continuous stretch 1.13 s — and that one is a real 20 m drop off the
spawn platform, not a flight. The run **never rises above its own spawn
altitude**.

Of the 3.40 s spent outside any occupied cell, only **0.57 s** is airborne; the
rest is a car on a surface that simply is not inside a block's cell. That
distinction flips on centimetres — 7.5 cm lower and the same car changes
category — so treat "inside a cell" as bookkeeping, not physics.

And the departure from the built volume is *the map's own doing*. The finish
sits at **y 292**, and the only thing that lifts a car back up is the lift
column — which exists at **cell-x 18 and 28 only**. A car that drops to the
lattice floor between them can never get high enough to finish. So the run's
ugly-looking stall at 12 km/h is not waste: it is the only way to be high
enough, and it caps the glide's entry speed at 109 km/h.

**A note on what that column is.** Earlier versions of this page called it a
*reactor* column. That is wrong: a census of the map's own block list finds
**zero blocks whose name contains "Reactor"**. What the map actually carries is
**9 `GateExpandableSpecialBoost` gates and 9 `RoadBumpSpecialTurboRoulette`
bumps**, plus 18 reset gates and 18 no-steering gates. The lift itself is real
and measured — the car climbs **19.2 m between 4.25 and 7.50 s** while
accelerating 11.8 → 182 km/h, with no block beneath it for the first 1.5 s and
vertical acceleration nowhere near the −25.20 m/s² of free fall, so it is
neither supported by geometry nor ballistic. Three boost gates sit 6.6–12.1 m
away during that climb, and the run passes within 0.7 m of a turbo bump at
0.350 s. That is a coincidence in space and time; **the mechanism is not
demonstrated** and this page does not claim one.

## The line

Spawn at (976, 341, 714). **181 km/h by 1.25 s.** A hard contact at 2.5 s knocks
it down to about 50 km/h. The **cx-28 lift column raises it 18 m between 4.3
and 6.3 s**, which is what pays for the glide that follows. Then down through
the lattice, **228 → 254 km/h from 8.75 to 11.25 s**, arriving at the Goal at
(580, 292, 713) doing **263 km/h** at 12.25 s and stopping dead into it.

Altitude falls smoothly from 341 m to 285 m across the whole run.

## The run, as inputs

**1,525 input change events over 247 distinct steering values** — a dense analog
tape. Throttle is held almost throughout: only **76 ticks of lift** in the whole
run, and **144 ticks of brake**, most of them tapped while the gas is still
down.

```
race 0.000–0.310  straight        | off the line
race 0.310–0.660  full RIGHT      | −127, with one brake tap at 0.400
race 0.860–1.470  full RIGHT      | held, and the brake TAPPED 14 times
                                  |   through it — gas never lifted
race 1.470–2.090  straight        | 620 ms of nothing
race 2.240–2.450  GAS OFF         | the only real lift in the run, 210 ms
race 2.770–2.820  full LEFT stab  | +127 for 50 ms
race 2.820–4.060  straight        | 1.24 s of nothing — the lift column
race 4.060 onward the lattice     | continuous analog work to the Goal
```

The two long straights and the single throttle lift are the parts a person could
copy. The rest is not.

## Can a human do this?

Not this tape — it is analog, dense, and 1,525 events long. **13.349** is the
more human-shaped number: the first finish anybody has recorded on this map, and
still 10.490 under the author time.

## How it was found

No online records, no author ghost, no embedded telemetry — no reference line of
any kind. The search climbed toward the finish gate for hours, stuck at one
point **0.375 m short in y for 85 rounds and 25,500 candidates**.

What broke it was not more search on that axis. Re-sweeping the *other* axes at
the stuck rung showed the run could already reach y 292 — it was 4 m short in
**x**. **A one-axis ladder reports "no fire" identically whether that axis is
exhausted or is simply the wrong axis**, and one 24-validation sweep tells the
two apart.

After the first finish at 13.349, the search improved twelve times in 94 seconds
and then found nothing in a further 2.3 million evaluations at a 74 % finish
rate. Two fresh basins were tried; both are blocked by the reactor-column
geometry above.

## Verification

Nadeo's own dedicated server, on the map file trackmania.exchange serves:

```
"ValidatedResult" : { "NbCheckpoints" : 1, "NbRespawns" : 0, "Time" : 12759 }
"DeclaredResult"  : { "NbCheckpoints" : 1, "NbRespawns" : 0, "Time" : 12759 }
"IsValid" : true
```

The ghost carries its own regenerated telemetry — read out of engine memory
while replaying the tape's inputs — so it plays back as itself. 26 of 33
independent regenerations agree to within 1 mm, and a cross-check against a
completely different readout path agrees to **0.48 mm**.

## Files

| file | what |
|---|---|
| `replays/TAS_12759.Ghost.Gbx` | the run |
| `inputs/TAS_12759.inputs.csv` | per-tick inputs |
