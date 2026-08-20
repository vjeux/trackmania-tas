# finish is on the roof to your right

**The finish gate is sunk about a metre below the roof you drive on, so the
world record drives straight over it without finishing and has to come back.
Arrive at the patch and *stop turning right* — almost anything else fires the
gate and saves 10.5 s.**

| run | time | vs human WR | vs author time |
|---|---|---|---|
| **TAS** | **50.229** | **−11.000** | +7.150 |
| TAS, the same idea refined | 50.469 | −10.760 | +7.390 |
| **the world record plus one input** | **50.659** | **−10.570** | +7.580 |
| Author time | 43.079 | — | — |
| Human WR — lasyoppwtf | 61.229 | — | +18.150 |

TMX map [285885](https://trackmania.exchange/maps/285885) · author **lasyopp** ·
**3 recorded runs**. The author time has not been beaten. The "world record" is
the author's own alt account, 18.150 slower than the time they set themselves.

## The map's joke

The finish gate is an item sunk below the surface of the roof. A car resting on
the roof sits **above** the trigger, and driving over the finish does nothing at
all. All three recorded runs drive over it repeatedly without finishing.

The world record's lowest point inside the gate's footprint is

```
y = 145.2506      against a ceiling of      y = 145.2500
```

**It misses by under five millimetres.** Rank 2 misses by about 15 mm, rank 3 by
about 20 mm — the same near-miss three times, not three different routes. That
near-miss costs rank 1 **10.6 s** and rank 2 **37.2 s** of driving on, turning
round and coming back to fire the gate from the other side.

Raise the gate by five millimetres and all three runs finish about ten seconds
earlier. **Five millimetres of gate height is worth 10.170.**

What the record actually does wrong is simple: it **snaps to full right lock on
the roof and climbs out of the trigger volume about 4 m too early.** The car is
already low enough on the way up — it does not need a landing, a bounce or a dip.
It needs to stay in the box a moment longer.

## The run, as inputs

Take the world record's own tape and change one thing:

```
race 50.910   hold the steering ~7 % LEFT for 0.18 s
              (where the record snaps to full right lock)   ->  51.059
race 50.440   hold ~50 % RIGHT for 0.25 s                   ->  50.659
```

Both are one input on the roof arrival, and the second is the best simple
version. The published tape is a clean override of the steering channel rather
than a literal key press, so the thing to trust is the tolerance below, not the
tape's exact contents.

**The sector-by-sector guide for the rest of this map is not written yet.**

## How forgiving it is

Absurdly. Sweeping the held steering value at that instant:

| value held | result |
|---|---|
| hard left, −127 … −30 | DNF — the run is lost |
| gentle left, −20 … −6 | 51.059 – 51.069 |
| **neutral — just stop steering** | **51.049** |
| anything right, +3 … +127 | 51.039 – 51.069 |
| what the world record actually does | 61.229 |

**Sixteen of the twenty-three values tried fire the gate ten seconds early, and
one of the few that does not is the record's own input.** Neutral works. Full
right lock works. Only hard left loses the run.

The timing window is about **550 ms** wide, with a couple of chaotic holes in it,
and hold durations anywhere from 0.08 to 0.5 s all work. For a driver this is
not a trick:

> **Arrive, and do almost anything except what the record did.**

## Where the remaining time is

Firing the gate on the way up removes a fixed 10.5 s. The author time is still
7.4 s further on, and all of that is in the route before the roof.

**The hairpin is the whole deficit, and it is bigger than the deficit.** The
record lands at 36.049, drives *away* from the finish — west and downhill — to a
hairpin apex at 40.074, turns round, and does not get back to the same place
(25 m higher) until 44.789. **That U-turn costs 8.740, and the map needs 7.400.**
So this is a landing problem, not a rooftop-crawl problem: arrive on the upper
roof at around 36 s instead of 41.7 s and the author time is gone without
touching the finish at all.

Rank 2 is worth studying here: it never touches the hairpin at all, reaches the
roof within 0.4 s of the world record by a completely different approach, and is
27 s slower overall for other reasons. Nobody has looked at that approach.

There is also a faster way up: full throttle over the roofs at **150–190 km/h**
where the field lifts off to 30–100 km/h. That line reaches the finish patch
9.6 s earlier than any human — but it arrives about **70 mm too high** to fire
the gate, and 70 mm is not recoverable on that line.

### Why height alone will not do it

The trigger does not test the middle of the car. It tests a point fixed in the
car's body about **0.84 m above its centre** — roughly the roof of the car.
Turning the car over moves that tested point **1.7 m down**, which is why all
three humans finish **upside down**, and on this map that is a necessity rather
than a fumble.

Everything else is negligible: suspension compression is 3–8 mm and does not
change with speed (there is no downforce lever here), full lock leaves the car
*flatter* than not steering, and the roof under the finish is one clean plane at
11.4°, so a car sitting on it simply takes the plane's attitude. Firing the gate
upright from the fast line would need about **26° of body tilt**, and the
cheapest source of that tilt on this map is rank 1's own flip, which costs
**11.2 s** against a budget of about 2 s.

### The wall is height, and it is not a measurement artefact

An earlier reading suggested the deficit might be lateral rather than vertical —
sweeping the goal over an (x, z) grid at its true height produces a fire region
whose edge sits 1–1.5 m away, which reads as "short in z, not in height".

That is an artefact of displacing the gate in raw axes. Re-swept **along the
roof plane** — which is what a car displacement actually means on an 11.4° slope
— the answer is unambiguous: **424 stations spanning 12 m × 8 m, and the fast
line fires none of them**, while the human record fires about 40 in a coherent
band. The instrument says yes where there is a yes. The deficit is height, and
no amount of moving the car along the roof reaches it.

### Rank 1's flip is not an unsearched lead — it is what this run already does

An earlier version of this page said rank 1 had a validated way to finish that
had never been searched, with about 6 s unclaimed upstream. That was wrong on
both counts.

The flip is a **low-speed pitch-over**: the car climbs a short steep face at
70–90 km/h, loses the surface, tips over backwards, lands on its roof having
travelled 8 m and lost 62 km/h, then drives the last 133 m inverted. **Our
50.229 already does exactly this**, at the same place, and its inverted crawl is
already twice as fast as the human's — 8.42 s against 21.2 s over the same
133 m.

The arithmetic closes it. The fastest anything reaches the top of the steep
climb is 37.97, the flip itself costs about 3.2 s, and the inverted crawl adds
8.42 s: **a floor of roughly 48.5 s against an author time of 43.079.** Free
time upstream would not be enough, and there is no 6 s upstream to find.

### What is actually open

Banked surfaces. At race 35.0–35.4, one second after the launch landing, both
the fast line and rank 1 ride something that puts them **74° on their side at
165 km/h**, 142 m from the finish patch. So this map does contain geometry that
tilts this car far past the 26° the gate needs, at speed, without the
pitch-over's 3.2 s. Three separate efforts have measured the finish ramp as a
bare plane; nobody has mapped what is beside it.

That is why the finish is a dead end and the approach is not.

## Files

| file | what |
|---|---|
| `replays/TAS_50229.Ghost.Gbx` | the fastest validated run — 11.0 faster than any human |
| `replays/POKE_1input_50659.Ghost.Gbx` | **the world record with one steering change** — 61.229 → 50.659 |
| `replays/TRIGGERPOKE_50469.Ghost.Gbx` | the same idea, refined |
