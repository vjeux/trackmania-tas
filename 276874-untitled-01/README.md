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

With the inputs overlaid — **the strip is centred on the playhead**: three
seconds of input behind `NOW` and three ahead, the future half drawn dimmer, so
an input is on screen before the thing it causes.

https://github.com/user-attachments/assets/3e14059f-3a90-40ed-9cb6-7d78d14a9429

**Re-shot 2026-08-23 from a ghost regenerated out of this page's own tape**
(`replays/TAS_12759.Ghost.Gbx`). The tape was identified by simulation, not by
what a file says: nine of the ghosts held for this map declare 29.286 in their
headers and the plain oracle finishes exactly one of them at 12.759. That file's
input tape reproduces `inputs/TAS_12759.inputs.csv` **byte for byte**
(`ghost tape csv`), which is what ties the clip to the number in the caption —
a second stored file also finishes at 12.759 and its tape differs in the last
40 ticks, all of them after the finish.

`ghost verify` is clean end to end on the file that was filmed: **kappa 1.000**
(the recording in it is this tape's own run, 256 of 256 samples), the plain
oracle re-simulates the WRITTEN file to **12.759**, and its trajectory is
bit-identical — 0.000000 m over 256 samples, metres away one sample either side
— to the file the previous clip was shot from. Nothing per-run in it is the
container donor's: login `TAS`, no account id, our own livery.

**Re-shot again on 2026-08-23 with the carrier bytes actually written, and this
is the version above.** The gather had been reading ~99 of the 116 per-sample
telemetry bytes out of engine memory and reporting them, while the binding that
supplies them was gated on an unrelated flag — so every one of them reached the
file as the donor container's constant, and the clip had no wheel or reactor
effects in it at all. What is live in the filmed file now, by `tmtraj bytes`:
the four wheel rotations (6/8/10/12), side speed (14/15/18), rpm (4/5), the four
suspension travels (23/25/27/29), turbo (31), byte 76 `is_top_contact`, byte 89
`is_ground_contact`, byte 91 gear, and byte 90 `booster_air_control` — the
reactor member the wheel effects read, which is a bit-field packed across bytes
and which no per-byte fit could ever have written. Byte 89 moves through
0 · 1 · 40 · 41 · 48 across the run and holds its lift-column values from
**4.050 to 11.450**, which is exactly the stretch where the effects show.

Per-wheel **ice (81–84) and dirt (93–99) are still absent** — written as zero
and named, not inherited — so those two effects, and only those, are missing
rather than somebody else's.

Byte 32 is worth its own note, because it is why this clip existed twice before:
the game's chase camera reads it, a regeneration that left it at zero filmed the
sibling map with the camera **under the track** for the last second, and the fix
was to write the plain constant 128. It is no longer a constant — the gather
reaches it and it is live, four distinct values across the run. See `GHOSTS.md`,
"The camera reads a byte the gate cannot see".

**What the frames show, old against new**, same instants, both clips pulled back
anonymously (`clip frames`): at **5.000** and **8.000** the old clip's car has
flat black wheels and the new one's hubs are lit orange-white with sparks coming
off them; at **6.000** the whole underside of the car is throwing light. The
camera framing moves a little between the two — the chase camera reads the
visual state and that state is no longer constant — and the run passes through
the lattice into the Goal at speed, so both clips go dark for a tenth of a
second around 12.400, the new one ~0.1 s longer than the old.

## What kind of result this is

Same author and upload day as [untitled 02](../276877-untitled-02), and the same
lattice — **0 checkpoints, 2 finish gates, nothing enforcing a route.**

But **this run drives the map.** The test is model-free: a car with nothing under
it accelerates downward at 25.20 m/s² in this engine (calibrated on two tapes that
provably fall out of the world). Across the stretch that looks like a glide, the
car drops **18.4 m in 2.71 s** where free fall would drop 92.5 m — it is supported
the whole way.

*(On the constant: free fall in this engine is `a_y = −g − k·v_y` with
g = 24.78 ± 0.10 and k = 0.032, so 25.20 is `a_y` at v_y ≈ +13 m/s rather than a
map constant — quote it with its `v_y`. It does not touch this argument: the gap
here is 18.4 m against 92.5 m, and no value of `g` in the engine's whole
measured range of 22 – 29 m/s² closes a factor of five.)*

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

**What that column is.** A census of the map's own block list finds **zero blocks
whose name contains "Reactor"**. What the map actually carries is **9
`GateExpandableSpecialBoost` gates and 9 `RoadBumpSpecialTurboRoulette` bumps**,
plus 18 reset gates and 18 no-steering gates. The lift itself is real and
measured — the car climbs **19.2 m between 4.25 and 7.50 s** while accelerating
11.8 → 182 km/h, with no block beneath it for the first 1.5 s and vertical
acceleration nowhere near the −25.20 m/s² of free fall, so it is neither
supported by geometry nor ballistic. Three boost gates sit 6.6–12.1 m away during
that climb, and the run passes within 0.7 m of a turbo bump at 0.350 s. That is a
coincidence in space and time; **the mechanism is not demonstrated** and this
page does not claim one.

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
point **0.375 m short in y for 85 rounds and 25,500 candidates**; re-sweeping the
other axes at that rung showed the run could already reach y 292 and was 4 m
short in **x**.

After the first finish at 13.349, the search improved twelve times in 94 seconds
and then found nothing in a further 2.3 million evaluations at a 74 % finish
rate. Two fresh basins were tried; both are blocked by the lift-column geometry
above.

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

> **That 0.48 mm is now a reason to re-check, not a certificate — corrected
> 2026-08-22.** ≈0.5 mm has been measured to be **the distance between two
> copies of the car in the server's memory**, so agreement at that figure is
> what two readings of the *wrong* copy look like; a gather that has found the
> car agrees bit-identically or at ~0.000001 m. Note also that "26 of 33 agree"
> is a **reproduction count, and a majority must never outrank a test that can
> identify the answer** — five regenerations of one 134672 tape produced the car
> once and four wrong picks, two of which agreed with each other to the metre.
> The time is unaffected: the oracle reads the tape. See `tools/README.md`.

## Files

| file | what |
|---|---|
| `replays/TAS_12759.Ghost.Gbx` | **the ghost the clip is shot from**, regenerated from the tape below on 2026-08-23. `ghost verify --map` V1–V11 clean, kappa 1.000, oracle 12.759 on the written file |
| `inputs/TAS_12759.inputs.csv` | per-tick inputs — **the run itself**: this is what the oracle validates at 12.759, and `ghost tape csv replays/TAS_12759.Ghost.Gbx --from -1480` reproduces this file byte for byte |

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
