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
**3 recorded runs** (board 2026-08-24, unchanged). The author time has not been beaten. The "world record" is
the author's own alt account, 18.150 slower than the time they set themselves.

## The map's joke

The finish gate is an item sunk below the surface of the roof. A car resting on
the roof sits **above** the trigger, and driving over the finish does nothing at
all. All three recorded runs drive over it repeatedly without finishing.

The near-miss is not a matter of millimetres of height, and the paragraph that
used to stand here saying it was has been **retired**. The trigger tests a point
fixed in the car's BODY, about 0.84 m up its own up-axis — the car's roof — so
what it measures is *attitude*, not altitude: inverting the car moves the tested
point 1.7 m down, and that is why all three humans finish upside down. Model-free
witness, one tape and one gate: rank 1 reaches a tested point of 143.962 without
finishing and then finishes with a tested point of 144.486, **half a metre
higher**. Upright on the fast line the tested point sits 1.087 m above the roof
plane and the trigger needs plane − 0.034: **about 26° of body tilt, or nothing.**

That near-miss costs rank 1 **10.6 s** and rank 2 **37.2 s** of driving on,
turning round and coming back to fire the gate from the other side.

What the record does wrong on its second pass is simple: it **snaps to full
right lock on the roof and climbs out of the trigger volume about 4 m too
early.** It needs to stay in the box a moment longer.

## Where this is actually played

The map is 113 blocks, every one of them at y = 10, and **two waypoints: the
spawn and the Goal. There are no checkpoints.** The Goal stands at
(419.03, 144.00, 1704.64) — 500 m past the last block in z and 134 m above the
highest one. Every surface the endgame is driven on belongs to the **Stadium
decoration**, not to the map.

The 113 blocks are one walled corridor from the spawn at (144, 10, 656) to
(1296, 10, 1168), screens stacked to y = 34 on both sides. It is the only
accelerator on the map — off it, from a standstill, the car does about 6 m/s —
and it has exactly one exit. So the route is forced: out along the corridor at
620 km/h, off the north-east corner, onto the stadium, and west along it.

Measured with drop probes (`tmmaps dropscan`), the stadium presents a deck at
**y ≈ 50, z ≈ 1585** across x 592…944, and a high rim at **y ≈ 145…158,
z ≈ 1620…1670** from x ≈ 340 to x ≈ 1180 that carries the finish, falling to
y ≈ 110 at x ≈ 1449 and y ≈ 99 at x ≈ 1591. The run's launch apexes at 74.9 m
and passes *under* the rim's east end.

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

## The roof inverts a car that falls onto it

The finish needs the car about 26° past level, and everyone assumed that meant
finding a *rotation source* on the roof. There is a much simpler one, and it has
been there all along: **fall onto the roof from above and the car lands upside
down.** Measured from the live engine — a car dropped 36 m onto the roof's south
face is at `u_y = −1.000`, completely inverted, one second later, with no input
at all, doing 100 km/h and sliding straight at the finish on the finish's own
plane. Every previous inverted crossing was a 4-to-14 km/h crawl that cost 8.42 s.

**A wall at z = 1667.6 stops it 37 m short.** The car goes 60 → 13 km/h in one
tick and sits there. 892 tapes were thrown at that wall from one drop point (521
of them fire the last rung before it, none fires anything past it), and 1 386
more from six others, on the fast route's own up-ramp line, with the fast route
itself firing all six of those rungs in order as the control. Nothing crosses.
East of x ≈ 460 there is no roof above the deck to fall onto at all.

That reframes the endgame's question from "where is 26° of tilt" to "how does
the run get above the finish face with air under it" — **and both ways onto that
face are now measured shut.** From the north, 3 904 evaluations against the wall
across four arrival speeds and nine drop points, with a firing control in every
batch, put nothing past it; the one place where a faster probe does clear the
wall, at x ≈ 496, it also clears the roof, because x = 507 is the world edge.
From the south, 768 overrides of the run's own ramp line give **152 ballistic
episodes on the roof and 0 on the finish face** — every one of them the same
z ≈ 1668 lip, 36 m past the patch, worth 0.1 s and under a metre. The ramp is
smooth and concave from the foot of the climb to the patch: there is no edge to
leave.

So the rotation the finish needs exists, is free, and comes at 100 km/h — and
the map keeps it on the far side of a wall that nothing can cross.

## The author time is not beaten, and here is the size of it

Our fastest upright line reaches the finish patch at **41.037** and
cannot fire the sunken gate; the only flip anyone has made work costs
**+8.95 s** over the last 133 m, so a run must reach the foot of the final climb
by **34.13** to come in under 43.079 — against **37.978**, the earliest arrival
there by any tape ever built on this map. The full measurement set, the geometry,
and the enumerated negatives are in
`~/persistent/private-30d/tm-unbeaten/285885/roof285885_RESULT_v1.md`.

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
27 s slower overall for other reasons. **UNKNOWN — do not read this as an
unsearched lead.** This page has already had to withdraw one claim of exactly
this shape (*"rank 1's 11.2 s approach has never been searched, ~6 s
unclaimed"*, which was wrong on both halves — that approach is what our own
50.229 does). Nobody has checked whether rank 2's approach is likewise something
we already drive. What would settle it: score rank 2's line at the same stations
as the incumbent and see whether it is a distinct route or the same one.

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

> **The 0.84 m model is a description, not a predictor, and it must not be used
> to rank candidates.** Scored as `y + 0.84·u_y`, **six probes it rates 0.10 –
> 0.17 m better than the incumbent fire nothing on the oracle ladder, while the
> incumbent fires at 144.070.** A model that mis-orders the candidates it is
> used to order is a correlate. It is kept here because it explains why the
> humans are upside down, which it does; the *conclusions* below rest on the
> 424-station sweep and on the gate-at-the-crossing-point validation, both of
> which are oracle results and neither of which uses this model.

Everything else is negligible: suspension compression is 3–8 mm and does not
change with speed (there is no downforce lever here), full lock leaves the car
*flatter* than not steering, and the roof under the finish is one clean plane at
11.4°, so a car sitting on it simply takes the plane's attitude. Firing the gate
upright from the fast line would need about **26° of body tilt**, and the
cheapest source of that tilt on this map is rank 1's own flip, which costs
**11.2 s** against a budget of about 2 s.

### The wall is height

Sweeping the goal over a raw (x, z) grid at its true height produces a fire
region whose edge sits 1–1.5 m away, which reads as "short in z, not in height".
That is an artefact of displacing the gate in raw axes. Swept **along the roof
plane** — which is what a car displacement actually means on an 11.4° slope —
the answer is unambiguous: **424 stations spanning 12 m × 8 m, and the fast line
fires none of them**, while the human record fires about 40 in a coherent band.
The instrument says yes where there is a yes. The deficit is height, and no
amount of moving the car along the roof reaches it.

### Rank 1's flip is what this run already does

The flip is a **low-speed pitch-over**: the car climbs a short steep face at
70–90 km/h, loses the surface, tips over backwards, lands on its roof having
travelled 8 m and lost 62 km/h, then drives the last 133 m inverted. **Our
50.229 already does exactly this**, at the same place, and its inverted crawl is
already twice as fast as the human's — 8.42 s against 21.2 s over the same
133 m.

The arithmetic closes it. The fastest anything reaches the top of the steep
climb is 37.97, the flip itself costs about 3.2 s, and the inverted crawl adds
8.42 s: **a floor of roughly 48.5 s against an author time of 43.079.** Free
time upstream would not be enough.

### What is closed

The banked-surface lead has now been tested and it is dead — but read the
evidence carefully, because the first version of this section rested on a survey
that could not have seen the thing it reported absent.

At race 35.0–35.4 both the fast line and rank 1 ride something that puts them
74° on their side at 165 km/h, 142 m from the finish patch. The old reading was:
*"a 797-probe fan across the whole approach found 580 airborne episodes and the
nearest one comes within 82.6 m of the patch"* — with **coverage complete at 5 m
out to ±40 m and 10 m out to ±80 m**. Those two sentences do not sit together. A
fan reaching ±80 m around the patch **structurally could not see a rotation
source 142 m away**, and the source at 142 m is the one this very page names in
the paragraph above. *"No rotation source within 82.6 m"* was a statement about
where the fan looked.

**What actually closes it** is the window itself, searched directly: the fast
line is airborne at race 34.86–35.43 rolled 74° at 167 km/h, and **828 overrides
inside that window give 0 tilted arrivals, 0 earlier arrivals at the patch and 0
finishes**. A further 968 launch overrides give an apex ceiling of **86.1 m**
against a rim at 98.9. The lead is closed on the evidence that can carry it.

The rest of the old paragraph survives and is worth keeping: the lowest body-up
component anywhere on the roof within 20 m is 0.970 — essentially flat — and the
one tilt source that exists, a wall 36.4 m away, self-corrects in 0.30 s and
leaves the car wedged at 4 km/h at full throttle for the rest of the race.

**And the near miss is real, not a car falling past the plane.** A finish gate
is a plane you cross rather than a region you occupy, so a candidate can be
nearer the gate in every spatial sense and still trigger nothing. That does not
apply here, and the control is one validation: a gate placed at the incumbent's
own crossing point, at the real gate's real height, **fires** (40.964). The car
is travelling at ~50 m/s horizontally with *positive* vertical velocity —
climbing the ramp, ratio 0.15 — where a faller would need |vy| to exceed |vx|.
What it cannot do is fire the gate 5.29 m away where the gate actually is; raise
that one by 70 mm and it fires (41.074). The deficit is height, and it is
genuine.

Both search windows are frozen: 3483 candidates, 0 improvements, and all 78
splice handovers after the divergence are dead. **Neither of those two nulls has
a positive control**, so read them as "this search stopped finding things", not
as "there is nothing there" — the map's *other* nulls do have controls (the
424-station sweep fires ~40 on the human record; the gate at the incumbent's own
crossing point fires at 40.964) and those are the ones to lean on.

So the statement of this map is that **"improve our own record" and "beat the
author time" are different projects here.** The one lever with a measured
non-zero gradient is upstream of the launch — and it must be *re-searched* on
the fast lineage rather than ported, since a 0.201 s edit that works on the
older lineage DNFs on this one despite both tapes sharing those exact inputs.
That lever can improve 50.229. It cannot reach 43.079.

One more local measurement, and a correction to how it used to be stated here.
This page used to say: *"free fall on this map is −24.308 m/s², not the −25.20
measured elsewhere. Gravity here is per-map."* **The measurement is fine and the
conclusion was not.** Free fall in this engine is not a constant — it is linear
drag in vertical speed,

    a_y = −g − k·v_y        g = 24.78 ± 0.10 m/s²,  k = 0.032 ± 0.002 /s

fitted per-arc on four tapes across three maps (fleet notice F1994616772). A
scalar "gravity" is just `a_y` sampled at whatever `v_y` the probe happened to
span, and the two numbers this page contrasted are the *same law* read at two
speeds:

| quoted | v_y it implies | `−24.78 − 0.032·v_y` |
|---|---|---|
| −25.20, "measured elsewhere" | +13 m/s | **−25.20** |
| **−24.308, here** | **−16 m/s** | **−24.27** |

So: **MEASURED** — `a_y = −24.308 m/s²` at this map's probe speeds, and that is
the number to use for a probe at those speeds. **NOT SUPPORTED** — "gravity here
is per-map", which needs two *intercepts* to differ and compares two `a_y`
values instead. **UNKNOWN** — whether `g` really is per-map: the fleet's
intercept is fitted on one map (208024), and this map's own published tape
cannot settle it, because `tmtraj motion TAS_50229.Ghost.Gbx --fit-g` finds a
longest free-fall stretch of **2 samples, 14.770 … 14.820 s**. A 0.05 s lever
arm identifies nothing, which `tmtraj`'s own documentation already says. What
would settle it is re-fitting a second map against `v_y` on arcs long enough to
have an intercept.

Never quote a scalar `g` here without the `v_y` it was measured at.

## Files

| file | what |
|---|---|
| `replays/TAS_50229.Ghost.Gbx` | the fastest validated run — 11.0 faster than any human |
| `replays/POKE_1input_50659.Ghost.Gbx` | **the world record with one steering change** — 61.229 → 50.659 |
| `replays/TRIGGERPOKE_50469.Ghost.Gbx` | the same idea, refined |
