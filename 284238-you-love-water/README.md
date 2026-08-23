# You love water

**The whole map is one 71 m gap, jumped four times. Cross the lip at about
300 km/h and you fly it; cross at 255 and you fall into the tube below and lose
9–15 s. The only recorded human run misses it three times out of four.**

| run | time | vs author time | note |
|---|---|---|---|
| **TAS** | **97.325** | +46.866 | one life, no respawns |
| Human record — brick555 | 440.238 | +389.779 | contains 31 respawns |
| Author time | 50.459 | — | never beaten |

TMX map [284238](https://trackmania.exchange/maps/284238) · author
**Eating_My_Wings** · 4 checkpoints · **exactly one recorded run**.

The author time still stands. What follows is what the map is and where its time
goes, not a route that beats it — **the sector-by-sector guide for this map is
not written yet.**

## The map is one obstacle, built four times

186 blocks: the same 40-block module placed four times, plus the finish. Route
order is copy 0 → copy 3 → copy 1 → copy 2 → finish, and each copy is the same
cycle:

```
  launcher lane, two boost pads   ->  ~300 km/h
      |
      v  ~3.1 s of flight
  CHECKPOINT
      |  a curved chute, dropping ~100 m over ~3.5 s
      v
  THE LIP
      |
      |   *** THE GAP: 71 m across, 32 m down ***
      v
  FAR LIP  -> ride the half-pipe tube down and out onto the next launcher
```

Every one of the four timed cycles launches off a **water ramp**. Fall short of
the gap and you land in the closed end of the tube — the "bowl" — and have to
climb back out.

## Where the time goes: the lip

Across the 23 attempts inside the human record:

| speed at the lip | attempts | outcome |
|---|---|---|
| **302 and 305 km/h** | 2 | cleared the gap; reached the exit lane 5.300 s later |
| 61–255 km/h | 21 | fell in; when they reached the exit lane at all it took 14.5–20.4 s |

The threshold is **bracketed between 255 and 302 km/h**, not measured. Missing
the gap costs 9–15 s, and the record misses it in three of the four copies. That
is the 40 s of the run right there, before any respawns.

Lip speeds in that record, by copy: **305 km/h** (copy 0, cleared), then 240, 251
and 210 km/h — all three fell in at 89–129 km/h.

## Why copies 1–3 are so much harder than copy 0

Whether you arrive at the lip fast is decided far earlier, at the **wall curve**
after the launch — specifically by how far along the wall you meet it:

| | where the car meets the wall | speed lost in one tick | checkpoint crossing |
|---|---|---|---|
| the recorded run, cycle 1 | z 923.4, 77.4 m/s | 8.71 m/s | 45.8 m/s |
| off the start platform, copy 0 (works) | z 915.4, 73.1 m/s | — | — |
| a human on the author's remix | z 913.9, 80.8 m/s | 0.75 m/s | 69.4 m/s |

**Nine and a half metres of lateral position** separates a cycle that keeps its
speed from one that throws it away, and checkpoint speeds decay
52.8 → 45.8 → 41.1 → 37.4 m/s as the cycles go on because each bad contact feeds
the next.

**The obstacle is a surface to be ridden, not a gap to be jumped.** The human
crosses the ice kicker and the wall curve with his wheels **loaded the whole
way**, rolled 90° onto the car's side. Copies 1–3 launch our car off the kicker
instead, and from the instant the suspension fully unloads its angular velocity
is exactly constant and every input is inert — so the wall contact is decided by
the rotation at the ramp exit and by nothing afterwards.

Body-frame angular velocity from the ramp exit, in °/s:

| | exit | +0.10 s | +0.20 s | +0.30 s | +0.50 s |
|---|---|---|---|---|---|
| the human's 46.112 (works) | 222 | 150 | 92 | 84 | 68 |
| ours, copy 0 (works) | 275 | 142 | 81 | 57 | 46 |
| ours, copy 1 (fails) | **284.1** | **284.1** | **284.1** | **284.1** | **284.1** |
| his inputs grafted, copy 1 | **267.7** | **267.7** | **267.7** | **267.7** | **267.7** |

Identical to every printed digit for 40 consecutive ticks: a free rigid body.
His dampers read 0.039 at the first no-contact sample and extend *gradually* to
0.180 over 450 ms, never reaching full extension — he never leaves the surface.

The decisive test: grafting the human's own steer, gas and brake from the kicker
onto a launch matched to his state (2.61 m, 98.31 against 98.10 m/s, slip/pitch/
roll within 1.5°) gives roll at the wall **−150 against his +86**, flat across a
±60 ms phase sweep. **This is not a missing input sequence; it is a missing
state, and the state is angular velocity.**

Steering forced over the true flight window returns bit-identical state at the
wall for −1, −0.3, +0.3 and +1 — on copy 1 *and* on copy 0 (0.6° across the full
range).

Position, by contrast, is essentially solved — the human's crossing point is
reached to **0.58 m**, from 9.73 m earlier the same day. Roughly forty
coordinate-descent runs across eight objective variants, windows from tick 2000
to 2365, seeded both randomly and from the incumbent's own inputs, put roll at
the wall in **−131 … +176** and never within 100° of the human's +86.

The kicker's geometry is also cleared, on all three axes at once: applying
copies 1–3's *whole* offset from the sibling map (+0.70, −1.00, −0.29) to copy
0's kicker, in a matched A/B so the car arrives bit-identically, leaves the
working launch working — roll within 2.5° of control throughout. It is not what
separates the copies.

So the remaining lever is narrow and upstream: the ramp does the same thing to
whatever it is handed, so the target is the state at the **ramp entry** — the
tube and the arc, upstream of anything searched on this map — aiming to unload
at ~95 °/s instead of ~270.

Lateral position on the flat before the kicker is still what separates the
cycles, and copies 1–3 do not have that flat:

| | time on the flat | sideways speed achieved |
|---|---|---|
| copy 0 — start platform, ~100 m of deck | ~2 s | −17.9 m/s |
| copies 1–3 — fed by the tube | ~0.6 s | −1.9 as driven … −15.7 at full lock |

The tube is the only connection between one copy and the next, so copies 1–3
always arrive on the lane 100 m late with 0.6 s of flat left — and in 0.6 s the
car cannot build the sideways speed the wall contact wants.

It is **not** grip (full lock buys 13.4 m/s of sideways speed on this lane), not
speed (the kicker is crossed at 97.2 m/s on the attempt that fails and 90.9 on
the standing start that works), and not the six boost pads on the lane (they sit on the flat
*after* the aim is decided). It is also **not** the kicker's height: lowering
copy 0's kicker by 1.00 m in a matched A/B — one block's f32 `y`, so the car
arrives in a bit-identical state — leaves the roll profile essentially unchanged
(+105.2 against +108.4, both locking at +89.5 and driving the wall).

The one encouraging measurement: a standing start off copy 0 flies the good line
to within 2–7 m of it, point for point. The line is not exotic and this car can
drive it — copy 0 has the run-up that produces it and the others do not.

The author made exactly this substitution himself on his remix **279008 "Keep
dropping"**, which shares 167 of its 186 blocks with this map but replaces the
water ramps with tech blocks that give every copy a flat run-up. A human beats
that map's author time. Nobody has beaten this one.

## What the obstacle actually is: the crossing angle, and a window half a metre wide

> **The "engagement point law" that used to stand here is RETRACTED.** It said
> the frozen roll rate is a monotone function of `engage_x` — 912.5 → 145 °/s,
> 914.6 → 126, 918.0 → 71 — measured over 27 tapes. The 27 rows are correct and
> the law is not. Move the *kicker* instead of the car, one f32 on the map with
> the same tape and the same inputs, and `engage_x` travels 3.6 m for **12 °/s**:
>
> | map | engage_x | frozen roll rate | the law predicted |
> |---|---|---|---|
> | untouched | 910.76 | **284.1** | — |
> | kicker −0.50 m | 912.85 | **258.7** | ≈ 140 |
> | kicker −1.00 m | 914.34 | **272.7** | ≈ 126 |
>
> It was a within-family correlation of the brake sweep — the only family in
> that table with any variation, and the one in which the engagement point and
> the crossing angle move together.

### The number that decides it, measured on the human's own tape

The discriminator is **`vz` at the kicker: the crossing angle.** It is not
inferred from our failures — it is measured by perturbing the run that works.
Twenty-four one-flag perturbations of Yhomas_TM's 46.112, on his own map,
through one readout:

| crossing angle at the kicker | outcome | speed at the checkpoint |
|---|---|---|
| −26.34 | rides | 65.26 |
| −25.00 (unperturbed) | rides | **69.40** |
| −23.72 | rides | 71.70 |
| **−22.91** | **unloads** | 72.34 |
| −22.09 | unloads | 55.47 |
| −19.77 | unloads | 43.83 |
| −18.37 | unloads | **22.44** |

Every row at −23.72 or steeper keeps its wheels loaded; every row at −22.91 or
shallower launches. **The threshold is a 0.8 m/s window at 99.4 m/s** and a
6/127 steer offset held half a second crosses it. His tolerance in lateral
position is the same size: +0.26 m still rides, +0.43 m does not.

**284238's own record engages the kicker at vz −2.3**, and the best synthesised
shot at −18.2. The perturbation that puts *him* at −18.4 arrives at the
checkpoint at 22.44 m/s against his own 69.40. That is this map's whole deficit,
in one number, on his map, with his car.

### What our lane can deliver, and the one thing it cannot

Four searches, 36 restarts, aimed at his kicker state shifted by the launcher
assembly's own offset so it is the same place relative to the kicker:

| | target | best reached on our lane |
|---|---|---|
| position | 0.00 | **0.07 – 0.19 m** |
| speed | 99.22 | 97.9 |
| crossing angle | −25.20 | **−25.13** |
| body angular velocity | (−54.0, +15.2, −57.3) | **(−55.4, +15.3, −58.7)** |
| **roll** | **+1.23** | **+2.80**, and +4.5 in the rows that hold the rest |

Position, velocity and angular velocity are all solved. Roll is not, and not for
want of trying: every restart holding vz ≤ −22 at ≥ 93 m/s comes back at roll
+2.8 … +5.2, and every restart at roll ≤ +1.3 pays for it in the crossing angle
(−14.5), in position (1.4–12 m) or in 37 m/s of speed. All 36 were then replayed
and scored on the **outcome** rather than on the objective they minimised; none
of them rides.

### Why: the water lane is a trough, and it ties roll to the line

The launcher deck on copies 1–3 is a shallow **valley**. The car's own height on
it, at canonical x 902.5, over ten tapes at 99–100 m/s:

```
z 921.7  y 1873.258  roll +4.8      z 927.5  y 1873.088  roll -0.1   <- the floor
z 924.9  y 1873.104  roll +2.4      z 933.2  y 1873.428  roll -4.2
```

**−0.78 ° of roll per metre across the lane**, level only at z ≈ 927.4. The
sibling's tech deck, and our own copy 0's start platform, are flat: +0.45 at
z 920.9 and +0.54 at z 921.9, 0.09 °/m.

So on this lane:

* **keeping the wheels loaded needs roll ≈ 0**, which means z ≥ 926.4 — the floor
  of the trough;
* **rotation and the wall contact need vz ≤ −24**, which means z ≤ 924, where the
  same surface rolls the car +3.4 to +6.3.

They are the same variable. Walking a seed across that line one steer unit at a
time: at z 926.63 (roll +1.23, vz −12.19) the wheels **never unload** — 99 % of
ticks change the angular velocity, the human's own regime — and one notch off it
(z 926.43, roll +1.66, vz −13.56) it unloads, and every row past that launches.
The loaded tape reaches the wall at **32.30 m/s** against his 79.49, because at
roll 0 the kicker has no lever to rotate the car.

Speed is not the escape either: held on his lateral line, from 97.92 down to
88.19 m/s, the wheels unload every time within a tick or two of the same instant.
Our copy 0 rides at 91 m/s — on the flat start platform, at roll +0.45.

### Three things this closes

* **The 1.00 m kicker.** It is a rigid offset on a **four-block assembly**
  (`tmtraj blockdiff`), of which exactly one block is free-placed — so the two
  earlier experiments that "raised the kicker by 1.00 m" raised a quarter of it
  and built a step: entry speed 99.81 → **50.84**. And the deck already pays for
  it: the car's height above its own kicker at the kicker plane matches the
  human's to **8 mm**.
* **Copies 2 and 3.** The vector from a water ramp's anchor to the ice kicker is
  (32.572, −1.000, −0.164) on copy 1, copy 2 and copy 3 — identical to 1 mm. The
  three launchers differ only by where the whole assembly was dropped.
* **An airborne approach**, which would break the coupling because a car in
  flight carries the roll it left with. The only candidate crest is the floor of
  the trough, and a floor is not a crest: `y` never rises above the local surface
  on any of the ten tapes that cross it.

**What is left**: the water start (1.30 s, never searched, independent of all of
this), and the one question nobody has asked — whether the car can be put on the
kicker from somewhere that is *not* the trough's +4.8° wall.

Full account, tables and md5s:
`tm-unbeaten/284238/wtr_RESULT_v1_the_cross_slope_couples_roll_to_the_line.md`.


## The record's time is mostly retries

On a Trial-family map the clock runs through respawns, so a recorded time is
clean driving plus every failed attempt. Take the human's own last, successful
attempt in each sector and his clean driving is **93.914**. Our 97.325 is that
driving with the retries cut.

## Files

| file | what |
|---|---|
| `replays/TAS_97325.Ghost.Gbx` | the best validated run — one life, no respawns |
