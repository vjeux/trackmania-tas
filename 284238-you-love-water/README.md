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
range). An earlier 28.5° figure came from a window that mistakenly included
about 20 ticks of ramp.

**And the axis that does work is five times too short.** One constant input over
the last 0.50 s of ramp, eleven values spanning the whole alphabet, everything
else identical:

| input | frozen ω_z at unload | unload instant |
|---|---|---|
| steer −1.00 | **285.9 °/s** | 23.700 |
| steer 0 | 270.7 | 23.690 |
| steer +1.00 | **248.9** | 23.690 |
| brake | 256.9 | 23.700 |
| **the human** | **never unloads — 222 decaying to 68** | — |

Monotone in steer, so the instrument resolves the axis — and the same ladder
reports "no effect, bit-identical" on the flight window, which is its control.
**The reachable band is 37.4 °/s wide. The roll the finish needs is 90–100. The
gap is 150.** The unload *instant* is 23.690–23.700 for every input in the
alphabet: one tick of variation. The car leaves the ramp when the ramp says so.

Position, by contrast, is essentially solved — the human's crossing point is
reached to **0.58 m**, from 9.73 m earlier the same day. Roughly forty
coordinate-descent runs across eight objective variants, windows from tick 2000
to 2365, seeded both randomly and from the incumbent's own inputs, put roll at
the wall in **−131 … +176** and never within 100° of the human's +86.

The kicker's geometry is also cleared, on all three axes at once: applying
copies 1–3's *whole* offset from the sibling map (+0.70, −1.00, −0.29) to copy
0's kicker, in a matched A/B so the car arrives bit-identically, leaves the
working launch working — roll within 2.5° of control throughout. Three separate
efforts had pointed at that offset; it is not what separates the copies.

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

## The record's time is mostly retries

On a Trial-family map the clock runs through respawns, so a recorded time is
clean driving plus every failed attempt. Take the human's own last, successful
attempt in each sector and his clean driving is **93.914**. Our 97.325 is that
driving with the retries cut.

## Files

| file | what |
|---|---|
| `replays/TAS_97325.Ghost.Gbx` | the best validated run — one life, no respawns |
