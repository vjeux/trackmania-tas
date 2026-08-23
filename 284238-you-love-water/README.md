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

> **That kicker-height experiment is VOID, 2026-08-22.** The 1.00 m kicker is a
> **four-block assembly with one free block**, so moving "one block's f32 `y`"
> moved a quarter of the kicker and built a step in it. The same mistake voids
> the raise experiment. **A matched A/B is only matched if it moved the whole
> object** — and the car arriving bit-identically is exactly what makes that
> invisible.

The one encouraging measurement: a standing start off copy 0 flies the good line
to within 2–7 m of it, point for point. The line is not exotic and this car can
drive it — copy 0 has the run-up that produces it and the others do not.

The author made exactly this substitution himself on his remix **279008 "Keep
dropping"**, which shares 167 of its 186 blocks with this map but replaces the
water ramps with tech blocks that give every copy a flat run-up. A human beats
that map's author time. Nobody has beaten this one.

## What the obstacle actually is: the engagement point

> ### ⚠️ RETRACTED 2026-08-22 — the discriminator is the crossing angle, not `engage_x`
>
> **Everything in this section is a real measurement and the law it states is
> not the mechanism.** `engage_x` correlates with the frozen roll rate across
> the 27 tapes below, and the later work shows what it was standing in for:
> **the crossing angle `vz` at the ice kicker.**
>
> The correction was earned the right way round — **by perturbing the run that
> WORKS** instead of inferring a bar from our own failures. Taking Yhomas's own
> 46.112 on the sibling map and moving one flag at a time: he **rides at
> vz −23.72 and unloads at −22.91**, and his checkpoint arrival falls
> 69.40 → 55.47 → 43.83 → **22.44 m/s** as vz weakens to −18.4. **Our record
> engages at vz −2.3**, and the best shot at −18.2. That is the whole 47 s.
>
> And the reason it cannot simply be steered to — **superseded, see below.**
> The measured coupling is real: **the water launcher is a shallow trough**
> (−0.78 °/m, floor at Z 927.4) where his tech deck is flat (0.09 °/m).
>
> **SUPERSEDED THE SAME DAY: the roll IS reachable, and it is not enough.**
> The four searches behind "loading and rotating are the same variable" all
> minimised a **weighted sum** of position, velocity and attitude — and a
> weighted sum lets a candidate *buy attitude with crossing angle*, which is
> exactly the trade the surface forces. So the search settled on the surface's
> own trade curve and reported it back as a frontier. Put the crossing angle and
> the speed on **hard bars** instead and roll comes down to **+1.13 at
> vz −22.8**, and **+1.29 at vz −23.6 at 95.9 m/s** — the human's own attitude,
> on the lane that had just been published as unable to produce it.
>
> **What the retraction did NOT overturn is any measurement.** The coupling, the
> threshold and the matched pair all stand. What changed is a **sufficiency**
> claim: both cars now cross the kicker within 2.6° of roll and both reach
> ~240 °/s, and the difference appears **17 m later** — his wheels stay loaded
> and the rate bleeds 242 → 189 → 129 → 87, while ours goes rigid and holds
> **240.3 to the last digit**. The stronger negative was hiding behind the
> weaker one, and the scoring shape is what kept it hidden.
>
> Details: `tm-unbeaten/284238/RESULT.md` and branch
> `wtr-284238-crossing-angle`.
>
> **Two experiments on this page are additionally void**, and for a reason worth
> keeping: **the 1.00 m kicker is a four-block assembly with one free block**, so
> both "raise/lower the kicker" A/Bs moved a *quarter* of it and built a step
> (99.81 → 50.84). A matched A/B is only matched if it moved the whole object.
>
> Read the rest of this section as **the measurements, which stand**, and not as
> the law. Details: memory key `tm2020-map284238-symmetry.md`.

**The obstacle is governed by one number — `engage_x`, the point on the curved
ice kicker where the car first engages it.** That sets the car's roll rate at the
suspension unload, and after that instant every input is bit-exactly inert. 27
tapes across five families of intervention fall on one monotone curve:

| engage_x | frozen roll rate | how it was reached |
|---|---|---|
| 911.0–911.7 | **250 … 284** | no intervention; **all three lifts**; all six steer pulse pairs; brakes ≤ 40 ticks |
| 912.54 | 145.3 | brake 60 |
| 913.76 | 136.9 | brake 30, ended 40 ticks early |
| 916.57 | 121.1 | brake 30, ended 60 ticks early |
| 916.86 | 120.8 | brake 75 |
| **917.99** | **71.0** | brake 80 |
| 920.23 | −4.9 | brake 30, ended 80 ticks early |

**The roll the finish needs — about +86 at the wall, a rate near 90–100 — is
bracketed by measured points at 916.86 and 917.99.** Not extrapolated. And note
the shape: the curve sheds 24 °/s across the 4.4 m from 912.5 to 916.9, then
**50 °/s in the next 1.1 m.** The window that matters is roughly **half a metre
wide, sitting on a knee.**

**It is not speed, and the control that proves it is the lift family.** Lifting
the throttle sheds speed 98.55 → 95.24 m/s and moves the roll rate by 16,
non-monotonically. Braking over the same speed span moves it by **279.** Two
interventions costing the same speed, differing seventeen-fold — so what braking
does that lifting does not is *move where the car meets the ramp.*

### Two levers, and the door is shut between them

**The free one.** A pair of equal and opposite steer pulses shifts the car
1.67 m sideways, hands the heading back to within 0.05°, and costs no speed at
all. It moves `engage_x` by **0.24 m** — a gearing of 0.14 m per metre of shift,
so reaching 917 would need about **38 m of lateral shift on a lane that is not
38 m wide.** The lever is real, monotone in both directions and free, and it is
thirty times too weak.

**The paid one.** Braking reaches the window and destroys the line. Composed
properly — brake in the seed, steering free to re-aim around it, a hard gate
requiring the wall be reached — ten seeds over two passes: every seed that kept
enough brake to move the engagement point was **gated out 48–50 m short of the
wall**, and the one seed per pass that did reach the wall had spent its brake and
arrived at roll −147 and −172 against the target of +86.

So inside a 190-tick window the steering cannot recover the ~50 m that the
required brake costs. **That is a mechanism, not a budget.**

**The open question, for anyone who wants it:** the two levers above are
superseded by the crossing-angle finding at the top of this section — the target
is **vz ≤ −24 at the kicker with the wheels still loaded**, and the trough
geometry makes those two demands the same variable pulling opposite ways. The
one lead nobody has spent: **the water start is 1.30 s with no upstream coupling
and has never been searched** with a time-varying control. It may simply be
worth more than any of this.

## The record's time is mostly retries

On a Trial-family map the clock runs through respawns, so a recorded time is
clean driving plus every failed attempt. Take the human's own last, successful
attempt in each sector and his clean driving is **93.914**. Our 97.325 is that
driving with the retries cut.

## Files

| file | what |
|---|---|
| `replays/TAS_97325.Ghost.Gbx` | the best validated run — one life, no respawns |
