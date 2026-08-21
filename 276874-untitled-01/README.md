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

> ### ⚠️ Video withdrawn — the tape reports another player's identity
>
> The clip that was here has been taken down. It played, and there is nothing
> wrong with the driving in it: the run is ours, the time re-simulates on the
> game's own oracle to the millisecond in its name, and the declared time in the
> file agrees with what the server validates.
>
> What is wrong is whose file it is. Read by the game's own parser, this tape
> reports account `4c3537f3-381d-46d5-879a-45eca500dd4d`, login
> `TDU38zgdRtWHmkXspQDdTQ` — **a real player, not us.** Our own files report
> login `TAS` and carry no account at all. The same stranger's identity appears
> on this map and on its sibling, so it is a person rather than an artefact.
>
> A searched tape is built inside a *carrier* — an existing ghost — and inherits
> that carrier's container unless every field is rewritten. Identity is one of
> those fields, and on these two tapes it was never rewritten.
>
> **This map has no human recording to compare against** (or, for untitled 02,
> exactly one, set the day this page was written). Every trajectory-based check
> we have is therefore blind here, and the identity read is not one check among
> several — it is the whole verdict. That is why this took until now to find.
>
> The run stands and the time stands. A replacement clip will be filmed from a
> tape rebuilt on a clean carrier.

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

## The route in the video does not convert, and that is now measured

**Both reference clips show the same human failing the same route, 2.5 minutes
apart — the HUD reads `Finishes 0`, on attempts 265 and 278.** The route is: a
turbo bump to 200 km/h, a graze, into the boost column at cell-x 28, **eight
seconds shuffling in reverse gear**, throttle at 9.737, 11 to 305 km/h, through
the reset and no-steering columns, and a crash at cell-x 22 that ejects the car
above the structure. One clip runs at 0.600x, which at 60 fps is exactly one game
tick per frame, so the inputs could be read off frame by frame; the
reconstruction matches the video's own speed readout within 5 km/h across
sixteen consecutive samples.

The author's intended journey — the one both reference videos show — climbs the
lattice, crosses the roof, and comes down onto the finish platform from a great
height. Our 12.759 does not do that. It stays low, never rising above its own
spawn altitude, and uses the map's own lift to reach a finish that sits at the
bottom of the lattice.

**The obvious question is whether the high route is simply better and we had not
found it. It is not, and here is the census.** Two independent searches, 114
rounds each, 120 candidates a round — **27,360 evaluations, about 2,700 of them
genuine finishes on the untouched map** — scored on the untouched map so that
every accepted candidate really crosses the real gate:

> **Not one finish takes more lift than the 12.759 takes.** The best boost-column
> apex across all of them is **330.042 m** — the 12.759's own, to the millimetre.

### Why the height cannot be spent

| at x 676 | y | speed | what happens at the platform |
|---|---|---|---|
| the only known finishing line | **309.4** | **256 km/h** | arrives with vy −11 and **bounces 4.7 m up through the gate** |
| the roof route, i.e. the video's shape | **336.0** | **145 km/h** | arrives with vy −22, lands, slides, drops off the edge |

**27 m too high and 111 km/h too slow at the same place — and the two deficits
are coupled.** The height comes from the long lift, and the long lift is what
costs the speed. The finish trigger wants a fast, level arrival within about a
metre: it fires at x 579, 580 and 581, and DNFs at 578 and 582. So the lift buys
altitude at the price of exactly the speed the trigger needs.

The closest the roof route has come is **0.734 m short** — a tape that fires a
Goal relocated to y 291.266 and nothing at 291.281, against the real gate at
292.000.

**For anyone who takes this map next:** the finish wants a level ±1 m arrival at
roughly 250 km/h, and the only source of height on the map is a lift that costs
exactly that speed. If someone finds a second way up, or a way to spend roof
height horizontally instead of in a plunge, that 0.734 m is sitting there.

*(One method note, because it nearly produced a false result. The first version
of this search ranked candidates by an apex read from live memory, and under load
that reader can settle on the wrong object: it reported apexes of 349.5 m and
368.5 m, which would have answered the question the other way. Both were
phantoms — the winning files were byte-identical to the base and re-measured at
330.042 three times running — and the search then froze for 44 rounds chasing a
score it could never beat. The gate was sound throughout; it was the tiebreak
that lied. **A surrogate measured by a locate is not a number until it
reproduces**, so the leader is now re-measured before adoption. The census above
ran with that guard armed and rejected no phantoms at all.)*
