# 146612 — THE EXIT PROBLEM IS UNSOLVABLE, and the reason is a ballistic invariant

2026-08-19 07:33Z. This closes the question posed as "find a launch that lands
aligned with the +z run of sector 5". **It cannot be done, and the argument is
geometric rather than empirical — it is the same argument the other arm used to
kill the sector-3 cut, and it applies to my jump with the numbers measured.**

## 1. A ballistic flight changes horizontal heading by exactly zero

Measured, not assumed. `fk btraj`, heading = `atan2(vz, vx)` in degrees:

```
jump tape, free flight t = 29.85 .. 31.65 s   heading 29.4 29.4 29.4 29.4 29.4
                                                      29.4 29.4 29.4 29.4 29.4
                                                      29.4 29.4  (1.8 s, 0.0 deg drift)
beam tape, free flight t = 30.60 .. 31.50 s   heading 29.1 29.1 29.1 29.1
          after a clip at 31.8 s, t = 31.8 .. 33.0 s  heading 41.6 41.6 41.6 41.6
```

Constant to the printed precision across the whole flight. It has to be: there
are no horizontal forces on an airborne car. **Yaw of the chassis can be
changed in the air; the direction the car is TRAVELLING cannot.** Only ground
contact changes it — visible above as the 29.1 → 41.6 step at the 31.8 s clip.

## 2. The two headings this map demands are 53° apart

* **What the flight must fly.** The ramp is at (944, 10, 592); checkpoint 5 is
  at (1170, 42, 736). The bearing between them is `atan2(144, 226)` = **32.5°**,
  and the jump flies **29.4°**. There is no freedom here: the flight direction is
  pinned by the requirement to reach CP5.
* **What the landing surface runs at.** The human world record's own heading
  through the landing area: **82.2° at z = 736, 83.1° at 751, 88.3° at 766,
  93.2° at 782, 98.1° at 798.** The sector-5 road runs essentially along +z.

**Mismatch at touchdown: 82.2 − 29.4 = 52.8°.** The flight supplies 0° of it.

## 3. The landing keeps cos(mismatch) of the speed — measured

The beam tape, which is the best on-road landing found (station 04 at 35.489,
725 ms better than the greedy crawl, x = 1178.5 so inside the surface):

| t | heading | speed |
|---|---|---|
| 33.00 s | 41.6° | 71.2 m/s |
| 33.10 s | — | 69.0 m/s |
| **33.20 s** | — | **42.0 m/s** |
| 33.30 s | **88.4°** | 41.5 m/s |

The car rotates 46.8° onto the road in one tenth of a second and comes out at
41.5 m/s. Pure projection predicts `71.2 × cos(46.8°) = 48.7`; **the projection
accounts for 22.5 of the 29.7 m/s lost, about 76 %**, the rest being the impact
with a rising banked surface it meets from 1.5–2.5 m up.

This is not a search failure. **It is what happens when you arrive across a
road: you keep the component along it.**

## 4. Therefore the jump cannot be converted into a lap

Arrive at CP5 at 70 m/s with 53° of error and you have **≈ 42 m/s of usable
speed**. The human world record arrives at 75.3 m/s with ~0° of error and keeps
all of it. The jump buys 1.128 s and hands back 33 m/s of entry speed into a
6-second sector. Everything downstream follows: our best sector-5 line from the
jump's state is running ~500 ms behind the world record by station 04 and the
gap grows.

And there is no other launch. To land aligned you would have to fly at ~85°,
which from the ramp at (944, 592) puts you near (960, 781) after 190 m — 210 m
from the sector-5 road, which lives at x ≈ 1170 through that whole z range.
**No launch heading from that ramp both reaches the road and matches its
direction.** The two requirements are 53° apart and a flight satisfies exactly
one of them.

## 5. What this means for the map

The recombination bound stands as arithmetic and fails as a plan:

```
jump CP4  28.144  +  jump sector 4  4.558  =  CP5 at 32.702
required sector 5 to beat the AT      =  38.530 − 32.702  =  5.828 s
best sector 5 ever driven             =  6.147 s  (from rank 2's CP5 state,
                                                   arriving at 75 m/s, aligned)
sector 5 available from the JUMP's CP5 state, measured  =  worse, not better:
                                          ~42 m/s of usable entry speed
                                          against rank 2's ~75
```

The 7 ms of slack in the fleet bound was computed with a sector 5 driven from a
*different, better* CP5 state. From the jump's own state that component is not
6.147 and is not achievable — the entry speed is 44 % lower. **The bound is not
7 ms short of the author time; it is unreachable through this jump.**

## 6. The transferable law — same shape as the sector-3 result

> **A gap jump changes your horizontal heading by exactly zero. Before valuing
> one, compare the heading you must launch at to reach the landing, with the
> heading of the surface you land on. You keep cos(difference) of your speed.
> If that product is worse than the route you skipped, the jump is dead no
> matter how much time it saves to the next checkpoint — and no search will
> find otherwise, because there is nothing to find.**

The other arm's sector 3: launch −107°, return leg +92° to +117°, needs ~130° of
yaw, flight turns 38° (it turns at all only because that flight clips). Dead.
My sector 4: needs 53°, flight turns 0°. Dead, and for the cleaner reason.

The corollary is the useful half: **the jumps that work are the ones whose
landing surface runs the same way as the flight.** 227969's kicker-into-finish
worked because the finish plane was square to the flight. 270051's flying finish
worked for the same reason. The question to ask of any new jump is not "how much
time does it save" but "what is the angle between the flight and the landing".

## 7. What is still true about the technique

**The sector-4 finding itself is unaffected and remains the best route result on
this map.** The angled ramp jump reaches checkpoint 5 in 32.702 s against the
best human's 33.830 — validated by the plain oracle on the untouched map with
`cps = 5`, a real checkpoint volume — where all 181 humans either avoid the ramp
or take it square, and 0 of 181 land where it lands. What is now established is
that **the saving is real and unbankable**: this map's geometry converts it back
into speed loss at the landing. That is a complete answer to "why has nobody
done this", and it is a better answer than "nobody tried".
